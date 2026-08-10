//! Immersion — GPUI shell for the musit core (port of the Qt/QML app).

mod app;
mod text_input;
mod theme;
mod ui;

use gpui::{
    App, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, actions, prelude::*,
    px, size,
};
use gpui_platform::application;

use crate::text_input as ti;

actions!(immersion, [Quit, CloseModal]);

fn main() {
    application().run(|cx: &mut App| {
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
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| app::RootView::new(window, cx)),
            )
            .expect("failed to open main window");

        // The Qt app hides its window on close (tray app); mirror that by
        // hiding the application and reactivating from the Dock.
        window
            .update(cx, |_view, win, cx| {
                win.on_window_should_close(cx, |_win, cx| {
                    cx.hide();
                    false
                });
            })
            .ok();

        cx.activate(true);
    });
}
