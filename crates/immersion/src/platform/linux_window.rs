//! Linux (X11): hiding the main window to the tray. GPUI can only minimize
//! on Linux, which leaves the window in the launcher and task switcher; a
//! withdrawn window leaves both, as the macOS window does when it closes.
//! GPUI stops drawing while its window is unmapped.

use std::cell::RefCell;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConnectionExt, EventMask, MapState, UNMAP_NOTIFY_EVENT, UnmapNotifyEvent,
};
use x11rb::rust_connection::RustConnection;

/// The main window on a connection of our own; GPUI does not expose its.
struct MainWindow {
    connection: RustConnection,
    root: u32,
    window: u32,
}

thread_local! {
    static MAIN_WINDOW: RefCell<Option<MainWindow>> = const { RefCell::new(None) };
}

fn with_main_window(action: impl FnOnce(&MainWindow) -> bool) -> bool {
    MAIN_WINDOW.with(|slot| slot.borrow().as_ref().is_some_and(action))
}

/// Records the X11 window behind GPUI's main window.
pub fn adopt_main_window(window: &impl HasWindowHandle) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let id = match handle.as_raw() {
        RawWindowHandle::Xcb(handle) => handle.window.get(),
        RawWindowHandle::Xlib(handle) => handle.window as u32,
        _ => return,
    };
    let Ok((connection, screen)) = x11rb::connect(None) else {
        return;
    };
    let Some(root) = connection
        .setup()
        .roots
        .get(screen)
        .map(|screen| screen.root)
    else {
        return;
    };
    MAIN_WINDOW.with(|slot| {
        *slot.borrow_mut() = Some(MainWindow {
            connection,
            root,
            window: id,
        });
    });
}

/// ICCCM withdrawal: unmap, then tell the window manager with a synthetic
/// UnmapNotify so it also forgets an iconified window.
fn withdraw(main: &MainWindow) -> bool {
    let connection = &main.connection;
    let notify = UnmapNotifyEvent {
        response_type: UNMAP_NOTIFY_EVENT,
        sequence: 0,
        event: main.root,
        window: main.window,
        from_configure: false,
    };
    connection.unmap_window(main.window).is_ok()
        && connection
            .send_event(
                false,
                main.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                notify,
            )
            .is_ok()
        && connection.flush().is_ok()
}

pub fn hide_main_window() -> bool {
    with_main_window(withdraw)
}

/// Withdraws the window once the window manager has mapped it, and reports
/// whether it did. GPUI maps every new Linux window, and withdrawing before
/// the window manager has handled that map would be undone by it.
pub fn hide_main_window_if_mapped() -> bool {
    with_main_window(|main| {
        let viewable = main
            .connection
            .get_window_attributes(main.window)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|attributes| attributes.map_state == MapState::VIEWABLE);
        viewable && withdraw(main)
    })
}

/// Maps the window again. GPUI's activation then raises and focuses it.
pub fn show_main_window() -> bool {
    with_main_window(|main| {
        main.connection.map_window(main.window).is_ok() && main.connection.flush().is_ok()
    })
}
