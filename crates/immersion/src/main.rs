//! Immersion — GPUI desktop shell for the musit core.

mod app;
mod i18n;
mod platform;
mod theme;
mod ui;
mod updater;

use std::cell::{Cell, RefCell};
use std::time::Duration;

use gpui::{
    App, AsyncApp, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions,
    actions, prelude::*, px, size,
};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem as OsMenuItem, OsAction, SystemMenuType};
use gpui_platform::application;

use crate::platform::single_instance::{self, InstanceGuard};
use vampir::text_input as ti;

#[cfg(target_os = "linux")]
pub(crate) const LINUX_APP_ID: &str = "io.github._404oops.immersion";
// 256px, not the 2048px master: a 2048px RGBA _NET_WM_ICON is 16 MiB, past
// the X server's maximum request size, and opening the window fails.
#[cfg(target_os = "linux")]
const LINUX_ICON: vampir::AppIcon =
    vampir::AppIcon::png(include_bytes!("../assets/icons/window-linux.png"));

actions!(immersion, [Quit, Hide, HideOthers, ShowAll]);

#[cfg(target_os = "macos")]
fn set_localized_menus(cx: &App) {
    use crate::i18n::tr;
    cx.set_menus(vec![
        Menu::new("Immersion").items([
            OsMenuItem::os_submenu(tr("menu.services"), SystemMenuType::Services),
            OsMenuItem::separator(),
            OsMenuItem::action(tr("menu.hide_immersion"), Hide),
            OsMenuItem::action(tr("menu.hide_others"), HideOthers),
            OsMenuItem::action(tr("menu.show_all"), ShowAll),
            OsMenuItem::separator(),
            OsMenuItem::action(tr("menu.quit_immersion"), Quit),
        ]),
        Menu::new(tr("menu.edit")).items([
            OsMenuItem::os_action(tr("menu.undo"), ti::Undo, OsAction::Undo),
            OsMenuItem::os_action(tr("menu.redo"), ti::Redo, OsAction::Redo),
            OsMenuItem::separator(),
            OsMenuItem::os_action(tr("menu.cut"), ti::Cut, OsAction::Cut),
            OsMenuItem::os_action(tr("menu.copy"), ti::Copy, OsAction::Copy),
            OsMenuItem::os_action(tr("menu.paste"), ti::Paste, OsAction::Paste),
            OsMenuItem::os_action(tr("menu.select_all"), ti::SelectAll, OsAction::SelectAll),
        ]),
        Menu::new(tr("menu.window")).items(Vec::<OsMenuItem>::new()),
    ]);
}

