//! Immersion — GPUI shell for the musit core (port of the Qt/QML app).

mod app;
mod platform;
mod text_input;
mod theme;
mod ui;

use std::cell::RefCell;
use std::time::Duration;

use gpui::{
    App, AsyncApp, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions,
    actions, prelude::*, px, size,
};
use gpui_platform::application;

use crate::platform::single_instance::{self, InstanceGuard};
use crate::text_input as ti;

actions!(immersion, [Quit, CloseModal]);

thread_local! {
    static MAIN_WINDOW: RefCell<Option<WindowHandle<app::RootView>>> = const { RefCell::new(None) };
}

/// TrayController::showMainWindow + PlatformAgent::presentMainWindow.
fn present_main_window(cx: &mut App) {
    platform::set_background_agent_mode(false);
    MAIN_WINDOW.with(|window| {
        if let Some(handle) = window.borrow().as_ref() {
            handle
                .update(cx, |_view, win, _cx| {
                    win.activate_window();
                })
                .ok();
        }
    });
    cx.activate(true);
}

fn main() {
    // Single instance: secondary instances ask the primary to raise itself.
    let raise_rx = match single_instance::acquire() {
        InstanceGuard::Primary(rx) => rx,
        InstanceGuard::Secondary => return,
    };

    // Qt starts hidden in background-agent mode when a projects folder is
    // already configured (TrayController::attach).
    let saved_folder = musit_core::project_registry::ProjectRegistry.load_projects_folder();
    let start_hidden =
        !saved_folder.is_empty() && musit_core::folder_settings::has_layout_setting(&saved_folder);

    let gpui_app = application();

    // Dock icon click / Spotlight reopen shows the window.
    gpui_app.on_reopen(|cx| present_main_window(cx));

    gpui_app.run(move |cx: &mut App| {
        // Dev builds run outside a .app bundle; give the Dock the real icon.
        platform::set_dock_icon_if_unbundled();

        cx.bind_keys([
            // Text editing (context-scoped to text inputs).
            KeyBinding::new("backspace", ti::Backspace, Some("TextInput")),
            KeyBinding::new("delete", ti::Delete, Some("TextInput")),
            KeyBinding::new("left", ti::Left, Some("TextInput")),
            KeyBinding::new("right", ti::Right, Some("TextInput")),
            KeyBinding::new("up", ti::Up, Some("TextInput")),
            KeyBinding::new("down", ti::Down, Some("TextInput")),
            KeyBinding::new("shift-left", ti::SelectLeft, Some("TextInput")),
            KeyBinding::new("shift-right", ti::SelectRight, Some("TextInput")),
            KeyBinding::new("cmd-a", ti::SelectAll, Some("TextInput")),
            KeyBinding::new("cmd-v", ti::Paste, Some("TextInput")),
            KeyBinding::new("cmd-c", ti::Copy, Some("TextInput")),
            KeyBinding::new("cmd-x", ti::Cut, Some("TextInput")),
            KeyBinding::new("home", ti::Home, Some("TextInput")),
            KeyBinding::new("end", ti::End, Some("TextInput")),
            KeyBinding::new("enter", ti::Enter, Some("TextInput")),
            KeyBinding::new("ctrl-cmd-space", ti::ShowCharacterPalette, Some("TextInput")),
            // App-level.
            KeyBinding::new("escape", CloseModal, None),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(880.0), px(560.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Immersion".into()),
                        ..Default::default()
                    }),
                    show: !start_hidden,
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| app::RootView::new(window, cx)),
            )
            .expect("failed to open main window");

        MAIN_WINDOW.with(|slot| *slot.borrow_mut() = Some(window));

        // Closing the window hides the app into the status bar (tray app).
        window
            .update(cx, |_view, win, cx| {
                win.on_window_should_close(cx, |_win, cx| {
                    cx.hide();
                    platform::set_background_agent_mode(true);
                    false
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

        // Raise requests from secondary instances.
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                if raise_rx.try_recv().is_ok() {
                    cx.update(|cx| present_main_window(cx));
                }
            }
        })
        .detach();

        if start_hidden {
            platform::set_background_agent_mode(true);
        } else {
            cx.activate(true);
        }
    });

    single_instance::cleanup();
}
