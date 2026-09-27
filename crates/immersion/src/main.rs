//! Immersion — GPUI desktop shell for the musit core.

mod app;
mod platform;
mod theme;
mod ui;

use std::cell::{Cell, RefCell};
use std::time::Duration;

use gpui::{
    App, AsyncApp, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions,
    actions, prelude::*, px, size,
};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem as OsMenuItem, SystemMenuType};
use gpui_platform::application;

use crate::platform::single_instance::{self, InstanceGuard};
use vampir::text_input as ti;

#[cfg(target_os = "linux")]
const LINUX_APP_ID: &str = "io.github._404oops.immersion";
#[cfg(target_os = "linux")]
const LINUX_ICON: vampir::AppIcon = vampir::AppIcon::png(include_bytes!("../assets/icons/app.png"));

actions!(immersion, [Quit, Hide, HideOthers, ShowAll]);

thread_local! {
    static MAIN_WINDOW: RefCell<Option<WindowHandle<app::RootView>>> = const { RefCell::new(None) };
    // Whether the app is hidden in the menu bar (window closed / agent mode).
    static APP_HIDDEN: Cell<bool> = const { Cell::new(false) };
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
    cx.activate(true);
    if !platform::show_main_window() {
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

    // Start hidden in background-agent mode when a projects folder is
    // already configured. Only on macOS: other platforms have no status item
    // yet, so a hidden window would be unreachable.
    let saved_folders = musit_core::project_registry::ProjectRegistry.load_projects_folders();
    let start_hidden = cfg!(target_os = "macos")
        && !saved_folders.is_empty()
        && saved_folders
            .iter()
            .all(|folder| musit_core::folder_settings::has_layout_setting(folder));

    let gpui_app = application();

    // Dock icon click / Spotlight reopen shows the window.
    gpui_app.on_reopen(|cx| present_main_window(cx));

    gpui_app.run(move |cx: &mut App| {
        #[cfg(target_os = "linux")]
        cx.set_app_identity(LINUX_APP_ID, "Immersion");

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
            cx.set_menus(vec![
                Menu::new("Immersion").items([
                    OsMenuItem::os_submenu("Services", SystemMenuType::Services),
                    OsMenuItem::separator(),
                    OsMenuItem::action("Hide Immersion", Hide),
                    OsMenuItem::action("Hide Others", HideOthers),
                    OsMenuItem::action("Show All", ShowAll),
                    OsMenuItem::separator(),
                    OsMenuItem::action("Quit Immersion", Quit),
                ]),
                vampir::edit_menu(),
                Menu::new("Window").items(Vec::<OsMenuItem>::new()),
            ]);
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
        #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
        let mut options = WindowOptions {
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
        {
            options.app_id = Some(LINUX_APP_ID.to_string());
            options.icon = LINUX_ICON.window_icon();
        }
        let window = cx
            .open_window(options, |window, cx| {
                cx.new(|cx| app::RootView::new(window, cx))
            })
            .expect("failed to open main window");

        MAIN_WINDOW.with(|slot| *slot.borrow_mut() = Some(window));
        // Take ownership of the NSWindow for animated hide/show.
        platform::adopt_main_window("Immersion");

        // Closing the window leaves monitoring active where a tray control is
        // available. GPUI cannot hide on Linux, so minimize its X11 window.
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
                        _win.minimize_window();
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
                    cx.update(|cx| present_main_window(cx));
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
                let _ = cx.update(|cx| present_main_window(cx));
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
                    cx.update(|cx| present_main_window(cx));
                }
            }
        })
        .detach();

        if start_hidden {
            platform::set_background_agent_mode(true);
            APP_HIDDEN.with(|hidden| hidden.set(true));
        } else {
            cx.activate(true);
        }
    });

    // Reached only if run() ever returns (quit is handled in on_app_quit).
    single_instance::cleanup();
}
