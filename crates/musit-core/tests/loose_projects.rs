//! Several project files can sit side by side in one folder (the "Files"
//! layout). They share a folder but are separate projects, so selecting one
//! and giving it a note must not touch its neighbours: a version delete acts
//! on whatever is selected, and a selection that silently slid to a sibling
//! would rewrite the wrong project's history.

use musit_core::backend::AppBackend;
use std::fs;
use std::time::{Duration, Instant};

fn wait_for(backend: &mut AppBackend, mut done: impl FnMut(&mut AppBackend) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        backend.process_pending();
        backend.take_events();
        if done(backend) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    backend.process_pending();
    done(backend)
}

#[test]
fn loose_projects_in_one_folder_keep_their_own_selection_and_note() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_string_lossy().to_string();
    fs::write(dir.path().join("Alpha.flp"), b"alpha v1").expect("write alpha");
    fs::write(dir.path().join("Zulu.flp"), b"zulu v1").expect("write zulu");

    let mut backend = AppBackend::new();
    backend.add_projects_folder(&root, "Files");
    assert!(
        wait_for(&mut backend, |backend| backend.projects().len() == 2),
        "both loose project files are discovered as separate projects"
    );

    backend.set_selected_project_index(1);
    backend.process_pending();
    let selected_file = backend.selected_project_primary_file().to_string();
    assert!(
        !selected_file.is_empty(),
        "the selection names a project file"
    );
    backend.set_selected_project_note("note for the second project");
    backend.process_pending();

    // Any refilter rebuilds the list; the selection must survive it intact.
    backend.set_search_text("a");
    backend.set_search_text("");
    backend.process_pending();

    assert_eq!(
        backend.selected_project_primary_file(),
        selected_file,
        "the selection stayed on the same project file"
    );
    assert_eq!(
        backend.selected_project_note(),
        "note for the second project",
        "the note stayed with the project it was written for"
    );

    let sibling = if backend.selected_project_index() == 1 {
        0
    } else {
        1
    };
    backend.set_selected_project_index(sibling);
    backend.process_pending();
    assert!(
        backend.selected_project_note().is_empty(),
        "the neighbouring project did not inherit the note"
    );
}
