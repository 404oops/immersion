//! Application backend: orchestrates discovery, monitoring, versions,
//! restore/delete, notes, settings and the activity log.
//!
//! Port of `qt-legacy/src/ui/QmlBackend.{h,cpp}` minus the QML property
//! plumbing. Qt signals become [`BackendEvent`]s drained via
//! [`AppBackend::take_events`]; queued cross-thread signals become an
//! internal channel drained by [`AppBackend::process_pending`], which the
//! host (UI shell or test) pumps on its own cadence.

use crate::backup_template::{
    self, MAX_UNCOMPRESSED_RECENT_VERSIONS, MIN_UNCOMPRESSED_RECENT_VERSIONS,
    UNCOMPRESSED_RECENT_VERSIONS,
};
use crate::file_event::{FileEvent, FileEventType};
use crate::folder_settings::{self, ProjectsFolderLayout};
use crate::metadata_store::artifact_of_log_line;
use crate::object_store::ObjectStore;
use crate::path_cleanup::{
    artifact_equals, file_name, join_path, normalize_absolute_path, normalize_folder_path,
    parent_path, path_compare_case_insensitive, path_equals, path_is_under_root, path_key,
    relative_file_path, remove_empty_parent_dirs,
};
use crate::project_discovery::{self, DiscoveredProject};
use crate::project_registry::{app_config_directory, AppSettings, ProjectRegistry};
use crate::snapshot_service::{SnapshotNotice, SnapshotService};
use crate::version_id;
use crate::watcher::HybridFileWatcher;
use chrono::{DateTime, Local};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

const MAX_ACTIVITY_LINES: usize = 2000;
const THEME_HUE_PERSIST_DELAY: Duration = Duration::from_millis(400);

// ---- AppStrings (port of qt-legacy/src/ui/AppStrings.h) ------------------

pub mod strings {
    pub const STATUS_SELECT_PROJECTS_FOLDER: &str =
        "Please select the folder where all of your project files sit.";
    pub const SORT_NAME: &str = "Name";
    pub const SORT_LAST_OPENED: &str = "Last Opened";
    pub const LOG_LEVEL_INFO: &str = "Info";
    pub const LOG_LEVEL_DEBUG: &str = "Debug";
    pub const COLOR_SCHEME_SYSTEM: &str = "System";
    pub const COLOR_SCHEME_LIGHT: &str = "Light";
    pub const COLOR_SCHEME_DARK: &str = "Dark";
    pub const MUSIT_VERSION_LOG_RELATIVE_PATH: &str = ".musit/versions/log.jsonl";
    pub const MUSIT_STAGING_RELATIVE_PATH: &str = ".musit/staging";
}

// ---- Public value types ---------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortMode {
    Name,
    LastOpened,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Debug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSchemeMode {
    System,
    Light,
    Dark,
}

/// One row of the visible project list (the QVariantMap the QML used).
#[derive(Clone, Debug)]
pub struct ProjectListItem {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub file: String,
    pub last_opened: String,
}

#[derive(Clone, Debug, Default)]
pub struct VersionFileEntry {
    pub path: String,
    pub staged_path: String,
    pub object_hash: String,
}

#[derive(Clone, Debug, Default)]
pub struct VersionEntry {
    pub id: String,
    pub label: String,
    pub full_label: String,
    pub timestamp: String,
    pub note: String,
    pub parent: String,
    pub is_current: bool,
    pub files: Vec<VersionFileEntry>,
}

/// Version graph node: a version entry plus layout data.
#[derive(Clone, Debug, Default)]
pub struct VersionGraphNode {
    pub version: VersionEntry,
    pub parent_id: String,
    pub depth: i32,
    pub row: i32,
    pub x: f32,
    pub y: f32,
}

/// The Qt signals, as drainable events for the host shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendEvent {
    StatusMessageChanged,
    ProjectsChanged,
    ProjectsFolderChanged,
    IsScanningProjectsChanged,
    ActivityChanged,
    LogLevelChanged,
    SearchTextChanged,
    SortModeChanged,
    SelectedProjectIndexChanged,
    SelectedProjectVersionGraphChanged,
    SelectedProjectNoteChanged,
    SelectedProjectFilesChanged,
    SelectedProjectPrimaryFileChanged,
    ProjectsFolderLayoutChanged,
    PendingProjectsFolderSetupChanged,
    ProjectsFolderScanFinished(bool),
    ThemeHueChanged,
    LaunchAtStartupChanged,
    SnapshotRetentionChanged,
    NotificationsEnabledChanged,
    ColorSchemeModeChanged,
    ConfigReset,
    ProjectSaveRecorded {
        project_name: String,
        version_label: String,
        relative_path: String,
    },
}

/// Host-provided platform hooks (PlatformAgent in the Qt build).
pub struct PlatformHooks {
    pub launch_at_startup_supported: bool,
    pub set_launch_at_startup: Box<dyn Fn(bool) + Send>,
    /// Opens a file or folder with the OS default handler. Returns success.
    pub open_path: Box<dyn Fn(&str) -> bool + Send>,
}

impl Default for PlatformHooks {
    fn default() -> Self {
        Self {
            launch_at_startup_supported: false,
            set_launch_at_startup: Box::new(|_| {}),
            open_path: Box::new(default_open_path),
        }
    }
}

fn default_open_path(path: &str) -> bool {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(path).status();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", path])
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(path).status();
    result.map(|s| s.success()).unwrap_or(false)
}

// ---- Internal messages (queued connections in the Qt build) ---------------

enum BackendMsg {
    ScanDirectory {
        directory_path: String,
        directories_scanned: i32,
        scan_generation: i32,
    },
    ProjectsUpdated {
        partial_projects: Vec<DiscoveredProject>,
        directories_scanned: i32,
        folder_path: String,
        scan_generation: i32,
    },
    ScanCompleted {
        projects: Vec<DiscoveredProject>,
        elapsed_ms: i64,
        directories_scanned: i32,
        folder_path: String,
        scan_generation: i32,
    },
    WatcherEvent(FileEvent),
    WatcherScanLog {
        scan_kind: String,
        root_path: String,
        item_count: usize,
        elapsed_ms: u128,
    },
    Snapshot {
        project_root: String,
        notice: SnapshotNotice,
    },
}

// ---- Helpers ---------------------------------------------------------------

fn layout_display_name(layout: ProjectsFolderLayout) -> &'static str {
    match layout {
        ProjectsFolderLayout::Files => "Files",
        ProjectsFolderLayout::Bundles => "Bundles",
    }
}

fn layout_from_display_name(value: &str) -> ProjectsFolderLayout {
    if value.eq_ignore_ascii_case("Files") {
        ProjectsFolderLayout::Files
    } else {
        ProjectsFolderLayout::Bundles
    }
}

fn project_contains_file(project: &DiscoveredProject, file_name_str: &str) -> bool {
    project
        .project_files
        .iter()
        .any(|candidate| candidate == file_name_str)
}

fn format_human_date_time(time: Option<SystemTime>) -> String {
    match time {
        Some(t) => {
            let dt: DateTime<Local> = t.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        }
        None => String::new(),
    }
}

fn now_human() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn humanize_timestamp(raw_timestamp: &str) -> String {
    if raw_timestamp.is_empty() {
        return String::new();
    }
    match DateTime::parse_from_rfc3339(raw_timestamp) {
        Ok(parsed) => parsed
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        Err(_) => raw_timestamp.to_string(),
    }
}

fn version_label_from_id(version_id: &str) -> String {
    if version_id.is_empty() {
        return String::new();
    }
    if version_id.contains('.') {
        let last = version_id.rsplit('.').next().unwrap_or("");
        format!(".{last}")
    } else {
        format!("v{version_id}")
    }
}

fn files_are_identical(left_path: &str, right_path: &str) -> bool {
    let (Ok(left_meta), Ok(right_meta)) = (fs::metadata(left_path), fs::metadata(right_path))
    else {
        return false;
    };
    if left_meta.len() != right_meta.len() {
        return false;
    }

    let (Ok(mut left), Ok(mut right)) = (fs::File::open(left_path), fs::File::open(right_path))
    else {
        return false;
    };

    const CHUNK: usize = 256 * 1024;
    let mut left_buf = vec![0u8; CHUNK];
    let mut right_buf = vec![0u8; CHUNK];
    loop {
        let Ok(l) = left.read(&mut left_buf) else {
            return false;
        };
        let Ok(r) = right.read(&mut right_buf) else {
            return false;
        };
        if l != r || left_buf[..l] != right_buf[..r] {
            return false;
        }
        if l == 0 {
            return true;
        }
    }
}

fn sha256_file_hex(file_path: &str) -> String {
    let Ok(mut file) = fs::File::open(file_path) else {
        return String::new();
    };
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(_) => return String::new(),
        }
    }
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn resolve_staged_path(project_root: &str, staged_path: &str) -> String {
    if staged_path.is_empty() {
        return String::new();
    }
    if staged_path.starts_with('/') || staged_path.chars().nth(1) == Some(':') {
        return staged_path.to_string();
    }
    join_path(project_root, staged_path)
}

fn write_lines_atomically(file_path: &str, lines: &[Vec<u8>]) -> bool {
    let mut joined: Vec<u8> = Vec::new();
    for line in lines {
        joined.extend_from_slice(line);
    }
    crate::object_store::write_atomically(file_path, &joined)
}

fn strip_project_prefix(value: &str, project_prefix: &str) -> String {
    let normalized_value = value.replace('\\', "/");
    let normalized_prefix = project_prefix.replace('\\', "/");
    let prefix_with_slash = format!("{normalized_prefix}/");
    let matches = if path_compare_case_insensitive() {
        normalized_value
            .to_lowercase()
            .starts_with(&prefix_with_slash.to_lowercase())
    } else {
        normalized_value.starts_with(&prefix_with_slash)
    };
    if !matches {
        return String::new();
    }
    normalized_value[prefix_with_slash.len()..].to_string()
}

fn artifact_set_contains(artifacts: &HashSet<String>, candidate: &str) -> bool {
    artifacts
        .iter()
        .any(|artifact| artifact_equals(artifact, candidate))
}

fn read_log_raw_lines(log_path: &str) -> Option<Vec<Vec<u8>>> {
    let contents = fs::read(log_path).ok()?;
    let mut lines: Vec<Vec<u8>> = Vec::new();
    let mut start = 0usize;
    for (i, byte) in contents.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(contents[start..=i].to_vec());
            start = i + 1;
        }
    }
    if start < contents.len() {
        lines.push(contents[start..].to_vec());
    }
    Some(lines)
}

fn parse_log_line(raw: &[u8]) -> Option<Map<String, Value>> {
    let text = std::str::from_utf8(raw).ok()?.trim();
    if text.is_empty() {
        return None;
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Some(map),
        _ => None,
    }
}

fn json_str<'a>(obj: &'a Map<String, Value>, key: &str) -> &'a str {
    obj.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn compact_json_line(obj: &Map<String, Value>) -> Vec<u8> {
    let mut bytes =
        serde_json::to_vec(&Value::Object(obj.clone())).expect("log line serializes");
    bytes.push(b'\n');
    bytes
}

