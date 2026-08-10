//! Per-projects-folder settings (`<folder>/.immersion/settings.json`).
//! Port of `qt-legacy/src/persistence/ProjectsFolderSettings.{h,cpp}`.

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

pub fn load_layout(projects_folder_path: &str) -> ProjectsFolderLayout {
    let file_path = settings_file_path(projects_folder_path);
    let Ok(bytes) = fs::read(&file_path) else {
        return ProjectsFolderLayout::Bundles;
    };
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return ProjectsFolderLayout::Bundles;
    };
    ProjectsFolderLayout::from_str_value(doc.get("layout").and_then(|v| v.as_str()).unwrap_or(""))
}

pub fn save_layout(projects_folder_path: &str, layout: ProjectsFolderLayout) -> bool {
    if projects_folder_path.is_empty() {
        return false;
    }

    let settings_dir = join_path(projects_folder_path, settings_directory_name());
    if fs::create_dir_all(&settings_dir).is_err() {
        return false;
    }

    let root = json!({
        "layout": layout.to_str_value(),
        "updated_utc": Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    });

    let encoded = serde_json::to_string_pretty(&root).expect("static json serializes");
    crate::object_store::write_atomically(&settings_file_path(projects_folder_path), encoded.as_bytes())
}
