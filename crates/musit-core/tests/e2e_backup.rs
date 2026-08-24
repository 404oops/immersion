//! End-to-end test for universal backup templates + tiered compressed storage.
//!
//! Port of `qt-legacy/tests/e2e_backup_test.cpp`. Exercises the real
//! production stack (discovery, watcher, snapshot service, metadata/object
//! stores, AppBackend restore) against a fake Logic bundle:
//!   1. bundle internals (ProjectData, plists) are tracked, audio is not
//!   2. bundle files saved together share one grouped version
//!   3. staged copies are compacted past the 5 newest versions per file
//!   4. a compacted version restores from the compressed object store
//!   5. saving after a restore branches (1.1) on the artifact
//!
//! Everything runs in ONE #[test] because the app config directory override
//! is process-global.

use musit_core::backend::{AppBackend, PlatformHooks, VersionEntry};
use musit_core::backup_template;
use musit_core::folder_settings::ProjectsFolderLayout;
use musit_core::object_store::ObjectStore;
use musit_core::project_discovery;
use musit_core::project_registry;
use musit_core::snapshot_service::SnapshotService;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

fn wait_for(backend: &mut AppBackend, timeout: Duration, mut predicate: impl FnMut(&mut AppBackend) -> bool) -> bool {
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

fn write_file(path: &str, contents: &[u8]) -> bool {
    if let Some(parent) = Path::new(path).parent() {
        if fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    fs::write(path, contents).is_ok()
}

fn read_file(path: &str) -> Vec<u8> {
    fs::read(path).unwrap_or_default()
}

fn file_contains(path: &str, needle: &[u8]) -> bool {
    read_file(path)
        .windows(needle.len().max(1))
        .any(|window| window == needle)
}

fn version_by_id<'a>(versions: &'a [VersionEntry], id: &str) -> Option<&'a VersionEntry> {
    versions.iter().find(|v| v.id == id)
}

fn count_staged_files_named(project_root: &str, file_name: &str) -> usize {
    let staging_root = format!("{project_root}/.musit/staging");
    let mut count = 0;
    let mut stack = vec![staging_root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path.to_string_lossy().to_string());
            } else if entry.file_name().to_string_lossy() == file_name {
                count += 1;
            }
        }
    }
    count
}

fn staging_contains_suffix(project_root: &str, suffix: &str) -> bool {
    let staging_root = format!("{project_root}/.musit/staging");
    let suffix_lower = suffix.to_lowercase();
    let mut stack = vec![staging_root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path.to_string_lossy().to_string());
            } else if entry
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .ends_with(&suffix_lower)
            {
                return true;
            }
        }
    }
    false
}

fn project_index_by_file(backend: &AppBackend, file_name: &str) -> i32 {
    for (i, item) in backend.projects().iter().enumerate() {
        if item.file == file_name {
            return i as i32;
        }
    }
    -1
}

fn test_platform() -> PlatformHooks {
    PlatformHooks {
        launch_at_startup_supported: false,
        set_launch_at_startup: Box::new(|_| {}),
        // Never actually launch DAWs/Finder from the test suite.
        open_path: Box::new(|_| true),
    }
}

const WAIT: Duration = Duration::from_secs(20);

