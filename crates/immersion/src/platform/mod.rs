//! Platform glue: background-agent mode, window show/hide, status item,
//! launch-at-login and the single-instance guard, with no-op fallbacks on
//! platforms that have no implementation yet.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_autostart;
#[cfg(target_os = "linux")]
mod linux_icons;
#[cfg(target_os = "linux")]
mod linux_window;
pub mod single_instance;
#[cfg(target_os = "linux")]
pub use linux::{install_status_item, poll_status_item, show_notification, status_item_available};
/// Passed by the Linux autostart entry and the Windows Run key so a login
/// launch can start out of the way.
#[cfg(any(target_os = "linux", target_os = "windows"))]
const AUTOSTART_ARG: &str = "--autostart";

/// Whether this process was started by the Windows Run key.
#[cfg(target_os = "windows")]
pub fn launched_at_login() -> bool {
    std::env::args().skip(1).any(|arg| arg == AUTOSTART_ARG)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn launched_at_login() -> bool {
    false
}

#[cfg(target_os = "linux")]
pub use linux_autostart::{
    is_launch_at_startup_supported, launched_at_login, set_launch_at_startup,
};
#[cfg(target_os = "linux")]
pub use linux_icons::file_type_icon_png;
#[cfg(target_os = "linux")]
pub use linux_window::{
    adopt_main_window as adopt_x11_window, hide_main_window, hide_main_window_if_mapped,
    show_main_window,
};

#[cfg(target_os = "macos")]
pub mod mac;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub mod icon_pixels;

#[cfg(target_os = "windows")]
pub mod win;

#[cfg(target_os = "windows")]
pub use win::file_type_icon_png;

#[cfg(target_os = "macos")]
pub use mac::{
    adopt_main_window, file_type_icon_png, hide_main_window, install_activation_observer,
    install_status_item, is_launch_at_startup_supported, launched_at_login,
    set_background_agent_mode, set_dock_icon_if_unbundled, set_launch_at_startup, show_main_window,
    show_notification, suppress_launch_activation_reveal,
};

/// OS document icon for a file extension as PNG bytes. macOS, Windows and
/// Linux have implementations; elsewhere the UI keeps its monogram tiles.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn file_type_icon_png(_extension: &str, _size_px: usize) -> Option<Vec<u8>> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn set_background_agent_mode(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn adopt_main_window(_title: &str) {}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn hide_main_window() -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn show_main_window() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn install_activation_observer(_on_activate: Box<dyn Fn()>) {}

// Launch at login on Windows: HKCU Run key.
#[cfg(target_os = "windows")]
pub fn set_launch_at_startup(enabled: bool) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let mut command = std::process::Command::new("reg");
    if enabled {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let exe = exe.to_string_lossy().replace('/', "\\");
        command.args([
            "add",
            RUN_KEY,
            "/v",
            "Immersion",
            "/t",
            "REG_SZ",
            "/d",
            &format!("\"{exe}\" {AUTOSTART_ARG}"),
            "/f",
        ]);
    } else {
        command.args(["delete", RUN_KEY, "/v", "Immersion", "/f"]);
    }
    let _ = command.creation_flags(CREATE_NO_WINDOW).status();
}

#[cfg(target_os = "windows")]
pub fn is_launch_at_startup_supported() -> bool {
    true
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn set_launch_at_startup(_enabled: bool) {}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn is_launch_at_startup_supported() -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn show_notification(_title: &str, _body: &str) {}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn install_status_item(_on_open: Box<dyn Fn()>, _on_quit: Box<dyn Fn()>) {}

#[cfg(not(target_os = "linux"))]
pub fn status_item_available() -> bool {
    cfg!(target_os = "macos")
}

#[cfg(not(target_os = "linux"))]
pub fn poll_status_item() {}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon_if_unbundled() {}
