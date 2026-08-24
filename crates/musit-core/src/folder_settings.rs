//! Per-projects-folder settings (`<folder>/.immersion/settings.json`).
//! Port of `qt-legacy/src/persistence/ProjectsFolderSettings.{h,cpp}`, plus
//! the session bits (tab name, tab colour) that let a folder come back the
//! way the user left it when it is re-added.

use crate::path_cleanup::join_path;
use chrono::Utc;
use serde_json::json;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProjectsFolderLayout {
    #[default]
    Bundles,
    Files,
}

impl ProjectsFolderLayout {
    pub fn from_str_value(value: &str) -> Self {
        if value.eq_ignore_ascii_case("files") {
            ProjectsFolderLayout::Files
        } else {
            ProjectsFolderLayout::Bundles
        }
    }

    pub fn to_str_value(self) -> &'static str {
        match self {
            ProjectsFolderLayout::Files => "files",
            ProjectsFolderLayout::Bundles => "bundles",
        }
    }
}

/// Everything a folder remembers about itself. `name`/`hue` are the session
/// bits: empty/`None` means "no choice saved yet".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderSettings {
    pub layout: ProjectsFolderLayout,
    pub name: String,
    pub hue: Option<f64>,
}

pub fn settings_directory_name() -> &'static str {
    ".immersion"
}

pub fn settings_file_path(projects_folder_path: &str) -> String {
    if projects_folder_path.is_empty() {
        return String::new();
    }
    join_path(
        projects_folder_path,
        &format!("{}/settings.json", settings_directory_name()),
    )
}

pub fn has_layout_setting(projects_folder_path: &str) -> bool {
    let file_path = settings_file_path(projects_folder_path);
    !file_path.is_empty() && Path::new(&file_path).exists()
}

pub fn load(projects_folder_path: &str) -> FolderSettings {
    let file_path = settings_file_path(projects_folder_path);
    let Ok(bytes) = fs::read(&file_path) else {
        return FolderSettings::default();
    };
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return FolderSettings::default();
    };
    FolderSettings {
        layout: ProjectsFolderLayout::from_str_value(
            doc.get("layout").and_then(|v| v.as_str()).unwrap_or(""),
        ),
        name: doc
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        // Out-of-range values (a hand-edited file) are dropped rather than
        // wrapped: the folder just falls back to the app-wide hue.
        hue: doc
            .get("hue")
            .and_then(|v| v.as_f64())
            .filter(|hue| (0.0..360.0).contains(hue)),
    }
}

pub fn load_layout(projects_folder_path: &str) -> ProjectsFolderLayout {
    load(projects_folder_path).layout
}

pub fn save(projects_folder_path: &str, settings: &FolderSettings) -> bool {
    if projects_folder_path.is_empty() {
        return false;
    }

    let settings_dir = join_path(projects_folder_path, settings_directory_name());
    if fs::create_dir_all(&settings_dir).is_err() {
        return false;
    }

    let mut root = json!({
        "layout": settings.layout.to_str_value(),
        "updated_utc": Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    });
    let map = root.as_object_mut().expect("object literal");
    if !settings.name.is_empty() {
        map.insert("name".to_string(), json!(settings.name));
    }
    if let Some(hue) = settings.hue {
        map.insert("hue".to_string(), json!((hue * 10.0).round() / 10.0));
    }

    let encoded = serde_json::to_string_pretty(&root).expect("static json serializes");
    crate::object_store::write_atomically(
        &settings_file_path(projects_folder_path),
        encoded.as_bytes(),
    )
}

/// Read-modify-write helpers: each one keeps the other saved fields intact.
pub fn save_layout(projects_folder_path: &str, layout: ProjectsFolderLayout) -> bool {
    let mut settings = load(projects_folder_path);
    settings.layout = layout;
    save(projects_folder_path, &settings)
}

pub fn save_name(projects_folder_path: &str, name: &str) -> bool {
    let mut settings = load(projects_folder_path);
    settings.name = name.trim().to_string();
    save(projects_folder_path, &settings)
}

pub fn save_hue(projects_folder_path: &str, hue: f64) -> bool {
    let mut settings = load(projects_folder_path);
    settings.hue = Some(hue);
    save(projects_folder_path, &settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_fields_round_trip_and_survive_a_layout_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_string_lossy().replace('\\', "/");

        assert!(save(
            &root,
            &FolderSettings {
                layout: ProjectsFolderLayout::Files,
                name: "Beats".to_string(),
                hue: Some(212.5),
            }
        ));

        let loaded = load(&root);
        assert_eq!(loaded.layout, ProjectsFolderLayout::Files);
        assert_eq!(loaded.name, "Beats");
        assert_eq!(loaded.hue, Some(212.5));

        // A later layout choice must not wipe the remembered session.
        assert!(save_layout(&root, ProjectsFolderLayout::Bundles));
        let loaded = load(&root);
        assert_eq!(loaded.layout, ProjectsFolderLayout::Bundles);
        assert_eq!(loaded.name, "Beats");
        assert_eq!(loaded.hue, Some(212.5));
    }

    #[test]
    fn legacy_layout_only_file_loads_with_no_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_string_lossy().replace('\\', "/");
        fs::create_dir_all(join_path(&root, settings_directory_name())).expect("mkdir");
        fs::write(settings_file_path(&root), br#"{"layout":"files"}"#).expect("write");

        let loaded = load(&root);
        assert_eq!(loaded.layout, ProjectsFolderLayout::Files);
        assert!(loaded.name.is_empty());
        assert_eq!(loaded.hue, None);
    }
}
