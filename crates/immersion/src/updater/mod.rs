//! Signed, platform-owned updates. Never stop an active monitoring session.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "windows", test))]
mod manifest;
#[cfg(target_os = "windows")]
pub mod windows;

/// Also honored by developer bundles, before loading any updater framework.
pub fn disabled() -> bool {
    externally_disabled()
        || !musit_core::project_registry::ProjectRegistry
            .load_app_settings()
            .auto_updates_enabled
        || dirs::config_dir().is_some_and(|dir| dir.join("Immersion/disable-updates").exists())
}

pub fn supported() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

pub fn externally_disabled() -> bool {
    std::env::var_os("IMMERSION_DISABLE_UPDATES").is_some()
}

/// Persist before changing runtime behavior; never overwrite a malformed config.
pub fn set_enabled(enabled: bool) -> std::io::Result<()> {
    if !musit_core::project_registry::ProjectRegistry.set_auto_updates_enabled(enabled) {
        return Err(std::io::Error::other("could not save update preference"));
    }
    // Older releases use this marker; clear it when explicitly enabling updates.
    if enabled && let Some(config_dir) = dirs::config_dir() {
        match std::fs::remove_file(config_dir.join("Immersion/disable-updates")) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    #[cfg(target_os = "macos")]
    macos::settings_changed();
    #[cfg(target_os = "windows")]
    windows::settings_changed();
    Ok(())
}

pub fn start() {
    #[cfg(target_os = "macos")]
    macos::start();
    #[cfg(target_os = "windows")]
    windows::start();
}