/// Older watcher routing could attach a newly-created nested project to an
/// already-known ancestor project. Move those log entries and their storage
/// into the project's own .musit directory before baseline seeding runs.
fn migrate_misrouted_history(source_root: &str, destination_root: &str) -> usize {
    if path_equals(source_root, destination_root)
        || !path_is_under_root(destination_root, source_root)
    {
        return 0;
    }

    let project_prefix = relative_file_path(source_root, destination_root);
    if project_prefix.is_empty() || project_prefix.starts_with("..") {
        return 0;
    }

    let source_log_path = join_path(source_root, strings::MUSIT_VERSION_LOG_RELATIVE_PATH);
    let Some(source_lines) = read_log_raw_lines(&source_log_path) else {
        return 0;
    };

    let mut source_kept_lines: Vec<Vec<u8>> = Vec::new();
    let mut migrated_lines: Vec<Vec<u8>> = Vec::new();
    let mut migrated_objects: Vec<Map<String, Value>> = Vec::new();
    let mut migrated_artifacts: HashSet<String> = HashSet::new();

    for raw_line in &source_lines {
        let Some(mut obj) = parse_log_line(raw_line) else {
            source_kept_lines.push(raw_line.clone());
            continue;
        };

        let rebased_path = strip_project_prefix(json_str(&obj, "path"), &project_prefix);
        if rebased_path.is_empty() {
            source_kept_lines.push(raw_line.clone());
            continue;
        }

        obj.insert("path".to_string(), Value::String(rebased_path));
        if obj.contains_key("artifact") {
            let rebased_artifact =
                strip_project_prefix(json_str(&obj, "artifact"), &project_prefix);
            if rebased_artifact.is_empty() {
                source_kept_lines.push(raw_line.clone());
                continue;
            }
            obj.insert("artifact".to_string(), Value::String(rebased_artifact));
        }

        let artifact = artifact_of_log_line(&obj);
        migrated_artifacts.insert(artifact);
        migrated_lines.push(compact_json_line(&obj));
        migrated_objects.push(obj);
    }

    if migrated_lines.is_empty() {
        return 0;
    }

    let destination_log_path =
        join_path(destination_root, strings::MUSIT_VERSION_LOG_RELATIVE_PATH);
    let mut destination_kept_lines: Vec<Vec<u8>> = Vec::new();
    if let Some(destination_lines) = read_log_raw_lines(&destination_log_path) {
        for raw_line in &destination_lines {
            let Some(obj) = parse_log_line(raw_line) else {
                destination_kept_lines.push(raw_line.clone());
                continue;
            };

            let artifact = artifact_of_log_line(&obj);
            if !artifact_set_contains(&migrated_artifacts, &artifact) {
                destination_kept_lines.push(raw_line.clone());
                continue;
            }

            // A restart may already have seeded v1 in the correct location.
            // It is safe to replace only that plain seed when its content is
            // already represented by the history being migrated.
            let mut duplicate_seed = json_str(&obj, "version") == "1"
                && json_str(&obj, "parent").is_empty()
                && json_str(&obj, "note").is_empty();
            if duplicate_seed {
                duplicate_seed = false;
                for migrated in &migrated_objects {
                    if artifact_equals(&artifact_of_log_line(migrated), &artifact)
                        && artifact_equals(json_str(migrated, "path"), json_str(&obj, "path"))
                        && json_str(migrated, "object") == json_str(&obj, "object")
                    {
                        duplicate_seed = true;
                        break;
                    }
                }
            }

            // Do not merge two independent version-number namespaces.
            if !duplicate_seed {
                return 0;
            }
        }
    }

    let source_musit = join_path(source_root, ".musit");
    let destination_musit = join_path(destination_root, ".musit");
    let source_objects = ObjectStore::new(source_musit.clone());
    let destination_objects = ObjectStore::new(destination_musit);
    if !destination_objects.init() {
        return 0;
    }

    for (i, obj) in migrated_objects.iter_mut().enumerate() {
        let object_hash = json_str(obj, "object").to_string();
        if !object_hash.is_empty() {
            let source_object_path = source_objects
                .object_path_for_hash(&object_hash)
                .unwrap_or_default();
            let destination_object_path = destination_objects
                .object_path_for_hash(&object_hash)
                .unwrap_or_default();
            if !Path::new(&source_object_path).exists() {
                return 0;
            }
            if !Path::new(&destination_object_path).exists() {
                if fs::create_dir_all(parent_path(&destination_object_path)).is_err()
                    || fs::copy(&source_object_path, &destination_object_path).is_err()
                {
                    return 0;
                }
            }
        }

        let stored_staged_path = json_str(obj, "staged").to_string();
        if stored_staged_path.is_empty() {
            continue;
        }

        let source_staged_path = if stored_staged_path.starts_with('/')
            || stored_staged_path.chars().nth(1) == Some(':')
        {
            stored_staged_path.clone()
        } else {
            join_path(source_root, &stored_staged_path)
        };
        let relative_to_source_staging = relative_file_path(
            &join_path(&source_musit, "staging"),
            &source_staged_path,
        );
        let stamp = relative_to_source_staging
            .split('/')
            .next()
            .unwrap_or("")
            .to_string();
        let destination_relative_staged =
            format!(".musit/staging/{stamp}/{}", json_str(obj, "path"));
        let destination_staged_path = join_path(destination_root, &destination_relative_staged);
        if Path::new(&source_staged_path).exists()
            && !Path::new(&destination_staged_path).exists()
        {
            if fs::create_dir_all(parent_path(&destination_staged_path)).is_err()
                || fs::copy(&source_staged_path, &destination_staged_path).is_err()
            {
                return 0;
            }
        }
        obj.insert(
            "staged".to_string(),
            Value::String(destination_relative_staged),
        );
        migrated_lines[i] = compact_json_line(obj);
    }

    if fs::create_dir_all(parent_path(&destination_log_path)).is_err() {
        return 0;
    }
    destination_kept_lines.extend(migrated_lines.iter().cloned());
    if !write_lines_atomically(&destination_log_path, &destination_kept_lines) {
        return 0;
    }

    // Writing the destination first makes a partial failure non-destructive:
    // at worst the source still has a duplicate copy for a future cleanup.
    if !write_lines_atomically(&source_log_path, &source_kept_lines) {
        return 0;
    }

    migrated_lines.len()
}

// ---- AppBackend -------------------------------------------------------------

pub struct AppBackend {
    // State mirrored from the Qt member list.
    status_message: String,
    projects_folder_root: String,
    pending_projects_folder_setup: String,
    projects_folder_layout: ProjectsFolderLayout,
    project_root: String,
    projects: Vec<ProjectListItem>,
    activity: Vec<String>,
    activity_all: Vec<String>,
    activity_levels: Vec<LogLevel>,
    log_level: LogLevel,
    search_text: String,
    sort_mode: SortMode,
    selected_project_index: i32,
    selected_project_note: String,
    selected_project_primary_file: String,
    theme_hue: f64,
    theme_hue_persist_deadline: Option<Instant>,
    launch_at_startup: bool,
    snapshot_retention: i32,
    notifications_enabled: bool,
    color_scheme_mode: ColorSchemeMode,

    discovered_projects: Vec<DiscoveredProject>,
    visible_project_indexes: Vec<usize>,
    last_opened_at_by_project_root: HashMap<String, SystemTime>,
    project_notes: HashMap<String, String>,
    project_primary_files: HashMap<String, String>,

    project_registry: ProjectRegistry,
    scan_cancel_flag: Option<Arc<AtomicBool>>,
    is_scanning_projects: bool,
    last_scan_status_directories: i32,
    active_scan_folder: String,
    active_scan_generation: i32,
    file_watcher: Option<HybridFileWatcher>,
    snapshot_service_by_root: HashMap<String, SnapshotService>,
    // pathKey(root) -> root as discovered (hash keys may be lowercased).
    canonical_root_by_key: HashMap<String, String>,

    platform: PlatformHooks,

    msg_tx: Sender<BackendMsg>,
    msg_rx: Receiver<BackendMsg>,
    events: Vec<BackendEvent>,
}

impl AppBackend {
    pub fn new() -> Self {
        Self::with_platform(PlatformHooks::default())
    }

    pub fn with_platform(platform: PlatformHooks) -> Self {
        let (msg_tx, msg_rx) = mpsc::channel();
        let project_registry = ProjectRegistry;

        let mut backend = Self {
            status_message: strings::STATUS_SELECT_PROJECTS_FOLDER.to_string(),
            projects_folder_root: String::new(),
            pending_projects_folder_setup: String::new(),
            projects_folder_layout: ProjectsFolderLayout::Bundles,
            project_root: String::new(),
            projects: Vec::new(),
            activity: Vec::new(),
            activity_all: Vec::new(),
            activity_levels: Vec::new(),
            log_level: LogLevel::Info,
            search_text: String::new(),
            sort_mode: SortMode::Name,
            selected_project_index: -1,
            selected_project_note: String::new(),
            selected_project_primary_file: String::new(),
            theme_hue: 280.0,
            theme_hue_persist_deadline: None,
            launch_at_startup: false,
            snapshot_retention: UNCOMPRESSED_RECENT_VERSIONS,
            notifications_enabled: true,
            color_scheme_mode: ColorSchemeMode::System,
            discovered_projects: Vec::new(),
            visible_project_indexes: Vec::new(),
            last_opened_at_by_project_root: HashMap::new(),
            project_notes: HashMap::new(),
            project_primary_files: HashMap::new(),
            project_registry,
            scan_cancel_flag: None,
            is_scanning_projects: false,
            last_scan_status_directories: 0,
            active_scan_folder: String::new(),
            active_scan_generation: 0,
            file_watcher: None,
            snapshot_service_by_root: HashMap::new(),
            canonical_root_by_key: HashMap::new(),
            platform,
            msg_tx,
            msg_rx,
            events: Vec::new(),
        };

        let saved_settings = backend.project_registry.load_app_settings();
        backend.theme_hue = saved_settings.theme_hue;
        backend.sort_mode = sort_mode_from_string(if saved_settings.sort_mode.is_empty() {
            strings::SORT_NAME
        } else {
            &saved_settings.sort_mode
        });
        backend.log_level = log_level_from_string(if saved_settings.log_level.is_empty() {
            strings::LOG_LEVEL_INFO
        } else {
            &saved_settings.log_level
        });
        backend.snapshot_retention = saved_settings.snapshot_retention.clamp(
            MIN_UNCOMPRESSED_RECENT_VERSIONS,
            MAX_UNCOMPRESSED_RECENT_VERSIONS,
        );
        backend.notifications_enabled = saved_settings.notifications_enabled;
        backend.color_scheme_mode =
            color_scheme_mode_from_string(if saved_settings.color_scheme_mode.is_empty() {
                strings::COLOR_SCHEME_SYSTEM
            } else {
                &saved_settings.color_scheme_mode
            });
        backend.launch_at_startup = saved_settings.launch_at_startup;
        if backend.platform.launch_at_startup_supported {
            (backend.platform.set_launch_at_startup)(backend.launch_at_startup);
        }

        let saved_projects_folder = backend.project_registry.load_projects_folder();
        if !saved_projects_folder.is_empty() {
            if folder_settings::has_layout_setting(&saved_projects_folder) {
                backend.load_projects_from_folder(&saved_projects_folder, "");
            } else {
                backend.pending_projects_folder_setup = saved_projects_folder;
                backend.push_event(BackendEvent::PendingProjectsFolderSetupChanged);
            }
        }

        backend.run_startup_self_check();
        backend
    }