#[test]
fn e2e_backup() {
    // Isolate from the user's real config.
    let fake_config = tempfile::tempdir().expect("temp config dir");
    project_registry::set_app_config_directory_override(Some(
        fake_config.path().to_string_lossy().to_string(),
    ));

    // ---- Template registry unit checks (pure functions) ----
    assert!(
        backup_template::should_track_path("Song.logicx/Alternatives/000/ProjectData"),
        "template: ProjectData inside .logicx bundle is tracked"
    );
    assert!(
        backup_template::should_track_path("Song.logicx/Metadata.plist"),
        "template: plist inside .logicx bundle is tracked"
    );
    assert!(
        !backup_template::should_track_path("Song.logicx/Media/kick.wav"),
        "template: audio inside .logicx Media is excluded"
    );
    assert!(
        !backup_template::should_track_path("Song.logicx/Resources/stray.wav"),
        "template: stray audio inside bundle is excluded globally"
    );
    assert!(
        backup_template::should_track_path("My Set.als"),
        "template: top-level .als project file is tracked"
    );
    assert!(
        !backup_template::should_track_path("notes.txt"),
        "template: unrelated file is not tracked"
    );
    assert_eq!(
        backup_template::artifact_for_path("Song.logicx/Alternatives/000/ProjectData"),
        "Song.logicx",
        "template: bundle-internal path maps to bundle artifact"
    );
    assert_eq!(
        backup_template::artifact_for_path("My Set.als"),
        "My Set.als",
        "template: single-file path is its own artifact"
    );

    // ---- Compressed object integrity ----
    let object_store_dir = tempfile::tempdir().unwrap();
    let object_source = object_store_dir
        .path()
        .join("source.bin")
        .to_string_lossy()
        .to_string();
    let object_restore = object_store_dir
        .path()
        .join("restored.bin")
        .to_string_lossy()
        .to_string();
    let object_store = ObjectStore::new(
        object_store_dir
            .path()
            .join(".musit")
            .to_string_lossy()
            .to_string(),
    );
    assert!(
        object_store.init() && write_file(&object_source, b"object contents"),
        "object store: fixture initialized"
    );
    let object_hash = object_store
        .store_file(&object_source)
        .expect("object store: compressed object created");
    assert!(
        write_file(
            &object_store.object_path_for_hash(&object_hash).unwrap(),
            b"corrupt"
        ),
        "object store: compressed object corrupted for recovery test"
    );
    assert_eq!(
        object_store.store_file(&object_source).as_deref(),
        Some(object_hash.as_str()),
        "object store: corrupt existing object repaired"
    );
    assert!(
        object_store.extract_object(&object_hash, &object_restore)
            && read_file(&object_restore) == b"object contents",
        "object store: repaired object restores correctly"
    );

    // ---- Loose-files layout discovery ----
    let loose_projects_dir = tempfile::tempdir().unwrap();
    let loose_root = loose_projects_dir.path().to_string_lossy().to_string();
    let nested_flp = format!("{loose_root}/Nested Song/Nested Song.flp");
    let ignored_logic_bundle_file =
        format!("{loose_root}/Bundled Song.logicx/Alternatives/000/ProjectData");
    assert!(
        write_file(&nested_flp, b"fake flp")
            && write_file(&ignored_logic_bundle_file, b"fake bundled project"),
        "files layout: loose and bundled fixtures created"
    );

    let loose_projects = project_discovery::discover_all(
        &loose_root,
        None,
        None,
        None,
        20,
        ProjectsFolderLayout::Files,
    );
    assert_eq!(
        loose_projects.len(),
        1,
        "files layout: recursively finds only naked project files"
    );
    assert_eq!(
        loose_projects[0].primary_project_file, "Nested Song.flp",
        "files layout: project entry uses matching file extension"
    );

    // ---- Nested-project routing, migration, UI refresh, and restart ----
    let routing_projects_dir = tempfile::tempdir().unwrap();
    let routing_root = routing_projects_dir.path().to_string_lossy().to_string();
    let root_project_file = format!("{routing_root}/Root.flp");
    let legacy_project_root = format!("{routing_root}/Legacy Project");
    let legacy_project_file = format!("{legacy_project_root}/Legacy Project.flp");
    assert!(
        write_file(&root_project_file, b"root project")
            && write_file(&legacy_project_file, b"legacy v1"),
        "rediscovery routing: fixtures created"
    );

    // Reproduce history written by the old ancestor-routing bug.
    let mut ancestor_snapshots = SnapshotService::new();
    assert!(
        ancestor_snapshots.set_project_root(&routing_root)
            && ancestor_snapshots
                .snapshot_path_now(&legacy_project_file, "Legacy Project/Legacy Project.flp")
            && write_file(&legacy_project_file, b"legacy v2")
            && ancestor_snapshots
                .snapshot_path_now(&legacy_project_file, "Legacy Project/Legacy Project.flp"),
        "rediscovery migration: ancestor history fixture created"
    );

    let ancestor_log = format!("{routing_root}/.musit/versions/log.jsonl");
    assert!(
        file_contains(&ancestor_log, b"Legacy Project/Legacy Project.flp"),
        "rediscovery migration: history starts under ancestor root"
    );

    {
        let mut routing_backend = AppBackend::with_platform(test_platform());
        let mut projects_changed_count = 0usize;
        routing_backend.confirm_projects_folder(&routing_root, "Bundles");

        assert!(
            wait_for(&mut routing_backend, WAIT, |backend| {
                projects_changed_count += backend
                    .take_events()
                    .iter()
                    .filter(|e| matches!(e, musit_core::backend::BackendEvent::ProjectsChanged))
                    .count();
                !backend.is_scanning_projects()
                    && backend.projects().len() == 2
                    && backend.status_message().starts_with("Monitoring:")
            }),
            "rediscovery migration: initial monitoring ready"
        );

        let legacy_index = project_index_by_file(&routing_backend, "Legacy Project.flp");
        assert!(
            legacy_index >= 0
                && routing_backend.get_project_versions(legacy_index).len() == 2,
            "rediscovery migration: ancestor versions moved into project"
        );
        assert!(
            !file_contains(&ancestor_log, b"Legacy Project/Legacy Project.flp"),
            "rediscovery migration: ancestor log no longer owns project history"
        );

        let changes_before_new_project = projects_changed_count;
        let new_project_root = format!("{routing_root}/New Project");
        let new_project_file = format!("{new_project_root}/New Project.flp");
        assert!(
            write_file(&new_project_file, b"new v1"),
            "rediscovery routing: new nested project created"
        );

        assert!(
            wait_for(&mut routing_backend, WAIT, |backend| {
                projects_changed_count += backend
                    .take_events()
                    .iter()
                    .filter(|e| matches!(e, musit_core::backend::BackendEvent::ProjectsChanged))
                    .count();
                project_index_by_file(backend, "New Project.flp") >= 0
            }),
            "rediscovery routing: new project initialized"
        );
        assert!(
            projects_changed_count > changes_before_new_project,
            "rediscovery UI: main projects model refreshed after initialization"
        );

        let mut new_project_index = project_index_by_file(&routing_backend, "New Project.flp");
        assert!(
            new_project_index >= 0
                && routing_backend
                    .get_project_versions(new_project_index)
                    .len()
                    == 1,
            "rediscovery routing: initialization creates one baseline"
        );

        assert!(
            write_file(&new_project_file, b"new v2"),
            "rediscovery routing: nested project save written"
        );
        assert!(
            wait_for(&mut routing_backend, WAIT, |backend| {
                new_project_index = project_index_by_file(backend, "New Project.flp");
                new_project_index >= 0
                    && backend.get_project_versions(new_project_index).len() == 2
            }),
            "rediscovery routing: save recorded in nested project"
        );
        assert!(
            !file_contains(&ancestor_log, b"New Project/New Project.flp"),
            "rediscovery routing: ancestor project did not claim nested saves"
        );
    }

    {
        let mut restarted_backend = AppBackend::with_platform(test_platform());
        restarted_backend.load_projects_from_folder(&routing_root, "Bundles");
        assert!(
            wait_for(&mut restarted_backend, WAIT, |backend| {
                backend.take_events();
                !backend.is_scanning_projects() && backend.projects().len() == 3
            }),
            "rediscovery restart: projects loaded"
        );
        let restarted_index = project_index_by_file(&restarted_backend, "New Project.flp");
        assert!(
            restarted_index >= 0
                && restarted_backend
                    .get_project_versions(restarted_index)
                    .len()
                    == 2,
            "rediscovery restart: nested history persisted without new baseline"
        );
        restarted_backend.reset_config();
    }

    // ---- Fake Logic project ----
    let projects_dir = tempfile::tempdir().unwrap();
    let projects_root = projects_dir.path().to_string_lossy().to_string();
    let project_root = format!("{projects_root}/MySong");
    let bundle_root = format!("{project_root}/Song.logicx");
    let project_data_path = format!("{bundle_root}/Alternatives/000/ProjectData");
    let plist_path = format!("{bundle_root}/Metadata.plist");
    let audio_path = format!("{bundle_root}/Media/kick.wav");

    let created = write_file(&project_data_path, b"projectdata v1")
        && write_file(&plist_path, b"plist v1")
        && write_file(&audio_path, b"RIFFfakeaudio");
    assert!(created, "setup: fake .logicx bundle created");

    // ---- Discovery + baseline seeding ----
    let mut backend = AppBackend::with_platform(test_platform());
    backend.set_log_level("debug"); // expose suppression events in activity
    backend.load_projects_from_folder(&projects_root, "");

    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            backend.has_discovered_projects() && !backend.is_scanning_projects()
        }),
        "project scan never completed"
    );

    assert_eq!(
        backend.projects().len(),
        1,
        "discovery: exactly one project found (bundle dir, not its contents)"
    );

    backend.confirm_projects_folder(&projects_root, "Files");
    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            !backend.is_scanning_projects()
        }),
        "layout: Files scan completed"
    );
    assert!(
        !backend.has_discovered_projects(),
        "layout: bundle hidden in Files mode"
    );

    backend.confirm_projects_folder(&projects_root, "Bundles");
    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            backend.has_discovered_projects() && !backend.is_scanning_projects()
        }),
        "layout: Bundles rescan completed with projects"
    );
    assert_eq!(
        backend.projects().len(),
        1,
        "layout: bundle project restored after rescan"
    );

    backend.confirm_projects_folder(&projects_root, "Files");
    backend.confirm_projects_folder(&projects_root, "Bundles");
    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            backend.has_discovered_projects() && !backend.is_scanning_projects()
        }),
        "layout: rapid switch back to Bundles still discovers projects"
    );

    let file_url = format!("file://{projects_root}");
    backend.confirm_projects_folder(&file_url, "Files");
    backend.confirm_projects_folder(&file_url, "Bundles");
    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            backend.has_discovered_projects() && !backend.is_scanning_projects()
        }),
        "layout: file URL rescan still discovers projects"
    );

    backend.reselect_projects_folder_layout("Files");
    backend.reselect_projects_folder_layout("Bundles");
    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            backend.has_discovered_projects() && !backend.is_scanning_projects()
        }),
        "layout: reselect_projects_folder_layout restores bundle projects"
    );

    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            !backend.get_project_versions(0).is_empty()
        }),
        "monitoring baseline never seeded"
    );

    let mut versions = backend.get_project_versions(0);
    assert_eq!(versions.len(), 1, "baseline: one grouped version seeded");
    {
        let v1 = version_by_id(&versions, "1").expect("baseline v1 exists");
        assert_eq!(
            v1.files.len(),
            2,
            "baseline: v1 groups both bundle files (ProjectData + plist)"
        );
    }
    assert!(
        !staging_contains_suffix(&project_root, ".wav"),
        "baseline: no audio staged"
    );

    // ---- Event-triggered rediscovery of a new project ----
    let project_root2 = format!("{projects_root}/OtherSong");
    let bundle_root2 = format!("{project_root2}/Another.logicx");
    let project_data_path2 = format!("{bundle_root2}/Alternatives/000/ProjectData");
    let plist_path2 = format!("{bundle_root2}/Metadata.plist");
    assert!(
        write_file(&project_data_path2, b"other projectdata v1")
            && write_file(&plist_path2, b"other plist v1"),
        "rediscovery: second fake .logicx bundle created"
    );

    assert!(
        wait_for(&mut backend, Duration::from_secs(15), |backend| {
            backend.take_events();
            backend.projects().len() == 2
        }),
        "rediscovery: watcher detected new project folder"
    );

    // ---- Simulated saves through the real watcher ----
    // 7 saves -> 8 versions total; versions 1..3 must lose their staged
    // copies (5 newest kept per file).
    for save in 2..=8 {
        let payload = format!("projectdata v{save}");
        let plist_payload = format!("plist v{save}");
        assert!(
            write_file(&project_data_path, payload.as_bytes())
                && write_file(&plist_path, plist_payload.as_bytes()),
            "cannot rewrite bundle files (save {save})"
        );

        let save_id = save.to_string();
        assert!(
            wait_for(&mut backend, WAIT, |backend| {
                backend.take_events();
                versions = backend.get_project_versions(0);
                version_by_id(&versions, &save_id).is_some()
            }),
            "watcher: version {save} never appeared"
        );
    }
    assert_eq!(
        versions.len(),
        8,
        "watcher: 8 grouped versions after 7 saves"
    );

    {
        let v8 = version_by_id(&versions, "8").expect("v8 exists");
        assert_eq!(
            v8.files.len(),
            2,
            "grouping: save touching 2 bundle files yields one 2-file version"
        );
        assert!(v8.is_current, "current: newest version marked current");
    }

    // ---- Compaction ----
    assert_eq!(
        count_staged_files_named(&project_root, "ProjectData"),
        backup_template::UNCOMPRESSED_RECENT_VERSIONS as usize,
        "compaction: exactly 5 staged ProjectData copies remain"
    );
    assert_eq!(
        count_staged_files_named(&project_root, "Metadata.plist"),
        backup_template::UNCOMPRESSED_RECENT_VERSIONS as usize,
        "compaction: exactly 5 staged plist copies remain"
    );

    {
        // v1's staged copies must be gone from disk (only objects remain).
        let v1 = version_by_id(&versions, "1").expect("v1 exists");
        let mut any_staged_left = false;
        for file_entry in &v1.files {
            let absolute_staged = if file_entry.staged_path.starts_with('/') {
                file_entry.staged_path.clone()
            } else {
                format!("{project_root}/{}", file_entry.staged_path)
            };
            any_staged_left = any_staged_left || Path::new(&absolute_staged).exists();
        }
        assert!(
            !v1.files.is_empty() && !any_staged_left,
            "compaction: v1 staged copies deleted"
        );
    }

    backend.set_snapshot_retention(2);
    assert_eq!(
        count_staged_files_named(&project_root, "ProjectData"),
        2,
        "retention change: existing ProjectData copies compact immediately"
    );
    assert_eq!(
        count_staged_files_named(&project_root, "Metadata.plist"),
        2,
        "retention change: existing plist copies compact immediately"
    );

    // ---- Restore of a compacted version (object-store path) ----
    backend.set_selected_project_index(0);
    let restored = backend.restore_version_by_id("1");
    assert!(restored, "restore: compacted v1 restore reported success");
    assert_eq!(
        read_file(&project_data_path),
        b"projectdata v1",
        "restore: ProjectData content back to v1"
    );
    assert_eq!(
        read_file(&plist_path),
        b"plist v1",
        "restore: plist content back to v1"
    );
    assert_eq!(
        read_file(&audio_path),
        b"RIFFfakeaudio",
        "restore: audio file untouched by restore"
    );

    versions = backend.get_project_versions(0);
    assert!(
        version_by_id(&versions, "1").is_some_and(|v| v.is_current),
        "restore: v1 now marked current"
    );

    // ---- Branching after restore ----
    // The restore rewrote both bundle files; the watcher will report those
    // changes and the snapshot service suppresses them (self-triggered).
    // Wait until both suppressions have happened, otherwise the branch save
    // below would coalesce with the restore change and be swallowed.
    let suppressed_count = |backend: &AppBackend| {
        backend
            .activity()
            .iter()
            .filter(|line| line.contains("Suppressed self-triggered update"))
            .count()
    };
    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            suppressed_count(backend) >= 2
        }),
        "restore: watcher events for restored files were suppressed"
    );

    assert!(
        write_file(&project_data_path, b"projectdata branch")
            && write_file(&plist_path, b"plist branch"),
        "cannot rewrite bundle files (branch save)"
    );

    assert!(
        wait_for(&mut backend, WAIT, |backend| {
            backend.take_events();
            versions = backend.get_project_versions(0);
            version_by_id(&versions, "1.1").is_some()
        }),
        "branch: save after restore creates version 1.1"
    );
    {
        let v11 = version_by_id(&versions, "1.1").expect("v1.1 exists");
        assert_eq!(v11.parent, "1", "branch: 1.1 has parent 1");
        assert_eq!(
            v11.files.len(),
            2,
            "branch: 1.1 groups both bundle files"
        );
    }
}
