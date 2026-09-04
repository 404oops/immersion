//! App-level config: saved projects folder, per-project notes/primary file,
//! and app settings.
//!
//! Files live under the platform's per-user data directory (org/app "musit"):
//!   macOS:   ~/Library/Application Support/musit/musit
//!   Linux:   ~/.local/share/musit/musit (respecting XDG_DATA_HOME)
//!   Windows: %APPDATA%/musit/musit

use crate::backup_template::{
    MAX_UNCOMPRESSED_RECENT_VERSIONS, MIN_UNCOMPRESSED_RECENT_VERSIONS,
    UNCOMPRESSED_RECENT_VERSIONS,
};
use crate::object_store::write_atomically;
use crate::path_cleanup::{clean_path, join_path, path_equals, path_key};

/// Registry entries are identified by folder *and* main project file: the
/// "Files" layout puts several projects in one folder, and keying on the
/// folder alone would let them overwrite each other.
fn record_key(root_path: &str, primary_file: &str) -> String {
    format!(
        "{}\u{0}{}",
        path_key(root_path),
        primary_file.to_lowercase()
    )
}

/// The key of a stored entry, whose main file may be missing in files
/// written by older versions.
fn record_key_of(obj: &Map<String, Value>) -> String {
    let root_path = obj.get("root_path").and_then(|v| v.as_str()).unwrap_or("");
    let primary = obj
        .get("primary_project_file")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    record_key(root_path, primary)
}
use crate::project_discovery::{DiscoveredProject, known_kinds};
use chrono::Utc;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub theme_hue: f64,
    pub launch_at_startup: bool,
    pub sort_mode: String,
    pub log_level: String,
    pub snapshot_retention: i32,
    pub notifications_enabled: bool,
    pub color_scheme_mode: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_hue: 280.0,
            launch_at_startup: false,
            sort_mode: String::new(),
            log_level: String::new(),
            snapshot_retention: UNCOMPRESSED_RECENT_VERSIONS,
            notifications_enabled: true,
            color_scheme_mode: String::new(),
        }
    }
}

fn now_utc_iso_ms() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[cfg(not(target_os = "windows"))]
fn home_dir() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn canonical_config_dir() -> String {
    #[cfg(target_os = "macos")]
    {
        join_path(&home_dir(), "Library/Application Support/musit/musit")
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA")
            .unwrap_or_default()
            .replace('\\', "/");
        if appdata.is_empty() {
            return String::new();
        }
        join_path(&appdata, "musit/musit")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| join_path(&home_dir(), ".local/share"));
        join_path(&base, "musit/musit")
    }
}