    // ---- Event/message pumping ------------------------------------------

    pub fn take_events(&mut self) -> Vec<BackendEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn has_pending_events(&self) -> bool {
        !self.events.is_empty()
    }

    fn push_event(&mut self, event: BackendEvent) {
        self.events.push(event);
    }

    /// Drains queued cross-thread messages (discovery progress, watcher
    /// events, snapshot notices) and due timers. Call this regularly.
    pub fn process_pending(&mut self) {
        loop {
            match self.msg_rx.try_recv() {
                Ok(msg) => self.handle_msg(msg),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }

        if self
            .theme_hue_persist_deadline
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            self.theme_hue_persist_deadline = None;
            self.persist_app_settings();
            let hue = self.theme_hue;
            self.log_config_change(&format!("theme hue set to {hue:.0}"));
        }
    }

    fn handle_msg(&mut self, msg: BackendMsg) {
        match msg {
            BackendMsg::ScanDirectory {
                directory_path,
                directories_scanned,
                scan_generation,
            } => {
                if scan_generation == self.active_scan_generation {
                    self.handle_project_scan_directory(&directory_path, directories_scanned);
                }
            }
            BackendMsg::ProjectsUpdated {
                partial_projects,
                directories_scanned,
                folder_path,
                scan_generation,
            } => {
                self.handle_projects_updated(
                    partial_projects,
                    directories_scanned,
                    &folder_path,
                    scan_generation,
                );
            }
            BackendMsg::ScanCompleted {
                projects,
                elapsed_ms,
                directories_scanned,
                folder_path,
                scan_generation,
            } => {
                self.handle_project_scan_completed(
                    projects,
                    elapsed_ms,
                    directories_scanned,
                    &folder_path,
                    scan_generation,
                );
            }
            BackendMsg::WatcherEvent(event) => {
                // Resolve a newly-created nested project before an ancestor
                // project gets a chance to claim the event.
                let mut owned_by_nested_project = false;
                for project_root in self.canonical_root_by_key.values() {
                    if !path_equals(project_root, &self.projects_folder_root)
                        && path_is_under_root(&event.absolute_path, project_root)
                    {
                        owned_by_nested_project = true;
                        break;
                    }
                }
                if !owned_by_nested_project && self.try_discover_project_from_event(&event) {
                    return;
                }
                self.dispatch_file_event(&event);
            }
            BackendMsg::WatcherScanLog {
                scan_kind,
                root_path,
                item_count,
                elapsed_ms,
            } => {
                self.append_debug_activity(&format!(
                    "[{}] watcher {} scan: {} items in {} ms ({})",
                    now_human(),
                    scan_kind,
                    item_count,
                    elapsed_ms,
                    root_path
                ));
            }
            BackendMsg::Snapshot {
                project_root,
                notice,
            } => match notice {
                SnapshotNotice::Created(message) => {
                    self.append_activity(&format!("[{}] {}", now_human(), message));
                    if path_equals(&project_root, &self.project_root) {
                        self.push_event(BackendEvent::SelectedProjectVersionGraphChanged);
                    }
                }
                SnapshotNotice::SaveRecorded {
                    version_id,
                    relative_path,
                } => {
                    let project_name = file_name(&project_root).to_string();
                    self.push_event(BackendEvent::ProjectSaveRecorded {
                        project_name,
                        version_label: version_label_from_id(&version_id),
                        relative_path,
                    });
                }
                SnapshotNotice::Skipped(reason) => {
                    self.append_debug_activity(&format!("[{}] {}", now_human(), reason));
                }
                SnapshotNotice::Error(error) => {
                    self.append_activity(&format!("[{}] {}", now_human(), error));
                }
            },
        }
    }

    // ---- Simple accessors -------------------------------------------------

    pub fn status_message(&self) -> &str {
        &self.status_message
    }

    pub fn projects(&self) -> &[ProjectListItem] {
        &self.projects
    }

    pub fn has_discovered_projects(&self) -> bool {
        !self.discovered_projects.is_empty()
    }

    pub fn has_projects_folder(&self) -> bool {
        !self.projects_folder_root.is_empty()
    }

    pub fn projects_folder_path(&self) -> &str {
        &self.projects_folder_root
    }

    pub fn is_scanning_projects(&self) -> bool {
        self.is_scanning_projects
    }

    pub fn activity(&self) -> &[String] {
        &self.activity
    }

    pub fn theme_hue(&self) -> f64 {
        self.theme_hue
    }

    pub fn set_theme_hue(&mut self, value: f64) {
        let normalized = value % 360.0;
        let wrapped = if normalized < 0.0 {
            normalized + 360.0
        } else {
            normalized
        };
        if (self.theme_hue - wrapped).abs() < 1e-9 {
            return;
        }

        self.theme_hue = wrapped;
        self.push_event(BackendEvent::ThemeHueChanged);
        self.theme_hue_persist_deadline = Some(Instant::now() + THEME_HUE_PERSIST_DELAY);
    }

    pub fn launch_at_startup(&self) -> bool {
        self.launch_at_startup
    }

    pub fn set_launch_at_startup(&mut self, value: bool) {
        if self.launch_at_startup == value {
            return;
        }

        self.launch_at_startup = value;
        (self.platform.set_launch_at_startup)(value);
        self.persist_app_settings();
        self.log_config_change(if value {
            "launch at startup enabled"
        } else {
            "launch at startup disabled"
        });
        self.push_event(BackendEvent::LaunchAtStartupChanged);
    }

    pub fn launch_at_startup_supported(&self) -> bool {
        self.platform.launch_at_startup_supported
    }

    pub fn snapshot_retention(&self) -> i32 {
        self.snapshot_retention
    }

    pub fn set_snapshot_retention(&mut self, value: i32) {
        let clamped = value.clamp(
            MIN_UNCOMPRESSED_RECENT_VERSIONS,
            MAX_UNCOMPRESSED_RECENT_VERSIONS,
        );
        if self.snapshot_retention == clamped {
            return;
        }

        let decreased = clamped < self.snapshot_retention;
        self.snapshot_retention = clamped;
        self.apply_snapshot_retention_to_services(decreased);
        self.persist_app_settings();
        self.log_config_change(&format!("snapshot retention set to {clamped}"));
        self.push_event(BackendEvent::SnapshotRetentionChanged);
    }

    pub fn notifications_enabled(&self) -> bool {
        self.notifications_enabled
    }

    pub fn set_notifications_enabled(&mut self, value: bool) {
        if self.notifications_enabled == value {
            return;
        }

        self.notifications_enabled = value;
        self.persist_app_settings();
        self.log_config_change(if value {
            "save notifications enabled"
        } else {
            "save notifications disabled"
        });
        self.push_event(BackendEvent::NotificationsEnabledChanged);
    }

    pub fn color_scheme_mode(&self) -> ColorSchemeMode {
        self.color_scheme_mode
    }

