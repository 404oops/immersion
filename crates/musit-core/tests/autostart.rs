//! The installer can set the OS login entry before Immersion has app settings.

use std::sync::{Arc, Mutex};

use musit_core::backend::{AppBackend, PlatformHooks};
use musit_core::project_registry::{self, AppSettings, ProjectRegistry};

#[test]
fn os_login_entry_is_reconciled_with_saved_settings() {
    let config = tempfile::tempdir().expect("temp config dir");
    project_registry::set_app_config_directory_override(Some(
        config.path().to_string_lossy().into_owned(),
    ));
    let registry = ProjectRegistry;
    assert!(registry.save_app_settings(&AppSettings::default()));

    let applied = Arc::new(Mutex::new(Vec::new()));
    let platform = |enabled| {
        let applied = applied.clone();
        PlatformHooks {
            launch_at_startup_supported: true,
            launch_at_startup_enabled: Some(enabled),
            set_launch_at_startup: Box::new(move |value| {
                applied.lock().expect("applied lock").push(value);
            }),
            ..PlatformHooks::default()
        }
    };

    let backend = AppBackend::with_platform(platform(true));
    assert!(backend.launch_at_startup());
    assert!(registry.load_app_settings().launch_at_startup);
    drop(backend);

    let backend = AppBackend::with_platform(platform(false));
    assert!(!backend.launch_at_startup());
    assert!(!registry.load_app_settings().launch_at_startup);
    assert_eq!(*applied.lock().expect("applied lock"), [true, false]);
}
