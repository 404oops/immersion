//! Update preferences share app configuration without replacing unrelated settings.
use musit_core::backend::AppBackend;
use musit_core::project_registry::{self, ProjectRegistry};
use serde_json::json;

#[test]
fn update_preference_survives_other_settings_and_rejects_invalid_config() {
    let config = tempfile::tempdir().unwrap();
    project_registry::set_app_config_directory_override(Some(
        config.path().to_string_lossy().into_owned(),
    ));
    let registry = ProjectRegistry;
    let path = config.path().join("config.json");
    assert!(registry.load_app_settings().auto_updates_enabled);
    std::fs::write(
        &path,
        json!({"language": "sr", "custom": {"keep": true}}).to_string(),
    )
    .unwrap();
    assert!(registry.load_app_settings().auto_updates_enabled);

    // An already loaded backend must not overwrite changes made by the updater.
    let mut backend = AppBackend::new();
    for enabled in [false, true, false] {
        assert!(registry.set_auto_updates_enabled(enabled));
        assert_eq!(registry.load_app_settings().auto_updates_enabled, enabled);
        backend.set_notifications_enabled(!backend.notifications_enabled());
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["auto_updates_enabled"], enabled);
        assert_eq!(saved["language"], "sr");
        assert_eq!(saved["custom"], json!({"keep": true}));
    }
    drop(backend);
    std::fs::write(&path, "broken configuration").unwrap();
    assert!(!registry.set_auto_updates_enabled(true));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "broken configuration"
    );
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(!registry.set_auto_updates_enabled(true));
}