    pub fn color_scheme_mode_string(&self) -> &'static str {
        color_scheme_mode_to_string(self.color_scheme_mode)
    }

    pub fn set_color_scheme_mode(&mut self, value: &str) {
        let next_mode = color_scheme_mode_from_string(value);
        if next_mode == self.color_scheme_mode {
            return;
        }

        self.color_scheme_mode = next_mode;
        self.persist_app_settings();
        let mode_name = self.color_scheme_mode_string().to_string();
        self.log_config_change(&format!("appearance set to {mode_name}"));
        self.push_event(BackendEvent::ColorSchemeModeChanged);
    }

    fn apply_snapshot_retention_to_services(&mut self, compact_existing: bool) {
        let retention = self.snapshot_retention;
        for service in self.snapshot_service_by_root.values_mut() {
            service.set_uncompressed_recent_versions(retention);
            if compact_existing {
                service.compact_all_staged_copies();
            }
        }
    }

    pub fn log_level(&self) -> &'static str {
        match self.log_level {
            LogLevel::Debug => strings::LOG_LEVEL_DEBUG,
            LogLevel::Info => strings::LOG_LEVEL_INFO,
        }
    }

    pub fn set_log_level(&mut self, value: &str) {
        let next_level = log_level_from_string(value);
        if next_level == self.log_level {
            return;
        }

        self.log_level = next_level;
        self.persist_app_settings();
        let level_name = self.log_level().to_string();
        self.log_config_change(&format!("activity log level set to {level_name}"));
        self.push_event(BackendEvent::LogLevelChanged);
        self.rebuild_visible_activity();
    }

    pub fn search_text(&self) -> &str {
        &self.search_text
    }

    pub fn set_search_text(&mut self, value: &str) {
        if self.search_text == value {
            return;
        }

        self.search_text = value.to_string();
        self.push_event(BackendEvent::SearchTextChanged);
        self.rebuild_visible_projects();
    }

    pub fn sort_mode(&self) -> &'static str {
        match self.sort_mode {
            SortMode::LastOpened => strings::SORT_LAST_OPENED,
            SortMode::Name => strings::SORT_NAME,
        }
    }

    pub fn set_sort_mode(&mut self, value: &str) {
        let next_mode = sort_mode_from_string(value);
        if next_mode == self.sort_mode {
            return;
        }

        self.sort_mode = next_mode;
        self.persist_app_settings();
        let mode_name = self.sort_mode().to_string();
        self.log_config_change(&format!("default sort mode set to {mode_name}"));
        self.push_event(BackendEvent::SortModeChanged);
        self.rebuild_visible_projects();
    }

    pub fn selected_project_index(&self) -> i32 {
        self.selected_project_index
    }

    pub fn set_selected_project_index(&mut self, value: i32) {
        // No early return on an unchanged index: the visible list can have
        // been refiltered/resorted, so the same index may point at a
        // different project.
        self.apply_selection(value);
        self.push_event(BackendEvent::SelectedProjectIndexChanged);
    }

    fn apply_selection(&mut self, visible_index: i32) {
        self.selected_project_index = visible_index;
        if visible_index >= 0 && (visible_index as usize) < self.visible_project_indexes.len() {
            let project =
                &self.discovered_projects[self.visible_project_indexes[visible_index as usize]];
            self.project_root = project.root_path.clone();
            self.selected_project_note = self
                .project_notes
                .get(&project.root_path)
                .cloned()
                .unwrap_or_default();
            self.selected_project_primary_file = project.primary_project_file.clone();
            self.status_message = format!(
                "Selected: {} ({})",
                project.name,
                project.kind.to_display_string()
            );
            self.push_event(BackendEvent::StatusMessageChanged);
        } else {
            self.project_root.clear();
            self.selected_project_note.clear();
            self.selected_project_primary_file.clear();
        }

        self.push_event(BackendEvent::SelectedProjectNoteChanged);
        self.push_event(BackendEvent::SelectedProjectFilesChanged);
        self.push_event(BackendEvent::SelectedProjectPrimaryFileChanged);
        self.push_event(BackendEvent::SelectedProjectVersionGraphChanged);
    }

    pub fn selected_project_files(&self) -> Vec<String> {
        if self.selected_project_index < 0
            || self.selected_project_index as usize >= self.visible_project_indexes.len()
        {
            return Vec::new();
        }
        self.discovered_projects
            [self.visible_project_indexes[self.selected_project_index as usize]]
            .project_files
            .clone()
    }

    pub fn selected_project_primary_file(&self) -> &str {
        &self.selected_project_primary_file
    }

    pub fn set_selected_project_primary_file(&mut self, value: &str) {
        if self.selected_project_index < 0
            || self.selected_project_index as usize >= self.visible_project_indexes.len()
        {
            return;
        }

        let discovered_index = self.visible_project_indexes[self.selected_project_index as usize];
        let trimmed = value.trim().to_string();
        {
            let project = &self.discovered_projects[discovered_index];
            if trimmed.is_empty() || !project_contains_file(project, &trimmed) {
                return;
            }
            if project.primary_project_file == trimmed {
                return;
            }
        }

        let (root_path, project_name);
        {
            let project = &mut self.discovered_projects[discovered_index];
            project.primary_project_file = trimmed.clone();
            root_path = project.root_path.clone();
            project_name = project.name.clone();
        }
        self.project_primary_files
            .insert(root_path.clone(), trimmed.clone());
        self.selected_project_primary_file = trimmed.clone();

        if self
            .project_registry
            .save_project_primary_file(&root_path, &trimmed)
        {
            let project = self.discovered_projects[discovered_index].clone();
            self.project_registry.save_project(&project);
        }

        self.rebuild_visible_projects();
        self.push_event(BackendEvent::SelectedProjectPrimaryFileChanged);
        self.push_event(BackendEvent::SelectedProjectVersionGraphChanged);
        self.log_config_change(&format!(
            "primary project file for {project_name} set to {trimmed}"
        ));
    }

    pub fn projects_folder_layout(&self) -> &'static str {
        layout_display_name(self.projects_folder_layout)
    }

    pub fn projects_folder_layout_for_path(&self, folder_path: &str) -> &'static str {
        let clean = normalize_folder_path(folder_path);
        if clean.is_empty() {
            return layout_display_name(ProjectsFolderLayout::Bundles);
        }
        layout_display_name(folder_settings::load_layout(&clean))
    }

    pub fn display_local_path(&self, url_or_path: &str) -> String {
        normalize_folder_path(url_or_path)
    }

    pub fn pending_projects_folder_setup(&self) -> &str {
        &self.pending_projects_folder_setup
    }

    pub fn selected_project_note(&self) -> &str {
        &self.selected_project_note
    }

    pub fn set_selected_project_note(&mut self, value: &str) {
        if self.selected_project_index < 0
            || self.selected_project_index as usize >= self.visible_project_indexes.len()
        {
            return;
        }

        let project = &self.discovered_projects
            [self.visible_project_indexes[self.selected_project_index as usize]];
        let root_path = project.root_path.clone();
        let trimmed = value.trim().to_string();
        if self
            .project_notes
            .get(&root_path)
            .cloned()
            .unwrap_or_default()
            == trimmed
        {
            return;
        }

        if self.project_registry.save_project_note(&root_path, &trimmed) {
            self.project_notes.insert(root_path, trimmed.clone());
            self.selected_project_note = trimmed;
            self.push_event(BackendEvent::SelectedProjectNoteChanged);
        }
    }

    // ---- Folder selection / scanning --------------------------------------

    pub fn reselect_projects_folder_layout(&mut self, layout: &str) {
        if self.projects_folder_root.is_empty() {
            return;
        }
        let root = self.projects_folder_root.clone();
        self.confirm_projects_folder(&root, layout);
    }

    pub fn confirm_projects_folder(&mut self, folder_path: &str, layout: &str) {
        let clean = normalize_folder_path(folder_path);
        if clean.is_empty() {
            return;
        }

        let folder_layout = layout_from_display_name(layout);
        self.append_activity(&format!(
            "[{}] folder layout set to {} for {}",
            now_human(),
            layout_display_name(folder_layout),
            clean
        ));
        if !folder_settings::save_layout(&clean, folder_layout) {
            self.append_activity(&format!(
                "[{}] failed to save projects folder layout under {}",
                now_human(),
                clean
            ));
        }

        if self.pending_projects_folder_setup == clean {
            self.pending_projects_folder_setup.clear();
            self.push_event(BackendEvent::PendingProjectsFolderSetupChanged);
        }

        self.projects_folder_layout = folder_layout;
        self.push_event(BackendEvent::ProjectsFolderLayoutChanged);
        self.load_projects_from_folder(&clean, layout);
    }

    fn apply_saved_primary_file_overrides(&self, projects: &mut [DiscoveredProject]) {
        for project in projects.iter_mut() {
            let saved_primary = self
                .project_registry
                .load_project_primary_file(&project.root_path);
            if saved_primary.is_empty() || !project_contains_file(project, &saved_primary) {
                continue;
            }
            project.primary_project_file = saved_primary;
        }
    }

    pub fn load_projects_from_folder(&mut self, folder_path: &str, layout_override: &str) {
        if folder_path.is_empty() {
            return;
        }

        let clean = normalize_folder_path(folder_path);
        if clean.is_empty() {
            return;
        }

        if let Some(flag) = &self.scan_cancel_flag {
            flag.store(true, Ordering::Relaxed);
        }

        // A stale filter from a previous folder must not hide the new projects.
        if !self.search_text.is_empty() {
            self.search_text.clear();
            self.push_event(BackendEvent::SearchTextChanged);
        }

        let folder_path_changed = !path_equals(&self.projects_folder_root, &clean);
        self.projects_folder_root = clean.clone();
        if !layout_override.is_empty() {
            self.projects_folder_layout = layout_from_display_name(layout_override);
        } else {
            self.projects_folder_layout = folder_settings::load_layout(&clean);
        }
        self.active_scan_folder = clean.clone();
        self.active_scan_generation += 1;
        let scan_generation = self.active_scan_generation;
        if folder_path_changed {
            self.push_event(BackendEvent::ProjectsFolderChanged);
        }
        self.push_event(BackendEvent::ProjectsFolderLayoutChanged);
        self.log_config_change(&format!("projects folder set to {clean}"));
        self.last_scan_status_directories = 0;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.scan_cancel_flag = Some(cancel_flag.clone());

        self.stop_monitoring();
        self.discovered_projects.clear();
        self.rebuild_visible_projects();

        if !self.is_scanning_projects {
            self.set_scanning_projects(true);
        }
        self.status_message = format!("Scanning {clean} for projects...");
        self.push_event(BackendEvent::StatusMessageChanged);
        self.append_activity(&format!(
            "[{}] project scan started: {}",
            now_human(),
            clean
        ));

        // Discovery worker thread (ProjectDiscoveryScanWorker::scan port).
        let tx = self.msg_tx.clone();
        let layout = self.projects_folder_layout;
        std::thread::Builder::new()
            .name("musit-discovery".to_string())
            .spawn(move || {
                let timer = Instant::now();
                let mut directories_scanned = 0i32;
                let dir_tx = tx.clone();
                let progress_tx = tx.clone();
                let progress_folder = clean.clone();
                let mut on_dir = |directory_path: &str| {
                    directories_scanned += 1;
                    let _ = dir_tx.send(BackendMsg::ScanDirectory {
                        directory_path: directory_path.to_string(),
                        directories_scanned,
                        scan_generation,
                    });
                };
                let mut on_progress = |partial: &[DiscoveredProject], scanned: i32| {
                    let _ = progress_tx.send(BackendMsg::ProjectsUpdated {
                        partial_projects: partial.to_vec(),
                        directories_scanned: scanned,
                        folder_path: progress_folder.clone(),
                        scan_generation,
                    });
                };
                let projects = project_discovery::discover_all(
                    &clean,
                    Some(&mut on_dir),
                    Some(&cancel_flag),
                    Some(&mut on_progress),
                    20,
                    layout,
                );

                if cancel_flag.load(Ordering::Relaxed) {
                    return;
                }

                let _ = tx.send(BackendMsg::ScanCompleted {
                    projects,
                    elapsed_ms: timer.elapsed().as_millis() as i64,
                    directories_scanned,
                    folder_path: clean,
                    scan_generation,
                });
            })
            .expect("discovery thread spawns");
    }

    pub fn cancel_project_scan(&mut self) {
        if let Some(flag) = &self.scan_cancel_flag {
            flag.store(true, Ordering::Relaxed);
        }

        if !self.is_scanning_projects {
            return;
        }

        let folder = self.active_scan_folder.clone();
        self.append_activity(&format!(
            "[{}] project scan cancelled: {}",
            now_human(),
            folder
        ));
        self.set_scanning_projects(false);
    }

    fn set_scanning_projects(&mut self, scanning: bool) {
        if self.is_scanning_projects == scanning {
            return;
        }
        self.is_scanning_projects = scanning;
        self.push_event(BackendEvent::IsScanningProjectsChanged);
    }

    fn handle_project_scan_directory(&mut self, directory_path: &str, directories_scanned: i32) {
        self.append_debug_activity(&format!(
            "[{}] scan folder: {}",
            now_human(),
            directory_path
        ));

        if !self.is_scanning_projects {
            return;
        }

        self.last_scan_status_directories = directories_scanned;
        if directories_scanned == 1 || directories_scanned % 25 == 0 {
            self.status_message = format!(
                "Scanning {} ({} folders)...",
                self.active_scan_folder, directories_scanned
            );
            self.push_event(BackendEvent::StatusMessageChanged);
        }
    }

    fn handle_projects_updated(
        &mut self,
        partial_projects: Vec<DiscoveredProject>,
        directories_scanned: i32,
        folder_path: &str,
        scan_generation: i32,
    ) {
        if scan_generation != self.active_scan_generation {
            return;
        }

        if !path_equals(folder_path, &self.active_scan_folder) || !self.is_scanning_projects {
            return;
        }

        let partial_count = partial_projects.len();
        self.apply_discovered_projects(partial_projects);

        self.last_scan_status_directories = directories_scanned;
        self.status_message = if partial_count == 0 {
            format!(
                "Scanning {} ({} folders)...",
                self.active_scan_folder, directories_scanned
            )
        } else {
            format!(
                "Scanning {} ({} folders, {} projects)...",
                self.active_scan_folder, directories_scanned, partial_count
            )
        };
        self.push_event(BackendEvent::StatusMessageChanged);
    }

    fn apply_discovered_projects(&mut self, projects: Vec<DiscoveredProject>) {
        let mut merged = projects;
        self.apply_saved_primary_file_overrides(&mut merged);
        self.discovered_projects = merged;
        self.rebuild_visible_projects();
    }

    fn handle_project_scan_completed(
        &mut self,
        projects: Vec<DiscoveredProject>,
        elapsed_ms: i64,
        directories_scanned: i32,
        folder_path: &str,
        scan_generation: i32,
    ) {
        if scan_generation != self.active_scan_generation {
            return;
        }

        if !path_equals(folder_path, &self.active_scan_folder) {
            return;
        }

        if self
            .scan_cancel_flag
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return;
        }

        self.set_scanning_projects(false);
        self.append_activity(&format!(
            "[{}] project scan finished: {} folders in {} ms ({} projects)",
            now_human(),
            directories_scanned,
            elapsed_ms,
            projects.len()
        ));

        self.finish_loading_projects(projects, folder_path.to_string());
    }

    fn finish_loading_projects(&mut self, projects: Vec<DiscoveredProject>, clean_path: String) {
        self.apply_discovered_projects(projects);
        if self.discovered_projects.is_empty() {
            self.stop_monitoring();
            self.status_message = "No supported project files found.".to_string();
            self.push_event(BackendEvent::StatusMessageChanged);
            self.append_activity(&format!(
                "[{}] first-day setup found no project files under {}",
                now_human(),
                clean_path
            ));
            self.push_event(BackendEvent::ProjectsFolderScanFinished(false));
            return;
        }

        self.project_registry.save_projects_folder(&clean_path);
        self.project_notes.clear();
        self.project_primary_files.clear();
        for project in self.discovered_projects.clone() {
            self.project_registry.save_project(&project);
            self.project_notes.insert(
                project.root_path.clone(),
                self.project_registry.load_project_note(&project.root_path),
            );
            self.project_primary_files.insert(
                project.root_path.clone(),
                project.primary_project_file.clone(),
            );
        }

        self.status_message = format!(
            "First-time setup complete. Found {} project folders.",
            self.discovered_projects.len()
        );
        self.push_event(BackendEvent::StatusMessageChanged);

        self.append_activity(&format!(
            "[{}] discovered {} projects from {}",
            now_human(),
            self.discovered_projects.len(),
            clean_path
        ));

        self.start_monitoring();
        self.push_event(BackendEvent::ProjectsFolderScanFinished(true));
    }

    // ---- Opening / selecting ----------------------------------------------

    pub fn open_project(&mut self, visible_index: i32) {
        if visible_index < 0 || visible_index as usize >= self.visible_project_indexes.len() {
            return;
        }

        self.set_selected_project_index(visible_index);

        let project =
            self.discovered_projects[self.visible_project_indexes[visible_index as usize]].clone();
        if project.primary_project_file.is_empty() {
            self.append_activity(&format!(
                "[{}] open failed: no project file selected",
                now_human()
            ));
            return;
        }

        let project_file_path = join_path(&project.root_path, &project.primary_project_file);
        if !Path::new(&project_file_path).exists() {
            self.append_activity(&format!(
                "[{}] open failed: missing file {}",
                now_human(),
                project_file_path
            ));
            return;
        }

        let is_bundle = backup_template::kind_is_bundle(project.kind);
        // Bundles (.logicx/.band) are directories; if the OS has no DAW
        // association, open the project folder instead of the bundle's parent.
        let fallback_path = if is_bundle {
            project.root_path.clone()
        } else {
            parent_path(&project_file_path)
        };

        let mut opened = (self.platform.open_path)(&project_file_path);
        if !opened {
            opened = (self.platform.open_path)(&fallback_path);
            if !opened {
                self.append_activity(&format!(
                    "[{}] open failed for {}",
                    now_human(),
                    project_file_path
                ));
                return;
            }

            self.last_opened_at_by_project_root
                .insert(path_key(&project.root_path), SystemTime::now());
            if self.sort_mode == SortMode::LastOpened {
                self.rebuild_visible_projects();
            }

            self.append_activity(&format!(
                "[{}] file association unavailable; opened folder {}",
                now_human(),
                fallback_path
            ));
            return;
        }

        self.last_opened_at_by_project_root
            .insert(path_key(&project.root_path), SystemTime::now());
        self.append_activity(&format!(
            "[{}] opened {}",
            now_human(),
            project_file_path
        ));

        if self.sort_mode == SortMode::LastOpened {
            self.rebuild_visible_projects();
        }
    }

    pub fn manage_project_versions(&mut self, visible_index: i32) {
        if visible_index < 0 || visible_index as usize >= self.visible_project_indexes.len() {
            return;
        }
        self.set_selected_project_index(visible_index);
    }

    // ---- Versions -----------------------------------------------------------

    pub fn get_project_versions(&self, visible_index: i32) -> Vec<VersionEntry> {
        if visible_index < 0 || visible_index as usize >= self.visible_project_indexes.len() {
            return Vec::new();
        }

        let project =
            &self.discovered_projects[self.visible_project_indexes[visible_index as usize]];
        let log_path = join_path(&project.root_path, strings::MUSIT_VERSION_LOG_RELATIVE_PATH);

        let Some(raw_lines) = read_log_raw_lines(&log_path) else {
            return Vec::new();
        };

        // One entry per version. Bundle saves write several log lines (one
        // per internal file) sharing a version id; they are folded into one
        // entry whose "files" list holds every file of that save.
        let mut versions: Vec<VersionEntry> = Vec::new();
        let mut version_index_by_id: HashMap<String, usize> = HashMap::new();
        let mut legacy_count = 0i64;

        for raw in &raw_lines {
            let Some(obj) = parse_log_line(raw) else {
                continue;
            };

            let staged_path = json_str(&obj, "staged");
            if !artifact_equals(&artifact_of_log_line(&obj), &project.primary_project_file)
                || staged_path.is_empty()
            {
                continue;
            }

            let mut version_id = json_str(&obj, "version").to_string();
            if version_id.is_empty() {
                legacy_count += 1;
                version_id = legacy_count.to_string();
            }

            let file_entry = VersionFileEntry {
                path: json_str(&obj, "path").to_string(),
                staged_path: staged_path.to_string(),
                object_hash: json_str(&obj, "object").to_string(),
            };

            if let Some(&index) = version_index_by_id.get(&version_id) {
                let version = &mut versions[index];
                version.files.push(file_entry);
                if version.note.is_empty() {
                    version.note = json_str(&obj, "note").to_string();
                }
                continue;
            }

            let version = VersionEntry {
                id: version_id.clone(),
                label: version_label_from_id(&version_id),
                full_label: format!("v{version_id}"),
                timestamp: humanize_timestamp(json_str(&obj, "ts")),
                note: json_str(&obj, "note").to_string(),
                parent: json_str(&obj, "parent").to_string(),
                is_current: false,
                files: vec![file_entry],
            };
            version_index_by_id.insert(version_id, versions.len());
            versions.push(version);
        }

        // A version is "current" when every file it captured matches what is
        // on disk right now (by object hash, or by content against the
        // staged copy).
        let mut current_version_id = String::new();
        let mut disk_hash_by_path: HashMap<String, String> = HashMap::new();
        for version in &versions {
            let mut all_match = !version.files.is_empty();
            for file_entry in &version.files {
                let disk_path = join_path(&project.root_path, &file_entry.path);
                if !Path::new(&disk_path).exists() {
                    all_match = false;
                    break;
                }

                let disk_hash = disk_hash_by_path
                    .entry(disk_path.clone())
                    .or_insert_with(|| sha256_file_hex(&disk_path))
                    .clone();

                let mut matches = false;
                if !disk_hash.is_empty() && !file_entry.object_hash.is_empty() {
                    matches = file_entry.object_hash.eq_ignore_ascii_case(&disk_hash);
                }

                if !matches {
                    let staged = resolve_staged_path(&project.root_path, &file_entry.staged_path);
                    if !staged.is_empty() && Path::new(&staged).exists() {
                        matches = files_are_identical(&disk_path, &staged);
                    }
                }

                if !matches {
                    all_match = false;
                    break;
                }
            }

            if all_match
                && (current_version_id.is_empty()
                    || version_id::less_than(&current_version_id, &version.id))
            {
                current_version_id = version.id.clone();
            }
        }

        // If the on-disk file matches no snapshot, no version is marked
        // current. Claiming the latest one is current would mislead the user
        // into believing their working state is already versioned.
        if !current_version_id.is_empty() {
            for version in &mut versions {
                version.is_current = version.id == current_version_id;
            }
        }

        versions
    }

    pub fn selected_project_version_graph(&self) -> Vec<VersionGraphNode> {
        if self.selected_project_index < 0
            || self.selected_project_index as usize >= self.visible_project_indexes.len()
        {
            return Vec::new();
        }

        let versions = self.get_project_versions(self.selected_project_index);
        if versions.is_empty() {
            return Vec::new();
        }

        let mut node_by_id: HashMap<String, VersionEntry> = HashMap::new();
        let mut parent_of: HashMap<String, String> = HashMap::new();
        for version in &versions {
            if version.id.is_empty() {
                continue;
            }
            node_by_id.insert(version.id.clone(), version.clone());
            parent_of.insert(version.id.clone(), version.parent.clone());
        }

        let mut children_by_parent: HashMap<String, Vec<String>> = HashMap::new();
        let mut resolved_parent: HashMap<String, String> = HashMap::new();
        for (id, parent_id) in &parent_of {
            let mut parent_id = parent_id.clone();
            // Treat nodes whose parent no longer exists (e.g. deleted by
            // older app versions) as roots instead of silently dropping
            // their subtree.
            if !parent_id.is_empty() && !node_by_id.contains_key(&parent_id) {
                parent_id.clear();
            }
            resolved_parent.insert(id.clone(), parent_id.clone());
            children_by_parent.entry(parent_id).or_default().push(id.clone());
        }

        let mut graph: Vec<VersionGraphNode> = Vec::new();
        let mut row = 0i32;

        fn append_children(
            parent_id: &str,
            depth: i32,
            row: &mut i32,
            graph: &mut Vec<VersionGraphNode>,
            children_by_parent: &HashMap<String, Vec<String>>,
            node_by_id: &HashMap<String, VersionEntry>,
            resolved_parent: &HashMap<String, String>,
        ) {
            let mut children = children_by_parent
                .get(parent_id)
                .cloned()
                .unwrap_or_default();
            children.sort_by(|a, b| {
                if version_id::less_than(a, b) {
                    std::cmp::Ordering::Less
                } else if version_id::less_than(b, a) {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });
            for child_id in children {
                let Some(version) = node_by_id.get(&child_id) else {
                    continue;
                };
                graph.push(VersionGraphNode {
                    version: version.clone(),
                    parent_id: resolved_parent.get(&child_id).cloned().unwrap_or_default(),
                    depth,
                    row: *row,
                    x: 88.0 + depth as f32 * 176.0,
                    y: 72.0 + *row as f32 * 92.0,
                });
                *row += 1;
                append_children(
                    &child_id,
                    depth + 1,
                    row,
                    graph,
                    children_by_parent,
                    node_by_id,
                    resolved_parent,
                );
            }
        }

        append_children(
            "",
            0,
            &mut row,
            &mut graph,
            &children_by_parent,
            &node_by_id,
            &resolved_parent,
        );
        graph
    }

    // ---- Restore / notes / delete -------------------------------------------

    pub fn restore_version_by_id(&mut self, version_id_str: &str) -> bool {
        if self.selected_project_index < 0
            || self.selected_project_index as usize >= self.visible_project_indexes.len()
            || version_id_str.is_empty()
        {
            return false;
        }

        let project = self.discovered_projects
            [self.visible_project_indexes[self.selected_project_index as usize]]
            .clone();
        let artifact_path = join_path(&project.root_path, &project.primary_project_file);

        // If the on-disk state matches no snapshot (e.g. the watcher has not
        // seen the latest save yet), snapshot it now so the restore cannot
        // silently destroy the user's most recent work.
        {
            let pre_restore_versions = self.get_project_versions(self.selected_project_index);
            let any_current = pre_restore_versions.iter().any(|v| v.is_current);
            if !any_current && Path::new(&artifact_path).exists() {
                if let Some(service) = self
                    .snapshot_service_by_root
                    .get_mut(&path_key(&project.root_path))
                {
                    service.snapshot_path_now(&artifact_path, &project.primary_project_file);
                }
            }
        }

        let versions = self.get_project_versions(self.selected_project_index);
        let Some(selected_version) = versions.iter().find(|v| v.id == version_id_str) else {
            return false;
        };

        // A version may span several files (bundle internals saved together).
        // Materialize every file into a temp next to its destination first,
        // so nothing is touched unless the whole version is available;
        // recent versions come from staged copies, compacted ones are
        // decompressed from the object store.
        let object_store = ObjectStore::new(join_path(&project.root_path, ".musit"));

        struct PendingRestore {
            destination_path: String,
            temp_path: String,
            relative_path: String,
        }
        let mut pending: Vec<PendingRestore> = Vec::new();

        let cleanup_temps = |pending: &[PendingRestore]| {
            for item in pending {
                let _ = fs::remove_file(&item.temp_path);
            }
        };

        for file_entry in &selected_version.files {
            let relative_path = file_entry.path.clone();
            let destination_path = join_path(&project.root_path, &relative_path);

            if fs::create_dir_all(parent_path(&destination_path)).is_err() {
                cleanup_temps(&pending);
                self.append_activity(&format!(
                    "[{}] restore failed: could not create folder for {}",
                    now_human(),
                    destination_path
                ));
                return false;
            }

            let temp_path = format!("{destination_path}.musit-restore.tmp");
            let _ = fs::remove_file(&temp_path);

            let staged_path = resolve_staged_path(&project.root_path, &file_entry.staged_path);
            let object_hash = &file_entry.object_hash;

            let mut materialized = false;
            if !staged_path.is_empty() && Path::new(&staged_path).exists() {
                materialized = fs::copy(&staged_path, &temp_path).is_ok();
            }
            if !materialized && !object_hash.is_empty() {
                materialized = object_store.extract_object(object_hash, &temp_path);
            }

            if !materialized {
                let _ = fs::remove_file(&temp_path);
                cleanup_temps(&pending);
                self.append_activity(&format!(
                    "[{}] restore failed: staged snapshot missing for {} (v{})",
                    now_human(),
                    project.name,
                    version_id_str
                ));
                return false;
            }

            pending.push(PendingRestore {
                destination_path,
                temp_path,
                relative_path,
            });
        }

        if pending.is_empty() {
            return false;
        }

        // All temps are ready; swap them in.
        for item in &pending {
            if Path::new(&item.destination_path).exists()
                && fs::remove_file(&item.destination_path).is_err()
            {
                cleanup_temps(&pending);
                self.append_activity(&format!(
                    "[{}] restore failed: could not replace {}",
                    now_human(),
                    item.destination_path
                ));
                return false;
            }

            if fs::rename(&item.temp_path, &item.destination_path).is_err() {
                // The complete data is still in the temp file; try a plain
                // copy as a last resort before giving up.
                if fs::copy(&item.temp_path, &item.destination_path).is_err() {
                    self.append_activity(&format!(
                        "[{}] restore failed: copy failed from {} to {}",
                        now_human(),
                        item.temp_path,
                        item.destination_path
                    ));
                    return false;
                }
                let _ = fs::remove_file(&item.temp_path);
            }
        }

        if let Some(service) = self
            .snapshot_service_by_root
            .get_mut(&path_key(&project.root_path))
        {
            for item in &pending {
                service.suppress_next_events_for_path(&item.relative_path, 1);
            }
            service.set_branch_base_for_artifact(&project.primary_project_file, version_id_str);
        }

        self.status_message = format!("Restored {} to version {}", project.name, version_id_str);
        self.push_event(BackendEvent::StatusMessageChanged);
        self.append_activity(&format!(
            "[{}] restored {} to version {}",
            now_human(),
            project.name,
            version_id_str
        ));

        let selected = self.selected_project_index;
        self.open_project(selected);
        self.push_event(BackendEvent::SelectedProjectVersionGraphChanged);
        true
    }

    pub fn save_version_note(&mut self, version_id_str: &str, note: &str) -> bool {
        if self.selected_project_index < 0
            || self.selected_project_index as usize >= self.visible_project_indexes.len()
            || version_id_str.is_empty()
        {
            return false;
        }

        let project = &self.discovered_projects
            [self.visible_project_indexes[self.selected_project_index as usize]];
        let log_path = join_path(&project.root_path, strings::MUSIT_VERSION_LOG_RELATIVE_PATH);

        let Some(mut lines) = read_log_raw_lines(&log_path) else {
            return false;
        };

        // The note lives on the first log line of the version (a bundle
        // version spans several lines; get_project_versions reads the first
        // non-empty note).
        let primary_file = project.primary_project_file.clone();
        let mut updated = false;
        let mut count = 0i64;
        for raw_line in lines.iter_mut() {
            let Some(mut obj) = parse_log_line(raw_line) else {
                continue;
            };

            let staged_path = json_str(&obj, "staged").to_string();
            if !artifact_equals(&artifact_of_log_line(&obj), &primary_file)
                || staged_path.is_empty()
            {
                continue;
            }

            count += 1;
            let line_version = {
                let v = json_str(&obj, "version");
                if v.is_empty() {
                    count.to_string()
                } else {
                    v.to_string()
                }
            };
            if line_version != version_id_str {
                continue;
            }

            let trimmed_note = note.trim();
            if trimmed_note.is_empty() {
                obj.remove("note");
            } else {
                obj.insert("note".to_string(), Value::String(trimmed_note.to_string()));
            }

            if !obj.contains_key("version") {
                obj.insert("version".to_string(), Value::String(line_version));
            }

            *raw_line = compact_json_line(&obj);
            updated = true;
            break;
        }

        if !updated {
            return false;
        }

        if !write_lines_atomically(&log_path, &lines) {
            return false;
        }

        self.push_event(BackendEvent::SelectedProjectVersionGraphChanged);
        true
    }

    pub fn delete_version_by_id(&mut self, version_id_str: &str) -> bool {
        if self.selected_project_index < 0
            || self.selected_project_index as usize >= self.visible_project_indexes.len()
            || version_id_str.is_empty()
        {
            return false;
        }

        let project = self.discovered_projects
            [self.visible_project_indexes[self.selected_project_index as usize]]
            .clone();
        let log_path = join_path(&project.root_path, strings::MUSIT_VERSION_LOG_RELATIVE_PATH);

        let Some(original_lines) = read_log_raw_lines(&log_path) else {
            return false;
        };

        // First pass: find every line of the version (a bundle version spans
        // one line per internal file), plus its parent and object hashes.
        let mut parsed_lines: Vec<Option<Map<String, Value>>> = Vec::new();
        let mut deleted_staged_paths: Vec<String> = Vec::new();
        let mut deleted_object_hashes: HashSet<String> = HashSet::new();
        let mut deleted_line_indexes: HashSet<usize> = HashSet::new();
        let mut deleted_parent_id = String::new();
        let mut count = 0i64;

        for (i, raw_line) in original_lines.iter().enumerate() {
            let parsed = parse_log_line(raw_line);
            if let Some(obj) = &parsed {
                let obj_staged_path = json_str(obj, "staged").to_string();
                if artifact_equals(&artifact_of_log_line(obj), &project.primary_project_file)
                    && !obj_staged_path.is_empty()
                {
                    count += 1;
                    let line_version = {
                        let v = json_str(obj, "version");
                        if v.is_empty() {
                            count.to_string()
                        } else {
                            v.to_string()
                        }
                    };
                    if line_version == version_id_str {
                        deleted_staged_paths
                            .push(resolve_staged_path(&project.root_path, &obj_staged_path));
                        if deleted_parent_id.is_empty() {
                            deleted_parent_id = json_str(obj, "parent").to_string();
                        }
                        let object_hash = json_str(obj, "object");
                        if !object_hash.is_empty() {
                            deleted_object_hashes.insert(object_hash.to_lowercase());
                        }
                        deleted_line_indexes.insert(i);
                    }
                }
            }
            parsed_lines.push(parsed);
        }

        if deleted_line_indexes.is_empty() {
            return false;
        }

        // Second pass: drop the deleted lines and reparent children of the
        // deleted version onto its own parent so their subtree stays
        // reachable in the version graph.
        let mut kept_lines: Vec<Vec<u8>> = Vec::new();
        let mut still_referenced_hashes: HashSet<String> = HashSet::new();
        for (i, raw_line) in original_lines.iter().enumerate() {
            if deleted_line_indexes.contains(&i) {
                continue;
            }

            let Some(obj) = &parsed_lines[i] else {
                kept_lines.push(raw_line.clone());
                continue;
            };

            let mut obj = obj.clone();
            let mut rewritten = false;
            if artifact_equals(&artifact_of_log_line(&obj), &project.primary_project_file)
                && json_str(&obj, "parent") == version_id_str
            {
                if deleted_parent_id.is_empty() {
                    obj.remove("parent");
                } else {
                    obj.insert(
                        "parent".to_string(),
                        Value::String(deleted_parent_id.clone()),
                    );
                }
                rewritten = true;
            }

            let object_hash = json_str(&obj, "object").to_lowercase();
            if deleted_object_hashes.contains(&object_hash) {
                still_referenced_hashes.insert(object_hash);
            }

            kept_lines.push(if rewritten {
                compact_json_line(&obj)
            } else {
                raw_line.clone()
            });
        }

        if !write_lines_atomically(&log_path, &kept_lines) {
            return false;
        }

        let staged_root = join_path(&project.root_path, strings::MUSIT_STAGING_RELATIVE_PATH);
        for staged_path in &deleted_staged_paths {
            if staged_path.is_empty() {
                continue;
            }
            if Path::new(staged_path).exists() {
                let _ = fs::remove_file(staged_path);
            }
            remove_empty_parent_dirs(&parent_path(staged_path), &staged_root);
        }

        // Remove compressed objects no other version references (objects are
        // content-addressed and can be shared between versions).
        let object_store = ObjectStore::new(join_path(&project.root_path, ".musit"));
        for object_hash in &deleted_object_hashes {
            if still_referenced_hashes.contains(object_hash) {
                continue;
            }
            if let Some(object_path) = object_store.object_path_for_hash(object_hash) {
                if Path::new(&object_path).exists() {
                    let _ = fs::remove_file(&object_path);
                }
            }
        }

        self.append_activity(&format!(
            "[{}] deleted staged variation v{} for {}",
            now_human(),
            version_id_str,
            project.name
        ));
        self.push_event(BackendEvent::SelectedProjectVersionGraphChanged);
        true
    }

    // ---- Config / activity ---------------------------------------------------

    pub fn reset_config(&mut self) -> bool {
        self.cancel_project_scan();
        self.stop_monitoring();

        let config_dir = app_config_directory();
        if !config_dir.is_empty() {
            let _ = fs::remove_file(join_path(&config_dir, "config.json"));
            let _ = fs::remove_file(join_path(&config_dir, "projects.json"));
        }

        if self.platform.launch_at_startup_supported {
            (self.platform.set_launch_at_startup)(false);
        }

        self.projects_folder_root.clear();
        self.pending_projects_folder_setup.clear();
        self.projects_folder_layout = ProjectsFolderLayout::Bundles;
        self.active_scan_folder.clear();
        self.project_root.clear();
        self.discovered_projects.clear();
        self.visible_project_indexes.clear();
        self.projects.clear();
        self.activity_all.clear();
        self.activity.clear();
        self.activity_levels.clear();
        self.search_text.clear();
        self.selected_project_index = -1;
        self.selected_project_note.clear();
        self.selected_project_primary_file.clear();
        self.project_notes.clear();
        self.project_primary_files.clear();
        self.last_opened_at_by_project_root.clear();
        self.is_scanning_projects = false;

        self.theme_hue = 280.0;
        self.theme_hue_persist_deadline = None;
        self.sort_mode = SortMode::Name;
        self.log_level = LogLevel::Info;
        self.snapshot_retention = UNCOMPRESSED_RECENT_VERSIONS;
        self.notifications_enabled = true;
        self.color_scheme_mode = ColorSchemeMode::System;
        self.launch_at_startup = false;

        self.status_message = strings::STATUS_SELECT_PROJECTS_FOLDER.to_string();

        for event in [
            BackendEvent::ProjectsChanged,
            BackendEvent::ProjectsFolderChanged,
            BackendEvent::IsScanningProjectsChanged,
            BackendEvent::ActivityChanged,
            BackendEvent::StatusMessageChanged,
            BackendEvent::SearchTextChanged,
            BackendEvent::SelectedProjectIndexChanged,
            BackendEvent::SelectedProjectNoteChanged,
            BackendEvent::SelectedProjectFilesChanged,
            BackendEvent::SelectedProjectPrimaryFileChanged,
            BackendEvent::SelectedProjectVersionGraphChanged,
            BackendEvent::ProjectsFolderLayoutChanged,
            BackendEvent::PendingProjectsFolderSetupChanged,
            BackendEvent::ThemeHueChanged,
            BackendEvent::SortModeChanged,
            BackendEvent::LogLevelChanged,
            BackendEvent::SnapshotRetentionChanged,
            BackendEvent::NotificationsEnabledChanged,
            BackendEvent::ColorSchemeModeChanged,
            BackendEvent::LaunchAtStartupChanged,
            BackendEvent::ConfigReset,
        ] {
            self.push_event(event);
        }

        self.log_config_change("configuration reset to defaults");
        true
    }

    pub fn export_activity_log(&mut self, local_path: &str) -> bool {
        if local_path.is_empty() {
            return false;
        }

        let mut contents = String::new();
        for line in &self.activity_all {
            contents.push_str(line);
            contents.push('\n');
        }
        if !crate::object_store::write_atomically(local_path, contents.as_bytes()) {
            return false;
        }

        self.append_activity(&format!(
            "[{}] exported activity log to {}",
            now_human(),
            local_path
        ));
        true
    }

    pub fn default_activity_log_export_folder(&self) -> String {
        dirs::document_dir()
            .or_else(dirs::home_dir)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default()
    }

    pub fn default_activity_log_export_file(&self) -> String {
        let timestamp = Local::now().format("%Y-%m-%d %H-%M-%S").to_string();
        let file_name = format!("Immersion Logs at {timestamp}.log");
        join_path(&self.default_activity_log_export_folder(), &file_name)
    }

    fn persist_app_settings(&self) {
        let settings = AppSettings {
            theme_hue: self.theme_hue,
            launch_at_startup: self.launch_at_startup,
            sort_mode: self.sort_mode().to_string(),
            log_level: self.log_level().to_string(),
            snapshot_retention: self.snapshot_retention,
            notifications_enabled: self.notifications_enabled,
            color_scheme_mode: self.color_scheme_mode_string().to_string(),
        };
        self.project_registry.save_app_settings(&settings);
    }

    pub fn flush_pending_theme_hue_persist(&mut self) {
        if self.theme_hue_persist_deadline.is_none() {
            return;
        }
        self.theme_hue_persist_deadline = None;
        self.persist_app_settings();
    }

    fn append_activity_with_level(&mut self, line: &str, level: LogLevel) {
        self.activity_all.push(line.to_string());
        self.activity_levels.push(level);
        self.trim_activity_log();
        self.rebuild_visible_activity();
    }

    fn log_config_change(&mut self, detail: &str) {
        self.append_debug_activity(&format!("[{}] config: {}", now_human(), detail));
    }

    fn trim_activity_log(&mut self) {
        if self.activity_all.len() <= MAX_ACTIVITY_LINES {
            return;
        }

        let excess = self.activity_all.len() - MAX_ACTIVITY_LINES;
        self.activity_all.drain(..excess);
        if self.activity_levels.len() > excess {
            self.activity_levels.drain(..excess);
        } else {
            self.activity_levels.clear();
        }
    }

    fn append_activity(&mut self, line: &str) {
        self.append_activity_with_level(line, LogLevel::Info);
    }

    fn append_debug_activity(&mut self, line: &str) {
        self.append_activity_with_level(line, LogLevel::Debug);
    }

    fn rebuild_visible_activity(&mut self) {
        let mut filtered: Vec<String> = Vec::with_capacity(self.activity_all.len());
        for (i, line) in self.activity_all.iter().enumerate() {
            let level = self
                .activity_levels
                .get(i)
                .copied()
                .unwrap_or(LogLevel::Info);
            if self.log_level == LogLevel::Info && level == LogLevel::Debug {
                continue;
            }
            filtered.push(line.clone());
        }

        self.activity = filtered;
        self.push_event(BackendEvent::ActivityChanged);
    }

    fn run_startup_self_check(&mut self) {
        let mut issues: Vec<String> = Vec::new();

        // Probe the same directory the registry actually writes to.
        let app_data_dir = app_config_directory();
        if app_data_dir.is_empty() {
            issues.push("Could not resolve app config directory.".to_string());
        } else if fs::create_dir_all(&app_data_dir).is_err() {
            issues.push(format!(
                "Could not create config directory: {app_data_dir}"
            ));
        } else {
            let probe_path = join_path(&app_data_dir, ".musit_write_probe");
            if fs::write(&probe_path, b"ok").is_err() {
                issues.push(format!(
                    "Config directory is not writable: {app_data_dir}"
                ));
            } else {
                let _ = fs::remove_file(&probe_path);
            }
        }

        let saved_folder = self.project_registry.load_projects_folder();
        if !saved_folder.is_empty() {
            if !Path::new(&saved_folder).is_dir() {
                issues.push(format!(
                    "Saved projects folder no longer exists: {saved_folder}"
                ));
            } else {
                // Version storage lives under each project's .musit folder,
                // not at the top level of the folder the user picked. Verify
                // the projects folder itself is writable so per-project
                // storage can be created on first snapshot.
                let probe_path = join_path(&saved_folder, ".musit_write_probe");
                if fs::write(&probe_path, b"ok").is_err() {
                    issues.push(format!(
                        ".musit storage is not writable in: {saved_folder}"
                    ));
                } else {
                    let _ = fs::remove_file(&probe_path);
                }
            }
        }

        let platform_name = if cfg!(target_os = "macos") {
            "macOS"
        } else if cfg!(target_os = "windows") {
            "Windows"
        } else {
            "Linux"
        };

        if issues.is_empty() {
            self.append_debug_activity(&format!(
                "[{}] startup self-check passed on {}",
                now_human(),
                platform_name
            ));
            return;
        }

        let issue_count = issues.len();
        self.append_debug_activity(&format!(
            "[{}] startup self-check found {} issue(s) on {}",
            now_human(),
            issue_count,
            platform_name
        ));
    }

    // ---- Visible projects -----------------------------------------------------

    fn effective_last_opened(
        &self,
        project: &DiscoveredProject,
        allow_file_stat_fallback: bool,
    ) -> Option<SystemTime> {
        if let Some(explicit) = self
            .last_opened_at_by_project_root
            .get(&path_key(&project.root_path))
        {
            return Some(*explicit);
        }

        if !allow_file_stat_fallback || project.primary_project_file.is_empty() {
            return None;
        }

        let project_file_path = join_path(&project.root_path, &project.primary_project_file);
        fs::metadata(&project_file_path)
            .ok()
            .and_then(|m| m.modified().ok())
    }

    fn rebuild_visible_projects(&mut self) {
        let query = self.search_text.trim().to_lowercase();

        let mut visible: Vec<usize> = Vec::with_capacity(self.discovered_projects.len());
        for (i, project) in self.discovered_projects.iter().enumerate() {
            if !query.is_empty() {
                let haystack = format!(
                    "{} {} {} {}",
                    project.name,
                    project.primary_project_file,
                    project.root_path,
                    project.kind.to_display_string()
                )
                .to_lowercase();
                if !haystack.contains(&query) {
                    continue;
                }
            }
            visible.push(i);
        }

        let sort_mode = self.sort_mode;
        let is_scanning = self.is_scanning_projects;
        visible.sort_by(|&left, &right| {
            let l = &self.discovered_projects[left];
            let r = &self.discovered_projects[right];

            if sort_mode == SortMode::LastOpened {
                let l_opened = self.effective_last_opened(l, !is_scanning);
                let r_opened = self.effective_last_opened(r, !is_scanning);
                match (l_opened, r_opened) {
                    (Some(_), None) => return std::cmp::Ordering::Less,
                    (None, Some(_)) => return std::cmp::Ordering::Greater,
                    (Some(lo), Some(ro)) if lo != ro => return ro.cmp(&lo),
                    _ => {}
                }
            }

            let by_name = l.name.to_lowercase().cmp(&r.name.to_lowercase());
            if by_name != std::cmp::Ordering::Equal {
                return by_name;
            }
            l.root_path.to_lowercase().cmp(&r.root_path.to_lowercase())
        });

        self.visible_project_indexes = visible;

        // The selected index points into the visible list, so remap it
        // whenever that list changes; otherwise operations act on the wrong
        // project.
        if !self.project_root.is_empty() {
            let mut new_index = -1i32;
            for (i, &discovered_index) in self.visible_project_indexes.iter().enumerate() {
                if path_equals(
                    &self.discovered_projects[discovered_index].root_path,
                    &self.project_root,
                ) {
                    new_index = i as i32;
                    break;
                }
            }
            if new_index != self.selected_project_index {
                self.selected_project_index = new_index;
                self.push_event(BackendEvent::SelectedProjectIndexChanged);
            }
        } else if self.selected_project_index != -1 {
            self.selected_project_index = -1;
            self.push_event(BackendEvent::SelectedProjectIndexChanged);
        }

        let mut items: Vec<ProjectListItem> =
            Vec::with_capacity(self.visible_project_indexes.len());
        for &index in &self.visible_project_indexes {
            let project = &self.discovered_projects[index];
            let effective_last_opened = self.effective_last_opened(project, true);
            let parent = parent_path(&project.root_path);
            items.push(ProjectListItem {
                name: project.name.clone(),
                kind: project.kind.to_display_string().to_string(),
                path: if parent.is_empty() {
                    project.root_path.clone()
                } else {
                    parent
                },
                file: project.primary_project_file.clone(),
                last_opened: format_human_date_time(effective_last_opened),
            });
        }

        self.projects = items;
        self.push_event(BackendEvent::ProjectsChanged);
    }

    // ---- Monitoring ------------------------------------------------------------

    /// startMonitoringDeferred + advanceMonitoringInit, run synchronously.
    fn start_monitoring(&mut self) {
        if self.projects_folder_root.is_empty() || self.discovered_projects.is_empty() {
            return;
        }

        if let Some(watcher) = self.file_watcher.take() {
            watcher.stop_watching();
        }
        self.snapshot_service_by_root.clear();
        self.canonical_root_by_key.clear();

        let projects = self.discovered_projects.clone();
        let total = projects.len();
        for (i, project) in projects.iter().enumerate() {
            self.status_message = format!("Initializing versioning ({}/{})...", i + 1, total);
            self.push_event(BackendEvent::StatusMessageChanged);
            self.init_monitoring_for_project(project);
        }

        self.start_file_watcher_if_ready();
    }

    fn init_monitoring_for_project(&mut self, project: &DiscoveredProject) -> bool {
        let mut ancestor_roots: Vec<String> = vec![self.projects_folder_root.clone()];
        for candidate in &self.discovered_projects {
            if !path_equals(&candidate.root_path, &project.root_path)
                && path_is_under_root(&project.root_path, &candidate.root_path)
            {
                ancestor_roots.push(candidate.root_path.clone());
            }
        }
        ancestor_roots.dedup();
        {
            let mut seen: Vec<String> = Vec::new();
            for root in &ancestor_roots {
                if !seen.contains(root) {
                    seen.push(root.clone());
                }
            }
            ancestor_roots = seen;
        }
        ancestor_roots.sort_by(|left, right| right.len().cmp(&left.len()));
        for ancestor_root in &ancestor_roots {
            let migrated = migrate_misrouted_history(ancestor_root, &project.root_path);
            if migrated > 0 {
                self.append_activity(&format!(
                    "[{}] migrated {} snapshot entries into {}",
                    now_human(),
                    migrated,
                    project.root_path
                ));
            }
        }

        let mut snapshot_service = SnapshotService::new();
        if !snapshot_service.set_project_root(&project.root_path) {
            self.append_debug_activity(&format!(
                "[{}] failed to initialize versioning for {}",
                now_human(),
                project.root_path
            ));
            return false;
        }

        snapshot_service.set_uncompressed_recent_versions(self.snapshot_retention);

        let notice_tx = self.msg_tx.clone();
        let notice_root = project.root_path.clone();
        snapshot_service.set_notice_sink(move |notice| {
            let _ = notice_tx.send(BackendMsg::Snapshot {
                project_root: notice_root.clone(),
                notice,
            });
        });

        let root_key = path_key(&project.root_path);
        self.canonical_root_by_key
            .insert(root_key.clone(), project.root_path.clone());
        self.snapshot_service_by_root
            .insert(root_key.clone(), snapshot_service);

        let primary_file = project.primary_project_file.clone();
        if !primary_file.is_empty() {
            let service = self
                .snapshot_service_by_root
                .get_mut(&root_key)
                .expect("service just inserted");
            if !service.has_version_for_artifact(&primary_file) {
                let absolute_primary_path = join_path(&project.root_path, &primary_file);
                let seeded = service.snapshot_path_now(&absolute_primary_path, &primary_file);
                if seeded {
                    self.append_debug_activity(&format!(
                        "[{}] seeded initial version v1 for {}",
                        now_human(),
                        absolute_primary_path
                    ));
                }
            }
        }

        true
    }

    fn start_file_watcher_if_ready(&mut self) {
        if self.snapshot_service_by_root.is_empty() {
            self.status_message = "Failed to initialize versioning".to_string();
            self.push_event(BackendEvent::StatusMessageChanged);
            return;
        }

        let event_tx = self.msg_tx.clone();
        let log_tx = self.msg_tx.clone();
        let watcher = HybridFileWatcher::new(
            move |event| {
                let _ = event_tx.send(BackendMsg::WatcherEvent(event));
            },
            Some(Box::new(move |scan_kind, root_path, item_count, elapsed| {
                let _ = log_tx.send(BackendMsg::WatcherScanLog {
                    scan_kind: scan_kind.to_string(),
                    root_path: root_path.to_string(),
                    item_count,
                    elapsed_ms: elapsed,
                });
            })),
        );

        if !watcher.start_watching(&self.projects_folder_root) {
            self.status_message = "Failed to start watcher".to_string();
            self.push_event(BackendEvent::StatusMessageChanged);
            self.snapshot_service_by_root.clear();
            self.canonical_root_by_key.clear();
            return;
        }
        self.file_watcher = Some(watcher);

        self.status_message = format!("Monitoring: {}", self.projects_folder_root);
        self.push_event(BackendEvent::StatusMessageChanged);

        let folder = self.projects_folder_root.clone();
        self.append_activity(&format!(
            "[{}] monitoring started for {}",
            Local::now().format("%Y-%m-%dT%H:%M:%S"),
            folder
        ));
    }

    fn stop_monitoring(&mut self) {
        if let Some(watcher) = self.file_watcher.take() {
            watcher.stop_watching();
        }

        self.snapshot_service_by_root.clear();
        self.canonical_root_by_key.clear();
        self.append_activity(&format!(
            "[{}] monitoring stopped",
            Local::now().format("%Y-%m-%dT%H:%M:%S")
        ));
    }

    fn contains_discovered_project(&self, candidate: &DiscoveredProject) -> bool {
        for project in &self.discovered_projects {
            if !path_equals(&project.root_path, &candidate.root_path) {
                continue;
            }
            if self.projects_folder_layout == ProjectsFolderLayout::Bundles
                || artifact_equals(
                    &project.primary_project_file,
                    &candidate.primary_project_file,
                )
            {
                return true;
            }
        }
        false
    }

    fn adopt_discovered_project(&mut self, project: &DiscoveredProject) -> bool {
        if self.contains_discovered_project(project) {
            return false;
        }

        let mut adopted_projects = vec![project.clone()];
        self.apply_saved_primary_file_overrides(&mut adopted_projects);
        let adopted = adopted_projects.remove(0);

        // Initialize storage and seed/migrate history first. Publishing the
        // project afterwards guarantees the main view observes a usable
        // project.
        if !self
            .snapshot_service_by_root
            .contains_key(&path_key(&adopted.root_path))
            && !self.init_monitoring_for_project(&adopted)
        {
            return false;
        }

        self.discovered_projects.push(adopted.clone());
        self.project_registry.save_project(&adopted);
        self.project_notes.insert(
            adopted.root_path.clone(),
            self.project_registry.load_project_note(&adopted.root_path),
        );
        self.project_primary_files.insert(
            adopted.root_path.clone(),
            adopted.primary_project_file.clone(),
        );
        self.rebuild_visible_projects();

        self.append_activity(&format!(
            "[{}] new project detected: {} ({})",
            now_human(),
            adopted.name,
            adopted.root_path
        ));
        true
    }

    fn try_discover_project_from_event(&mut self, event: &FileEvent) -> bool {
        if self.is_scanning_projects || self.projects_folder_root.is_empty() {
            return false;
        }

        if !matches!(
            event.event_type,
            FileEventType::Created | FileEventType::Modified
        ) {
            return false;
        }

        let Some(discovered) = project_discovery::discover_project_for_changed_path(
            &self.projects_folder_root,
            &event.absolute_path,
            self.projects_folder_layout,
        ) else {
            return false;
        };

        if self.contains_discovered_project(&discovered) {
            return false;
        }

        self.adopt_discovered_project(&discovered)
    }

    fn dispatch_file_event(&mut self, event: &FileEvent) -> bool {
        let normalized_absolute_path = normalize_absolute_path(&event.absolute_path);

        let mut matched_root_key = String::new();
        let mut matched_length = -1i64;
        for root_key in self.snapshot_service_by_root.keys() {
            let Some(canonical_root) = self.canonical_root_by_key.get(root_key) else {
                continue;
            };
            if canonical_root.is_empty()
                || !path_is_under_root(&normalized_absolute_path, canonical_root)
            {
                continue;
            }

            if canonical_root.len() as i64 > matched_length {
                matched_root_key = root_key.clone();
                matched_length = canonical_root.len() as i64;
            }
        }

        if matched_root_key.is_empty() {
            return false;
        }

        let Some(canonical_root) = self.canonical_root_by_key.get(&matched_root_key).cloned()
        else {
            return false;
        };
        if canonical_root.is_empty() {
            return false;
        }

        let mut translated_event = event.clone();
        translated_event.relative_path =
            relative_file_path(&canonical_root, &normalized_absolute_path);

        let Some(service) = self.snapshot_service_by_root.get_mut(&matched_root_key) else {
            return false;
        };
        service.on_file_event(&translated_event);
        true
    }
}

