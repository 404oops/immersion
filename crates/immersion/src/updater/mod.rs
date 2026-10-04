//! Signed, platform-owned updates. Never stop an active monitoring session.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "windows", test))]
mod manifest;
#[cfg(target_os = "windows")]
pub mod windows;

/// Also honored by developer bundles, before loading any updater framework.
pub fn disabled() -> bool {
    std::env::var_os("IMMERSION_DISABLE_UPDATES").is_some()
        || dirs::config_dir().is_some_and(|dir| dir.join("Immersion/disable-updates").exists())
}

pub fn supported() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

pub fn externally_disabled() -> bool {
    std::env::var_os("IMMERSION_DISABLE_UPDATES").is_some()
}

fn save_enabled(config_dir: &std::path::Path, enabled: bool) -> std::io::Result<()> {
    let marker = config_dir.join("Immersion/disable-updates");
    if enabled {
        match std::fs::remove_file(marker) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    } else {
        std::fs::create_dir_all(marker.parent().unwrap())?;
        std::fs::File::create(marker)?;
    }
    Ok(())
}

/// Persist before changing runtime behavior; a failed write leaves the switch unchanged.
pub fn set_enabled(enabled: bool) -> std::io::Result<()> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| std::io::Error::other("no configuration directory"))?;
    save_enabled(&config_dir, enabled)?;
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

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn update_preference_preserves_other_settings_and_is_idempotent() {
        let config = tempfile::tempdir().unwrap();
        let app = config.path().join("Immersion");
        std::fs::create_dir(&app).unwrap();
        std::fs::write(app.join("settings.json"), "preserved").unwrap();
        for enabled in [true, false, false, true, true] {
            save_enabled(config.path(), enabled).unwrap();
            assert_eq!(app.join("disable-updates").exists(), !enabled);
            assert_eq!(
                std::fs::read_to_string(app.join("settings.json")).unwrap(),
                "preserved"
            );
        }
    }

    #[test]
    fn preference_write_failure_is_reported() {
        let config = tempfile::tempdir().unwrap();
        std::fs::write(config.path().join("Immersion"), "not a directory").unwrap();
        assert!(save_enabled(config.path(), false).is_err());
    }
}
