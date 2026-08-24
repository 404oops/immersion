//! Platform glue (PlatformAgent + TrayController + SingleInstanceGuard in Qt).

pub mod single_instance;

#[cfg(target_os = "macos")]
pub mod mac;

#[cfg(target_os = "macos")]
pub use mac::{
    install_status_item, is_launch_at_startup_supported, set_background_agent_mode,
    set_dock_icon_if_unbundled, set_launch_at_startup, show_notification,
};

#[cfg(not(target_os = "macos"))]
pub fn set_background_agent_mode(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_launch_at_startup(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn is_launch_at_startup_supported() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn show_notification(_title: &str, _body: &str) {}

#[cfg(not(target_os = "macos"))]
pub fn install_status_item(_on_open: Box<dyn Fn()>, _on_quit: Box<dyn Fn()>) {}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon_if_unbundled() {}

#[cfg(not(target_os = "macos"))]
pub fn is_bundled() -> bool {
    false
}
