//! Signed, platform-owned updates. Never stop an active monitoring session.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "windows", test))]
mod manifest;
#[cfg(target_os = "windows")]
pub mod windows;

/// Also honored by developer bundles, before loading any updater framework.
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn disabled() -> bool {
    std::env::var_os("IMMERSION_DISABLE_UPDATES").is_some()
        || dirs::config_dir().is_some_and(|dir| dir.join("Immersion/disable-updates").exists())
}

pub fn start() {
    #[cfg(target_os = "macos")]
    macos::start();
    #[cfg(target_os = "windows")]
    windows::start();
}
