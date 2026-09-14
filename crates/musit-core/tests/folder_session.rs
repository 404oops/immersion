//! A folder tab's session (its name and its colour) lives in the folder's own
//! `.immersion/settings.json`, so removing the tab and adding the folder back
//! — on this machine or another one — restores what the user had.
//!
//! One #[test]: the app config directory override is process-global.

use musit_core::backend::{AppBackend, PlatformHooks};
use musit_core::folder_settings;
use musit_core::project_registry;
use std::fs;
use std::time::{Duration, Instant};

fn test_platform() -> PlatformHooks {
    PlatformHooks {
        launch_at_startup_supported: false,
        set_launch_at_startup: Box::new(|_| {}),
        open_path: Box::new(|_| true),
    }
}

fn wait_for(
    backend: &mut AppBackend,
    timeout: Duration,
    mut predicate: impl FnMut(&mut AppBackend) -> bool,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        backend.process_pending();
        if predicate(backend) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    backend.process_pending();
    predicate(backend)
}

#[test]
fn folder_session_survives_removal_and_re_adding() {
    let fake_config = tempfile::tempdir().expect("temp config dir");
    project_registry::set_app_config_directory_override(Some(
        fake_config.path().to_string_lossy().to_string(),
    ));

    let projects_dir = tempfile::tempdir().expect("temp projects dir");
    let root = projects_dir.path().to_string_lossy().replace('\\', "/");
    fs::create_dir_all(format!("{root}/Song Project")).expect("project folder");
    fs::write(format!("{root}/Song Project/Song.als"), b"als").expect("project file");

    // ---- First session: add the folder, rename its tab, pick a colour ----
    let mut backend = AppBackend::with_platform(test_platform());
    backend.add_projects_folder(&root, "Bundles");
    assert!(
        wait_for(&mut backend, Duration::from_secs(20), |backend| {
            !backend.folder_tabs().is_empty()
        }),
        "the folder becomes a tab"
    );

    backend.rename_projects_folder(0, "Late Night Beats");
    backend.set_theme_hue(212.0);
    // Saturation is app-wide, not the folder's: it goes to the app config.
    backend.set_theme_saturation(0.6);
    // The theme writes are debounced; shutdown flushes them.
    backend.flush_pending_persist();

    assert_eq!(
        backend.folder_tabs()[0].name,
        "Late Night Beats",
        "the tab shows the chosen name"
    );

    let saved = folder_settings::load(&root);
    assert_eq!(
        saved.name, "Late Night Beats",
        "name is saved in the folder"
    );
    assert_eq!(saved.hue, Some(212.0), "colour is saved in the folder");

    // ---- Remove the tab: the folder is forgotten by the app ----
    backend.remove_projects_folder(0);
    assert!(
        backend.folder_tabs().is_empty(),
        "removing the tab leaves no folders"
    );
    drop(backend);

    // ---- Second session, fresh app state: add the same folder back ----
    let mut backend = AppBackend::with_platform(test_platform());
    assert!(
        backend.folder_tabs().is_empty(),
        "the removed folder does not come back on its own"
    );
    assert_eq!(
        backend.theme_saturation(),
        0.6,
        "saturation comes back from the app config"
    );
    // A different colour is in play before the folder is re-added.
    backend.set_theme_hue(30.0);

    backend.add_projects_folder(&root, "Bundles");
    assert!(
        wait_for(&mut backend, Duration::from_secs(20), |backend| {
            !backend.folder_tabs().is_empty()
        }),
        "the folder becomes a tab again"
    );

    assert_eq!(
        backend.folder_tabs()[0].name,
        "Late Night Beats",
        "re-adding the folder restores its tab name"
    );
    assert_eq!(
        backend.theme_hue(),
        212.0,
        "re-adding the folder restores its colour"
    );

    // ---- A folder with no saved session keeps the current colour ----
    let plain_dir = tempfile::tempdir().expect("second projects dir");
    let plain_root = plain_dir.path().to_string_lossy().replace('\\', "/");
    fs::create_dir_all(format!("{plain_root}/Other Project")).expect("project folder");
    fs::write(format!("{plain_root}/Other Project/Other.als"), b"als").expect("project file");

    backend.add_projects_folder(&plain_root, "Bundles");
    assert!(
        wait_for(&mut backend, Duration::from_secs(20), |backend| {
            backend.folder_tabs().len() == 2
        }),
        "the second folder becomes a tab"
    );
    assert_eq!(
        backend.folder_tabs()[1].name,
        std::path::Path::new(&plain_root)
            .file_name()
            .expect("folder name")
            .to_string_lossy(),
        "a folder with no saved name falls back to the folder's own name"
    );
    assert_eq!(
        folder_settings::load(&plain_root).hue,
        Some(212.0),
        "a folder with no colour adopts the current one and remembers it"
    );

    // ---- The folder's type is taken from its config, never re-guessed ----
    backend.set_active_folder_index(0);
    backend.confirm_projects_folder(&root, "Files");
    assert_eq!(
        folder_settings::load(&root).layout,
        folder_settings::ProjectsFolderLayout::Files,
        "confirming a layout saves it"
    );

    // Adding it again with no layout given must keep what the folder says,
    // rather than falling back to the default.
    backend.add_projects_folder(&root, "");
    assert_eq!(
        backend.projects_folder_layout_for_path(&root),
        "Files",
        "re-adding with no layout keeps the folder's saved type"
    );
    assert_eq!(
        folder_settings::load(&root).layout,
        folder_settings::ProjectsFolderLayout::Files,
        "re-adding with no layout does not rewrite the saved type"
    );

    // ---- Switching tabs switches the colour ----
    // Explicit about which tab is active: the colour set below belongs to it.
    backend.set_active_folder_index(1);
    backend.set_theme_hue(95.0);
    backend.flush_pending_persist();
    backend.set_active_folder_index(0);
    assert_eq!(
        backend.theme_hue(),
        212.0,
        "switching back to the first tab restores its colour"
    );
    backend.set_active_folder_index(1);
    assert_eq!(
        backend.theme_hue(),
        95.0,
        "switching to the second tab restores the colour set while it was active"
    );
}