/// Older builds used hand-rolled per-OS paths (and Windows accidentally
/// nested an extra "musit" segment). Copy config forward once so upgrades
/// keep their saved folder and project notes.
fn migrate_legacy_config_if_needed(canonical_dir: &str) {
    if Path::new(&join_path(canonical_dir, "config.json")).exists()
        || Path::new(&join_path(canonical_dir, "projects.json")).exists()
    {
        return;
    }

    let mut legacy_dirs: Vec<String> = Vec::new();
    #[cfg(target_os = "macos")]
    {
        legacy_dirs.push(join_path(&home_dir(), "Library/Application Support/musit"));
    }
    #[cfg(target_os = "windows")]
    {
        // The oldest hand-rolled builds used GenericConfigLocation, which is
        // %LOCALAPPDATA% on Windows.
        let local = std::env::var("LOCALAPPDATA")
            .unwrap_or_default()
            .replace('\\', "/");
        if !local.is_empty() {
            legacy_dirs.push(join_path(&local, "musit"));
        }
        let roaming = std::env::var("APPDATA")
            .unwrap_or_default()
            .replace('\\', "/");
        if !roaming.is_empty() {
            legacy_dirs.push(join_path(&roaming, "musit"));
            // Buggy path from appending "musit" to AppDataLocation twice.
            legacy_dirs.push(join_path(&roaming, "musit/musit/musit"));
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        legacy_dirs.push(join_path(&home_dir(), ".config/musit"));
        legacy_dirs.push(join_path(&home_dir(), ".local/share/musit/musit"));
    }

    for legacy_dir in legacy_dirs {
        if path_equals(&legacy_dir, canonical_dir) || !Path::new(&legacy_dir).is_dir() {
            continue;
        }

        let _ = fs::create_dir_all(canonical_dir);
        for file_name in ["config.json", "projects.json"] {
            let source_path = join_path(&legacy_dir, file_name);
            let dest_path = join_path(canonical_dir, file_name);
            if Path::new(&source_path).exists() && !Path::new(&dest_path).exists() {
                let _ = fs::copy(&source_path, &dest_path);
            }
        }
        return;
    }
}

static CONFIG_DIR_OVERRIDE: once_cell::sync::Lazy<std::sync::Mutex<Option<String>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

/// Test hook: point app config at a temp directory instead of the user's
/// real `~/Library/Application Support/musit/musit`.
pub fn set_app_config_directory_override(dir: Option<String>) {
    *CONFIG_DIR_OVERRIDE.lock().expect("config override lock") = dir;
}

/// Single source of truth for where app config lives; the startup
/// self-check and reset tooling must agree with this.
pub fn app_config_directory() -> String {
    if let Some(overridden) = CONFIG_DIR_OVERRIDE
        .lock()
        .expect("config override lock")
        .clone()
    {
        return overridden;
    }
    let canonical = canonical_config_dir();
    if canonical.is_empty() {
        return String::new();
    }
    migrate_legacy_config_if_needed(&canonical);
    canonical
}

fn write_json_atomically(file_path: &str, root: &Value) -> bool {
    let encoded = serde_json::to_string_pretty(root).expect("config json serializes");
    write_atomically(file_path, encoded.as_bytes())
}

fn project_to_json(project: &DiscoveredProject, note: &str) -> Value {
    let mut counts = Map::new();
    for kind in known_kinds() {
        counts.insert(
            kind.to_display_string().to_string(),
            json!(project.type_counts.get(&kind).copied().unwrap_or(0)),
        );
    }

    let mut item = Map::new();
    item.insert("name".to_string(), json!(project.name));
    item.insert("root_path".to_string(), json!(project.root_path));
    item.insert("kind".to_string(), json!(project.kind.to_display_string()));
    item.insert(
        "primary_project_file".to_string(),
        json!(project.primary_project_file),
    );
    item.insert("project_files".to_string(), json!(project.project_files));
    item.insert(
        "project_file_count".to_string(),
        json!(project.total_project_files),
    );
    item.insert("type_counts".to_string(), Value::Object(counts));
    item.insert("last_seen_utc".to_string(), json!(now_utc_iso_ms()));
    let trimmed_note = note.trim();
    if !trimmed_note.is_empty() {
        item.insert("note".to_string(), json!(trimmed_note));
    }
    Value::Object(item)
}

fn read_projects_array(file_path: &str) -> Vec<Value> {
    let Ok(bytes) = fs::read(file_path) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    doc.get("projects")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// What the registry remembers about a project beyond what a scan finds.
#[derive(Clone, Debug, Default)]
pub struct ProjectRecord {
    pub note: String,
    pub primary_project_file: String,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectRegistry;

impl ProjectRegistry {
    pub fn data_file_path(&self) -> String {
        join_path(&app_config_directory(), "projects.json")
    }

    fn config_file_path(&self) -> String {
        join_path(&app_config_directory(), "config.json")
    }

    fn read_config_root(&self) -> Map<String, Value> {
        let Ok(bytes) = fs::read(self.config_file_path()) else {
            return Map::new();
        };
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        }
    }

    pub fn save_project(&self, project: &DiscoveredProject) -> bool {
        let dir_path = app_config_directory();
        if dir_path.is_empty() || fs::create_dir_all(&dir_path).is_err() {
            return false;
        }

        let file_path = self.data_file_path();
        let projects = read_projects_array(&file_path);

        let mut updated: Vec<Value> = Vec::new();
        let mut replaced = false;
        for value in projects {
            let obj = value.as_object().cloned().unwrap_or_default();
            let root_path = obj.get("root_path").and_then(|v| v.as_str()).unwrap_or("");
            if path_equals(root_path, &project.root_path) {
                let note = obj.get("note").and_then(|v| v.as_str()).unwrap_or("");
                updated.push(project_to_json(project, note));
                replaced = true;
            } else {
                updated.push(Value::Object(obj));
            }
        }

        if !replaced {
            updated.push(project_to_json(project, ""));
        }

        let root = json!({ "projects": updated });
        write_json_atomically(&file_path, &root)
    }

    /// Saves every project of a scan in one read-modify-write of the
    /// registry, instead of one per project.
    pub fn save_projects(&self, projects: &[DiscoveredProject]) -> bool {
        let dir_path = app_config_directory();
        if dir_path.is_empty() || fs::create_dir_all(&dir_path).is_err() {
            return false;
        }

        let file_path = self.data_file_path();
        let mut incoming: std::collections::HashMap<String, &DiscoveredProject> = projects
            .iter()
            .map(|project| {
                (
                    record_key(&project.root_path, &project.primary_project_file),
                    project,
                )
            })
            .collect();

        let mut updated: Vec<Value> = Vec::new();
        for value in read_projects_array(&file_path) {
            let obj = value.as_object().cloned().unwrap_or_default();
            match incoming.remove(&record_key_of(&obj)) {
                Some(project) => {
                    let note = obj.get("note").and_then(|v| v.as_str()).unwrap_or("");
                    updated.push(project_to_json(project, note));
                }
                None => updated.push(Value::Object(obj)),
            }
        }
        // Registry order is irrelevant to readers; keep the new entries in
        // scan order for a stable file.
        let mut new_entries: Vec<&DiscoveredProject> = incoming.into_values().collect();
        new_entries.sort_by(|left, right| left.root_path.cmp(&right.root_path));
        for project in new_entries {
            updated.push(project_to_json(project, ""));
        }

        let root = json!({ "projects": updated });
        write_json_atomically(&file_path, &root)
    }

    /// Note and remembered primary file of every saved project, keyed by
    /// `path_key` of the root, from a single read of the registry.
    pub fn load_project_records(&self) -> std::collections::HashMap<String, ProjectRecord> {
        let mut records = std::collections::HashMap::new();
        for value in read_projects_array(&self.data_file_path()) {
            let Some(obj) = value.as_object() else {
                continue;
            };
            let root_path = obj.get("root_path").and_then(|v| v.as_str()).unwrap_or("");
            if root_path.is_empty() {
                continue;
            }
            records.insert(
                record_key_of(obj),
                ProjectRecord {
                    note: obj
                        .get("note")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    primary_project_file: obj
                        .get("primary_project_file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                },
            );
        }
        records
    }

    pub fn load_project_note(&self, root_path: &str, primary_file: &str) -> String {
        let wanted = record_key(root_path, primary_file);
        for value in read_projects_array(&self.data_file_path()) {
            let Some(obj) = value.as_object() else {
                continue;
            };
            if record_key_of(obj) == wanted {
                return obj
                    .get("note")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
        }
        String::new()
    }

    pub fn save_project_note(&self, root_path: &str, primary_file: &str, note: &str) -> bool {
        self.update_project_field(root_path, primary_file, "note", note)
    }

    /// The main file remembered for a project, looked up by the project it
    /// was discovered as.
    pub fn load_project_primary_file(&self, root_path: &str, discovered_primary: &str) -> String {
        let wanted = record_key(root_path, discovered_primary);
        for value in read_projects_array(&self.data_file_path()) {
            let Some(obj) = value.as_object() else {
                continue;
            };
            if record_key_of(obj) == wanted {
                return obj
                    .get("primary_project_file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
        }
        String::new()
    }

    pub fn save_project_primary_file(
        &self,
        root_path: &str,
        discovered_primary: &str,
        primary_file: &str,
    ) -> bool {
        self.update_project_field(
            root_path,
            discovered_primary,
            "primary_project_file",
            primary_file,
        )
    }

    fn update_project_field(
        &self,
        root_path: &str,
        primary_file: &str,
        field: &str,
        value: &str,
    ) -> bool {
        let dir_path = app_config_directory();
        if dir_path.is_empty() || fs::create_dir_all(&dir_path).is_err() {
            return false;
        }

        let projects = read_projects_array(&self.data_file_path());

        let trimmed = value.trim();
        let mut updated = false;
        let mut rewritten: Vec<Value> = Vec::new();
        let wanted = record_key(root_path, primary_file);
        for item in projects {
            let mut obj = item.as_object().cloned().unwrap_or_default();
            if record_key_of(&obj) == wanted {
                if trimmed.is_empty() {
                    obj.remove(field);
                } else {
                    obj.insert(field.to_string(), json!(trimmed));
                }
                updated = true;
            }
            rewritten.push(Value::Object(obj));
        }

        if !updated {
            let mut placeholder = Map::new();
            placeholder.insert("root_path".to_string(), json!(root_path));
            if !trimmed.is_empty() {
                placeholder.insert(field.to_string(), json!(trimmed));
            }
            rewritten.push(Value::Object(placeholder));
        }

        let root = json!({ "projects": rewritten });
        write_json_atomically(&self.data_file_path(), &root)
    }

    pub fn save_projects_folder(&self, folder_path: &str) -> bool {
        let dir_path = app_config_directory();
        if dir_path.is_empty() || fs::create_dir_all(&dir_path).is_err() {
            return false;
        }

        let mut root = self.read_config_root();
        root.insert(
            "projects_folder".to_string(),
            json!(clean_path(folder_path)),
        );
        root.insert("updated_utc".to_string(), json!(now_utc_iso_ms()));

        write_json_atomically(&self.config_file_path(), &Value::Object(root))
    }

    pub fn load_projects_folder(&self) -> String {
        let root = self.read_config_root();
        let saved_folder = root
            .get("projects_folder")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if saved_folder.is_empty() {
            return String::new();
        }

        let clean_folder = clean_path(saved_folder);
        if !Path::new(&clean_folder).is_dir() {
            return String::new();
        }
        clean_folder
    }

    /// Persists the full projects-folder list (`projects_folders`), keeping
    /// the legacy single `projects_folder` key set to the first entry so
    /// older builds still find a folder.
    pub fn save_projects_folders(&self, folder_paths: &[String]) -> bool {
        let dir_path = app_config_directory();
        if dir_path.is_empty() || fs::create_dir_all(&dir_path).is_err() {
            return false;
        }

        let cleaned: Vec<String> = folder_paths.iter().map(|p| clean_path(p)).collect();
        let mut root = self.read_config_root();
        root.insert("projects_folders".to_string(), json!(cleaned));
        match cleaned.first() {
            Some(first) => {
                root.insert("projects_folder".to_string(), json!(first));
            }
            None => {
                root.remove("projects_folder");
            }
        }
        root.insert("updated_utc".to_string(), json!(now_utc_iso_ms()));

        write_json_atomically(&self.config_file_path(), &Value::Object(root))
    }

    /// Custom display names for projects-folder tabs, keyed by folder path.
    pub fn load_projects_folder_names(&self) -> std::collections::HashMap<String, String> {
        let root = self.read_config_root();
        let mut names = std::collections::HashMap::new();
        if let Some(map) = root
            .get("projects_folder_names")
            .and_then(|v| v.as_object())
        {
            for (path, value) in map {
                if let Some(name) = value.as_str() {
                    if !name.trim().is_empty() {
                        names.insert(clean_path(path), name.trim().to_string());
                    }
                }
            }
        }
        names
    }

    /// Persists (or clears, when `name` is empty) a folder tab's custom name.
    pub fn save_projects_folder_name(&self, folder_path: &str, name: &str) -> bool {
        let dir_path = app_config_directory();
        if dir_path.is_empty() || fs::create_dir_all(&dir_path).is_err() {
            return false;
        }

        let clean = clean_path(folder_path);
        let mut root = self.read_config_root();
        let mut names = root
            .get("projects_folder_names")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let trimmed = name.trim();
        if trimmed.is_empty() {
            names.remove(&clean);
        } else {
            names.insert(clean, json!(trimmed));
        }
        root.insert("projects_folder_names".to_string(), Value::Object(names));
        root.insert("updated_utc".to_string(), json!(now_utc_iso_ms()));

        write_json_atomically(&self.config_file_path(), &Value::Object(root))
    }

    /// Loads the saved projects-folder list; falls back to the legacy single
    /// `projects_folder` key. Folders that no longer exist are dropped.
    pub fn load_projects_folders(&self) -> Vec<String> {
        let root = self.read_config_root();
        let mut folders: Vec<String> = Vec::new();
        if let Some(list) = root.get("projects_folders").and_then(|v| v.as_array()) {
            for value in list {
                let Some(path) = value.as_str() else { continue };
                let clean = clean_path(path);
                if !clean.is_empty() && Path::new(&clean).is_dir() {
                    folders.push(clean);
                }
            }
            return folders;
        }

        let legacy = self.load_projects_folder();
        if !legacy.is_empty() {
            folders.push(legacy);
        }
        folders
    }

    pub fn load_app_settings(&self) -> AppSettings {
        let mut settings = AppSettings::default();
        let root = self.read_config_root();
        if let Some(value) = root.get("theme_hue").and_then(|v| v.as_f64()) {
            settings.theme_hue = value;
        }
        if let Some(value) = root.get("launch_at_startup").and_then(|v| v.as_bool()) {
            settings.launch_at_startup = value;
        }
        if let Some(value) = root.get("sort_mode").and_then(|v| v.as_str()) {
            settings.sort_mode = value.to_string();
        }
        if let Some(value) = root.get("log_level").and_then(|v| v.as_str()) {
            settings.log_level = value.to_string();
        }
        if let Some(value) = root.get("snapshot_retention").and_then(|v| v.as_i64()) {
            settings.snapshot_retention = (value as i32).clamp(
                MIN_UNCOMPRESSED_RECENT_VERSIONS,
                MAX_UNCOMPRESSED_RECENT_VERSIONS,
            );
        }
        if let Some(value) = root.get("notifications_enabled").and_then(|v| v.as_bool()) {
            settings.notifications_enabled = value;
        }
        if let Some(value) = root.get("color_scheme_mode").and_then(|v| v.as_str()) {
            settings.color_scheme_mode = value.to_string();
        }
        settings
    }

    pub fn save_app_settings(&self, settings: &AppSettings) -> bool {
        let dir_path = app_config_directory();
        if dir_path.is_empty() || fs::create_dir_all(&dir_path).is_err() {
            return false;
        }

        let mut root = self.read_config_root();
        root.insert("theme_hue".to_string(), json!(settings.theme_hue));
        root.insert(
            "launch_at_startup".to_string(),
            json!(settings.launch_at_startup),
        );
        if !settings.sort_mode.is_empty() {
            root.insert("sort_mode".to_string(), json!(settings.sort_mode));
        }
        if !settings.log_level.is_empty() {
            root.insert("log_level".to_string(), json!(settings.log_level));
        }
        root.insert(
            "snapshot_retention".to_string(),
            json!(settings.snapshot_retention),
        );
        root.insert(
            "notifications_enabled".to_string(),
            json!(settings.notifications_enabled),
        );
        if !settings.color_scheme_mode.is_empty() {
            root.insert(
                "color_scheme_mode".to_string(),
                json!(settings.color_scheme_mode),
            );
        }
        root.insert("updated_utc".to_string(), json!(now_utc_iso_ms()));

        write_json_atomically(&self.config_file_path(), &Value::Object(root))
    }
}