thread_local! {
    static MAIN_WINDOW: RefCell<Option<WindowHandle<app::RootView>>> = const { RefCell::new(None) };
    // Whether the app is hidden in the menu bar (window closed / agent mode).
    static APP_HIDDEN: Cell<bool> = const { Cell::new(false) };
    // Whether a Linux login launch is still waiting to withdraw the window.
    #[cfg(target_os = "linux")]
    static LOGIN_HIDE_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// Brings the main window back: leaves background-agent mode, activates the
/// app and shows (or re-activates) the window.
///
/// The window is ordered out while hidden rather than kept on screen behind a
/// hidden app, so ordering it back in plays the system's window-open
/// animation instead of the window blinking into place.
fn present_main_window(cx: &mut App) {
    platform::set_background_agent_mode(false);
    APP_HIDDEN.with(|hidden| hidden.set(false));
    #[cfg(target_os = "linux")]
    LOGIN_HIDE_PENDING.with(|pending| pending.set(false));
    cx.activate(true);
    // Linux only maps the window here; GPUI's activation raises it.
    if !platform::show_main_window() || cfg!(target_os = "linux") {
        MAIN_WINDOW.with(|window| {
            if let Some(handle) = window.borrow().as_ref() {
                handle
                    .update(cx, |_view, win, _cx| {
                        win.activate_window();
                    })
                    .ok();
            }
        });
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    if updater::windows::run_helper_if_requested() {
        return;
    }
    // gpui-ce enables its Wayland feature transitively. Force X11 at process
    // startup so Wayland sessions use XWayland instead of the broken backend.
    #[cfg(target_os = "linux")]
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    // Single instance: secondary instances ask the primary to raise itself.
    let raise_rx = match single_instance::acquire() {
        InstanceGuard::Primary(rx) => rx,
        InstanceGuard::Secondary => return,
    };

    #[cfg(target_os = "windows")]
    if updater::windows::install_pending() {
        single_instance::cleanup();
        return;
    }

    // A launch at login stays out of the way once every projects folder is
    // set up; any other launch opens the window.
    let saved_folders = musit_core::project_registry::ProjectRegistry.load_projects_folders();
    let configured = !saved_folders.is_empty()
        && saved_folders
            .iter()
            .all(|folder| musit_core::folder_settings::has_layout_setting(folder));

    let gpui_app = application();

    // Dock icon click / Spotlight reopen shows the window.
    gpui_app.on_reopen(present_main_window);

    gpui_app.run(move |cx: &mut App| {
        // macOS can only tell a login launch while the launch is handled,
        // which is now. It starts hidden in the menu bar; Linux withdraws to
        // the tray once one shows (below); Windows, with no tray yet,
        // minimizes.
        let login_launch = configured && platform::launched_at_login();
        let start_hidden = cfg!(target_os = "macos") && login_launch;

        #[cfg(target_os = "linux")]
        cx.set_app_identity(LINUX_APP_ID, "Immersion");

        updater::start();

        // Dev builds run outside a .app bundle; give the Dock the real icon.
        platform::set_dock_icon_if_unbundled();

        // Tab, Shift-Tab, Escape and the text-editing keys are the toolkit's;
        // the application's own keys go in after, so they take precedence.
        vampir::bind_keys(cx);
        cx.bind_keys([
            KeyBinding::new(
                "ctrl-cmd-space",
                ti::ShowCharacterPalette,
                Some("TextInput"),
            ),
            KeyBinding::new("secondary-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        #[cfg(target_os = "macos")]
        {
            cx.bind_keys([
                KeyBinding::new("cmd-h", Hide, None),
                KeyBinding::new("cmd-alt-h", HideOthers, None),
            ]);
            cx.on_action(|_: &Hide, cx: &mut App| cx.hide());
            cx.on_action(|_: &HideOthers, cx: &mut App| cx.hide_other_apps());
            cx.on_action(|_: &ShowAll, cx: &mut App| cx.unhide_other_apps());
            set_localized_menus(cx);
        }

        // gpui's quit path never returns from run(), so shutdown work has to
        // hang off the quit hook: flush debounced settings and remove the
        // single-instance endpoint.
        cx.on_app_quit(|cx| {
            MAIN_WINDOW.with(|window| {
                if let Some(handle) = window.borrow().as_ref() {
                    let _ = handle.update(cx, |view, _win, _cx| view.flush_before_quit());
                }
            });
            single_instance::cleanup();
            async {}
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(880.0), px(560.0))),
            titlebar: Some(TitlebarOptions {
                title: Some("Immersion".into()),
                ..Default::default()
            }),
            show: !start_hidden,
            ..Default::default()
        };
        #[cfg(target_os = "linux")]
        let options = WindowOptions {
            app_id: Some(LINUX_APP_ID.to_string()),
            icon: LINUX_ICON.window_icon(),
            ..options
        };
        let window = cx
            .open_window(options, |window, cx| {
                cx.new(|cx| app::RootView::new(window, cx))
            })
            .expect("failed to open main window");

        MAIN_WINDOW.with(|slot| *slot.borrow_mut() = Some(window));
        // Take ownership of the NSWindow for animated hide/show.
        platform::adopt_main_window("Immersion");
        #[cfg(target_os = "linux")]
        window
            .update(cx, |_view, win, _cx| platform::adopt_x11_window(win))
            .ok();

        // Closing the window leaves monitoring active where a tray control is
        // available. On Linux the window is withdrawn from X11, or minimized
        // if that fails.
        window
            .update(cx, |_view, win, cx| {
                win.on_window_should_close(cx, |_win, cx| {
                    if cfg!(target_os = "macos") {
                        // Order the window out (animated) rather than hiding
                        // the whole app: an app-hidden window comes back with
                        // no animation at all.
                        if !platform::hide_main_window() {
                            cx.hide();
                        }
                        platform::set_background_agent_mode(true);
                        APP_HIDDEN.with(|hidden| hidden.set(true));
                        false
                    } else if cfg!(target_os = "linux") && platform::status_item_available() {
                        if !platform::hide_main_window() {
                            _win.minimize_window();
                        }
                        APP_HIDDEN.with(|hidden| hidden.set(true));
                        false
                    } else {
                        cx.quit();
                        true
                    }
                });
            })
            .ok();

        // Status bar item (menu bar): left-click opens, right-click menu.
        {
            let async_open: RefCell<AsyncApp> = RefCell::new(cx.to_async());
            let async_quit: RefCell<AsyncApp> = RefCell::new(cx.to_async());
            platform::install_status_item(
                Box::new(move || {
                    let cx = async_open.borrow_mut();
                    cx.update(present_main_window);
                }),
                Box::new(move || {
                    let cx = async_quit.borrow_mut();
                    cx.update(|cx| cx.quit());
                }),
            );
        }

        // Spotlight/Dock can activate the running process without triggering
        // applicationShouldHandleReopen; reveal the window on activation
        // while it's hidden.
        {
            let async_activate: RefCell<AsyncApp> = RefCell::new(cx.to_async());
            platform::install_activation_observer(Box::new(move || {
                if !APP_HIDDEN.with(|hidden| hidden.get()) {
                    return;
                }
                let cx = async_activate.borrow_mut();
                cx.update(present_main_window);
            }));
        }

        // Raise requests from secondary instances.
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                platform::poll_status_item();
                if raise_rx.try_recv().is_ok() {
                    cx.update(present_main_window);
                }
            }
        })
        .detach();

        // GPUI maps every Linux window when it opens, and autostart can run
        // before the window manager and the panel hosting the tray. Withdraw
        // the window once both are up; without a tray within 30 seconds it
        // stays, since it would be unreachable. Opening it first cancels.
        #[cfg(target_os = "linux")]
        if login_launch {
            LOGIN_HIDE_PENDING.with(|pending| pending.set(true));
            cx.spawn(async move |cx| {
                for _ in 0..1500 {
                    cx.background_executor()
                        .timer(Duration::from_millis(20))
                        .await;
                    if !LOGIN_HIDE_PENDING.with(|pending| pending.get()) {
                        break;
                    }
                    if platform::status_item_available() && platform::hide_main_window_if_mapped() {
                        APP_HIDDEN.with(|hidden| hidden.set(true));
                        break;
                    }
                }
                LOGIN_HIDE_PENDING.with(|pending| pending.set(false));
            })
            .detach();
        }
        #[cfg(target_os = "windows")]
        if login_launch {
            window
                .update(cx, |_view, win, _cx| win.minimize_window())
                .ok();
        }

        if start_hidden {
            platform::set_background_agent_mode(true);
            #[cfg(target_os = "macos")]
            platform::suppress_launch_activation_reveal();
            APP_HIDDEN.with(|hidden| hidden.set(true));
        } else if !login_launch {
            cx.activate(true);
        }
    });

    // Reached only if run() ever returns (quit is handled in on_app_quit).
    single_instance::cleanup();
}