impl Default for AppBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AppBackend {
    fn drop(&mut self) {
        self.flush_pending_theme_hue_persist();
        self.cancel_project_scan();
        if let Some(watcher) = self.file_watcher.take() {
            watcher.stop_watching();
        }
    }
}

// ---- Enum <-> string helpers -------------------------------------------------

fn sort_mode_from_string(value: &str) -> SortMode {
    if value.trim().eq_ignore_ascii_case(strings::SORT_LAST_OPENED) {
        SortMode::LastOpened
    } else {
        SortMode::Name
    }
}

fn log_level_from_string(value: &str) -> LogLevel {
    if value.trim().eq_ignore_ascii_case(strings::LOG_LEVEL_DEBUG) {
        LogLevel::Debug
    } else {
        LogLevel::Info
    }
}

fn color_scheme_mode_from_string(value: &str) -> ColorSchemeMode {
    let lowered = value.trim().to_lowercase();
    if lowered == strings::COLOR_SCHEME_LIGHT.to_lowercase() {
        ColorSchemeMode::Light
    } else if lowered == strings::COLOR_SCHEME_DARK.to_lowercase() {
        ColorSchemeMode::Dark
    } else {
        ColorSchemeMode::System
    }
}

fn color_scheme_mode_to_string(mode: ColorSchemeMode) -> &'static str {
    match mode {
        ColorSchemeMode::Light => strings::COLOR_SCHEME_LIGHT,
        ColorSchemeMode::Dark => strings::COLOR_SCHEME_DARK,
        ColorSchemeMode::System => strings::COLOR_SCHEME_SYSTEM,
    }
}
