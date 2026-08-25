//! Platform glue (PlatformAgent + TrayController + SingleInstanceGuard in Qt).

pub mod single_instance;

#[cfg(target_os = "macos")]
pub mod mac;

#[cfg(target_os = "macos")]
pub use mac::{
    adopt_main_window, hide_main_window, install_activation_observer, install_status_item,
    is_launch_at_startup_supported, set_background_agent_mode, set_dock_icon_if_unbundled,
    set_launch_at_startup, show_main_window, show_notification,
};

#[cfg(not(target_os = "macos"))]
pub fn set_background_agent_mode(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn adopt_main_window(_title: &str) {}

#[cfg(not(target_os = "macos"))]
pub fn hide_main_window() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn show_main_window() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn install_activation_observer(_on_activate: Box<dyn Fn()>) {}

// Launch at login on Windows: HKCU Run key, like PlatformAgent_win.cpp.
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
            &format!("\"{exe}\""),
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

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn set_launch_at_startup(_enabled: bool) {}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn is_launch_at_startup_supported() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn show_notification(_title: &str, _body: &str) {}

#[cfg(not(target_os = "macos"))]
pub fn install_status_item(_on_open: Box<dyn Fn()>, _on_quit: Box<dyn Fn()>) {}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon_if_unbundled() {}
