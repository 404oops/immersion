//! Application backend: orchestrates discovery, monitoring, versions,
//! restore/delete, notes, settings and the activity log.
//!
//! State changes surface as [`BackendEvent`]s drained via
//! [`AppBackend::take_events`]; cross-thread work arrives on an internal
//! channel drained by [`AppBackend::process_pending`], which the host (UI
//! shell or test) pumps on its own cadence.

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
use crate::project_registry::{
    AppSettings, DEFAULT_THEME_SATURATION, ProjectRegistry, app_config_directory,
    clamp_theme_saturation,
};
use crate::snapshot_service::SnapshotNotice;
use crate::version_id;
use crate::versioning::{self, Job as VersioningJob, Reply as VersioningReply};
use crate::watcher::HybridFileWatcher;
use chrono::{DateTime, Local};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const MAX_ACTIVITY_LINES: usize = 2000;
const THEME_HUE_PERSIST_DELAY: Duration = Duration::from_millis(400);

// ---- User-facing strings --------------------------------------------------

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

/// One row of the visible project list.
#[derive(Clone, Debug)]
pub struct ProjectListItem {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub file: String,
    pub last_opened: String,
}

/// One projects-folder tab shown in the main view.
#[derive(Clone, Debug)]
pub struct FolderTab {
    pub path: String,
    pub name: String,
    pub layout: &'static str,
    pub is_scanning: bool,
    /// Short progress suffix for the tab title ("scanning…", "queued", …);
    /// empty when the tab has nothing to report.
    pub status: String,
}

/// Per-projects-folder state: each configured folder is discovered, watched,
/// and monitored independently; the UI shows one folder at a time via tabs.
struct FolderWorkspace {
    root: String,
    layout: ProjectsFolderLayout,
    /// User-chosen tab name; empty means "use the folder's name".
    display_name: String,
    /// Theme hue remembered for this tab; `None` until one is saved.
    hue: Option<f64>,
    /// Live scan progress for the tab title (e.g. "25 folders").
    scan_progress: String,
    discovered_projects: Vec<DiscoveredProject>,
    watcher: Option<HybridFileWatcher>,
    watcher_ready: bool,
    scanning: bool,
    /// Bumped whenever this folder's monitoring is torn down, so versioning
    /// init replies from before that are recognised as stale.
    monitoring_generation: u64,
}

impl FolderWorkspace {
    fn new(root: String, layout: ProjectsFolderLayout) -> Self {
        Self {
            root,
            layout,
            display_name: String::new(),
            hue: None,
            scan_progress: String::new(),
            discovered_projects: Vec::new(),
            watcher: None,
            watcher_ready: false,
            scanning: false,
            monitoring_generation: 0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VersionFileEntry {
    pub path: String,
    pub staged_path: String,
    pub object_hash: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VersionEntry {
    pub id: String,
    pub label: String,
    pub full_label: String,
    pub timestamp: String,
    pub note: String,
    pub parent: String,
    pub is_current: bool,
    /// Compaction has removed every staged copy of this version; its content
    /// now lives only as compressed objects (restore decompresses on demand).
    pub is_compressed: bool,
    pub files: Vec<VersionFileEntry>,
}

/// Version graph node: a version plus its place in the tree. Emitted in
/// depth-first order by [`AppBackend::selected_project_version_graph`];
/// placing nodes on screen is the shell's job.
#[derive(Clone, Debug, Default)]
pub struct VersionGraphNode {
    pub version: VersionEntry,
    /// Parent version id; empty for a top-level version.
    pub parent_id: String,
    /// 0 for top-level versions, one more per branch level.
    pub depth: i32,
}

/// State-change notifications for the host shell, drained via
/// [`AppBackend::take_events`].
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
    /// A folder's discovery scan finished. Carries the folder so a dialog
    /// waiting on one folder doesn't react to another folder's background scan.
    ProjectsFolderScanFinished {
        folder: String,
        found_projects: bool,
    },
    FoldersChanged,
    ActiveFolderChanged,
    ThemeHueChanged,
    ThemeSaturationChanged,
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

/// Host-provided platform hooks.
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

// ---- Internal messages (cross-thread, drained by `process_pending`) -------

enum BackendMsg {
    ScanDirectory {
        directory_path: String,
        directories_scanned: i32,
        scan_generation: i32,
    },
    /// A scan partial is ready in `scan_partial_slot`.
    ProjectsUpdated {
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
    WatcherEvent {
        watch_root: String,
        event: FileEvent,
    },
    WatcherReady {
        root_path: String,
    },
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
    /// A reply from the versioning thread.
    Versioning(VersioningReply),
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

pub(crate) fn sha256_file_hex(file_path: &str) -> String {
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

/// Marks the version whose files match what is on disk, using `hash_of` to
/// read content hashes so each caller can bring its own cache.
pub(crate) fn mark_current_version_with(
    project: &DiscoveredProject,
    versions: &mut [VersionEntry],
    mut hash_of: impl FnMut(&str) -> String,
) {
    let mut current_version_id = String::new();
    for version in versions.iter() {
        let mut all_match = !version.files.is_empty();
        for file_entry in &version.files {
            let disk_path = join_path(&project.root_path, &file_entry.path);
            if !Path::new(&disk_path).exists() {
                all_match = false;
                break;
            }

            let disk_hash = hash_of(&disk_path);

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

    // If the on-disk file matches no snapshot, no version is marked current.
    // Claiming the latest one is current would mislead the user into
    // believing their working state is already versioned.
    if !current_version_id.is_empty() {
        for version in versions.iter_mut() {
            version.is_current = version.id == current_version_id;
        }
    }
}

/// Reads one project's version entries from its log. Streams line by
/// line: version logs can reach megabytes, and this runs wherever a
/// version list is needed, including the versioning thread.
pub(crate) fn parse_versions_from_log(log_path: &str, primary_file: &str) -> Vec<VersionEntry> {
    // Stream the log line by line instead of loading it whole: version
    // logs can reach megabytes and this runs on the UI thread.
    let Ok(file) = fs::File::open(log_path) else {
        return Vec::new();
    };
    let mut reader = std::io::BufReader::new(file);

    // One entry per version. Bundle saves write several log lines (one
    // per internal file) sharing a version id; they are folded into one
    // entry whose "files" list holds every file of that save.
    let mut versions: Vec<VersionEntry> = Vec::new();
    let mut version_index_by_id: HashMap<String, usize> = HashMap::new();
    // Lines written before version ids existed are numbered by their
    // position among the artifact's saves, exactly as the writer numbers
    // them when handing out the next id. Counting only the version-less
    // lines instead would give one of them an id an explicit version
    // already uses, and the two saves would be shown, and restored, as one.
    let mut running_ordinal = 0i64;
    let mut raw: Vec<u8> = Vec::new();

    loop {
        raw.clear();
        match std::io::BufRead::read_until(&mut reader, b'\n', &mut raw) {
            Ok(0) => break,
            Ok(_) => {}
            // A torn read must not get cached as the log's parsed state.
            Err(_) => return Vec::new(),
        }
        let Some(obj) = parse_log_line(&raw) else {
            continue;
        };

        let staged_path = json_str(&obj, "staged");
        if !artifact_equals(&artifact_of_log_line(&obj), primary_file) || staged_path.is_empty() {
            continue;
        }

        running_ordinal += 1;
        let mut version_id = json_str(&obj, "version").to_string();
        if version_id.is_empty() {
            version_id = running_ordinal.to_string();
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
            is_compressed: false,
            files: vec![file_entry],
        };
        version_index_by_id.insert(version_id, versions.len());
        versions.push(version);
    }

    versions
}

/// A version is compressed once compaction has removed its staged copies:
/// no file of the version still has one on disk, so restoring it
/// decompresses from the object store instead of copying a staged file.
pub(crate) fn mark_compressed_versions(project: &DiscoveredProject, versions: &mut [VersionEntry]) {
    for version in versions.iter_mut() {
        version.is_compressed = !version.files.is_empty()
            && version.files.iter().all(|file_entry| {
                let staged = resolve_staged_path(&project.root_path, &file_entry.staged_path);
                staged.is_empty() || !Path::new(&staged).exists()
            });
    }
}

/// Identity of a project. Several loose project files can live in one
/// folder (the "Files" layout), so the folder alone does not identify one:
/// notes, dates, the registry and the selection all key on this instead, or
/// siblings would share and overwrite each other's data.
pub(crate) fn project_key(root_path: &str, primary_file: &str) -> String {
    format!(
        "{}\u{0}{}",
        path_key(root_path),
        primary_file.to_lowercase()
    )
}

fn project_key_of(project: &DiscoveredProject) -> String {
    project_key(&project.root_path, &project.primary_project_file)
}

pub(crate) fn resolve_staged_path(project_root: &str, staged_path: &str) -> String {
    if staged_path.is_empty() {
        return String::new();
    }
    if staged_path.starts_with('/') || (cfg!(windows) && staged_path.chars().nth(1) == Some(':')) {
        return staged_path.to_string();
    }
    join_path(project_root, staged_path)
}

/// Replaces a version log with `lines`. Callers hold
/// [`crate::metadata_store::log_write_guard`] across their read and this
/// write, so a snapshot appended by the versioning thread in between is not
/// lost when the replacement file lands.
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
    if path_compare_case_insensitive() {
        return strip_prefix_case_insensitive(&normalized_value, &prefix_with_slash)
            .map(str::to_string)
            .unwrap_or_default();
    }
    match normalized_value.strip_prefix(&prefix_with_slash) {
        Some(rest) => rest.to_string(),
        None => String::new(),
    }
}

/// Case-insensitive prefix strip that never slices `value` off a char
/// boundary: lowercasing can change byte length, so `prefix.len()` is not a
/// valid index into `value`. Returns `None` when the prefix doesn't match or
/// when it ends inside one value char's lowercase expansion.
fn strip_prefix_case_insensitive<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let mut prefix_lower = prefix.chars().flat_map(char::to_lowercase);
    let mut value_indices = value.char_indices();
    loop {
        let Some(expected) = prefix_lower.next() else {
            let rest_start = value_indices.next().map_or(value.len(), |(i, _)| i);
            return Some(&value[rest_start..]);
        };
        let (_, value_char) = value_indices.next()?;
        let mut value_lower = value_char.to_lowercase();
        if value_lower.next() != Some(expected) {
            return None;
        }
        for unit in value_lower {
            if prefix_lower.next() != Some(unit) {
                return None;
            }
        }
    }
}

fn artifact_set_contains(artifacts: &HashSet<String>, candidate: &str) -> bool {
    artifacts
        .iter()
        .any(|artifact| artifact_equals(artifact, candidate))
}

/// One hashed file's cache entry costs ~200 bytes; this cap keeps the cache
/// around 1 MB while still covering every file of any realistic project.
const FILE_HASH_CACHE_CAP: usize = 4096;
/// Parsed-log entries hold a project's whole version list, so far fewer fit.
const PARSED_LOG_CACHE_CAP: usize = 16;

/// A cached value validated by the source file's (mtime, len), aged by a
/// shared use counter so stale projects fall out of the bounded caches.
struct CachedByStamp<T> {
    stamp: (Option<SystemTime>, u64),
    last_used: u64,
    value: T,
}

/// Drops the least-recently-used half of `map` (called only at the cap, so
/// the sort runs rarely and over at most a few thousand entries).
fn evict_lru_half<T>(map: &mut HashMap<String, CachedByStamp<T>>) {
    if map.is_empty() {
        return;
    }
    let mut ages: Vec<u64> = map.values().map(|entry| entry.last_used).collect();
    ages.sort_unstable();
    let cutoff = ages[ages.len() / 2];
    map.retain(|_, entry| entry.last_used > cutoff);
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
    let mut bytes = serde_json::to_vec(&Value::Object(obj.clone())).expect("log line serializes");
    bytes.push(b'\n');
    bytes
}

/// Older watcher routing could attach a newly-created nested project to an
/// already-known ancestor project. Move those log entries and their storage
/// into the project's own .musit directory before baseline seeding runs.
pub(crate) fn migrate_misrouted_history(source_root: &str, destination_root: &str) -> usize {
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
    // Both logs are read and rewritten here, so nothing may append to either
    // in the meantime.
    let _log_guard = crate::metadata_store::log_write_guard();
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
            if !Path::new(&destination_object_path).exists()
                && (fs::create_dir_all(parent_path(&destination_object_path)).is_err()
                    || fs::copy(&source_object_path, &destination_object_path).is_err())
            {
                return 0;
            }
        }

        let stored_staged_path = json_str(obj, "staged").to_string();
        if stored_staged_path.is_empty() {
            continue;
        }

        let source_staged_path = if stored_staged_path.starts_with('/')
            || (cfg!(windows) && stored_staged_path.chars().nth(1) == Some(':'))
        {
            stored_staged_path.clone()
        } else {
            join_path(source_root, &stored_staged_path)
        };
        let relative_to_source_staging =
            relative_file_path(&join_path(&source_musit, "staging"), &source_staged_path);
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
            && (fs::create_dir_all(parent_path(&destination_staged_path)).is_err()
                || fs::copy(&source_staged_path, &destination_staged_path).is_err())
        {
            return 0;
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
    status_message: String,
    // Configured projects folders; the UI shows one at a time via tabs, but
    // every folder is watched and versioned simultaneously.
    workspaces: Vec<FolderWorkspace>,
    active_workspace: i32,
    // Folders waiting for their discovery scan (scans run one at a time on a
    // background thread; the queue keeps startup with many folders sane).
    scan_queue: Vec<String>,
    pending_projects_folder_setup: String,
    project_root: String,
    /// Identity of the selected project (see [`project_key`]). The folder
    /// alone is ambiguous when several loose project files share one.
    selected_project_key: String,
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
    theme_saturation: f64,
    theme_saturation_persist_deadline: Option<Instant>,
    launch_at_startup: bool,
    snapshot_retention: i32,
    notifications_enabled: bool,
    color_scheme_mode: ColorSchemeMode,

    // Indexes into the ACTIVE workspace's discovered_projects.
    visible_project_indexes: Vec<usize>,
    /// Keyed by [`project_key`], not by folder.
    last_opened_at_by_project: HashMap<String, SystemTime>,
    /// Both keyed by [`project_key`].
    project_notes: HashMap<String, String>,
    project_primary_files: HashMap<String, String>,

    project_registry: ProjectRegistry,
    scan_cancel_flag: Option<Arc<AtomicBool>>,
    last_scan_status_directories: i32,
    active_scan_folder: String,
    active_scan_generation: i32,
    active_scan_started_at: Option<Instant>,
    // Projects awaiting monitoring init (tagged with their folder root),
    // advanced a time-sliced batch per pump so first-time seeding of a large
    // folder doesn't freeze the UI.
    monitoring_init_queue: std::collections::VecDeque<(String, DiscoveredProject)>,
    monitoring_init_total: usize,
    monitoring_init_done: usize,
    /// Set when an init batch blows its time budget (a project on a slow
    /// network share can take seconds for a single item): the next batch waits
    /// until then so the UI keeps a usable share of the pump.
    /// Newest discovery partial of the running scan, overwritten by the scan
    /// thread and taken by `handle_projects_updated`.
    scan_partial_slot: Arc<Mutex<Option<Vec<DiscoveredProject>>>>,
    /// Owns the snapshot services and does their file work off the UI thread.
    versioning: versioning::Worker,
    /// Init job currently running on the worker: (folder root, project root).
    monitoring_init_in_flight: Option<(String, String)>,
    /// Projects found by the watcher whose versioning is still coming up on
    /// the worker, by `path_key` of their root: (folder root, project).
    pending_adoptions: HashMap<String, (String, DiscoveredProject)>,
    /// Folder tab that owns each initialized project, by `path_key` of the
    /// project root. Teardown follows this rather than path containment, so
    /// a folder nested inside another does not drop the other's projects.
    folder_by_root_key: HashMap<String, String>,
    /// Version lists the versioning thread has built, by [`project_key`].
    /// The graph is drawn from these, so opening a project or receiving a
    /// save never makes the window wait on the filesystem.
    versions_by_project: HashMap<String, Vec<VersionEntry>>,
    /// Projects whose list is being rebuilt, so a burst of saves queues one
    /// rebuild rather than one per event.
    version_loads_in_flight: HashSet<String>,
    // (mtime, len)-validated caches so version-graph refreshes don't re-hash
    // project files or re-parse log.jsonl on the UI thread unless they
    // actually changed on disk. Both are bounded (see CachedByStamp): the
    // least-recently-used half is dropped at the cap, so a long session
    // across many projects can't accumulate entries forever.
    file_hash_cache: std::cell::RefCell<HashMap<String, CachedByStamp<String>>>,
    parsed_log_cache: std::cell::RefCell<HashMap<String, CachedByStamp<Vec<VersionEntry>>>>,
    cache_use_counter: std::cell::Cell<u64>,
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
        let versioning = {
            let reply_tx = msg_tx.clone();
            // The notice sink is shared by every service (hence Sync); the
            // sender itself is not Sync, so it sits behind a mutex.
            let notice_tx = Mutex::new(msg_tx.clone());
            versioning::Worker::spawn(
                move |reply| {
                    let _ = reply_tx.send(BackendMsg::Versioning(reply));
                },
                Arc::new(move |project_root: String, notice: SnapshotNotice| {
                    if let Ok(tx) = notice_tx.lock() {
                        let _ = tx.send(BackendMsg::Snapshot {
                            project_root,
                            notice,
                        });
                    }
                }),
            )
        };

        let mut backend = Self {
            status_message: strings::STATUS_SELECT_PROJECTS_FOLDER.to_string(),
            workspaces: Vec::new(),
            active_workspace: -1,
            scan_queue: Vec::new(),
            pending_projects_folder_setup: String::new(),
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
            theme_saturation: DEFAULT_THEME_SATURATION,
            theme_saturation_persist_deadline: None,
            launch_at_startup: false,
            snapshot_retention: UNCOMPRESSED_RECENT_VERSIONS,
            notifications_enabled: true,
            color_scheme_mode: ColorSchemeMode::System,
            visible_project_indexes: Vec::new(),
            selected_project_key: String::new(),
            last_opened_at_by_project: HashMap::new(),
            project_notes: HashMap::new(),
            project_primary_files: HashMap::new(),
            project_registry,
            scan_cancel_flag: None,
            last_scan_status_directories: 0,
            active_scan_folder: String::new(),
            active_scan_generation: 0,
            active_scan_started_at: None,
            monitoring_init_queue: std::collections::VecDeque::new(),
            monitoring_init_total: 0,
            monitoring_init_done: 0,
            scan_partial_slot: Arc::new(Mutex::new(None)),
            file_hash_cache: std::cell::RefCell::new(HashMap::new()),
            parsed_log_cache: std::cell::RefCell::new(HashMap::new()),
            cache_use_counter: std::cell::Cell::new(0),
            versioning,
            monitoring_init_in_flight: None,
            pending_adoptions: HashMap::new(),
            folder_by_root_key: HashMap::new(),
            versions_by_project: HashMap::new(),
            version_loads_in_flight: HashSet::new(),
            canonical_root_by_key: HashMap::new(),
            platform,
            msg_tx,
            msg_rx,
            events: Vec::new(),
        };

        let saved_settings = backend.project_registry.load_app_settings();
        backend.theme_hue = saved_settings.theme_hue;
        backend.theme_saturation = clamp_theme_saturation(saved_settings.theme_saturation);
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

        for saved_folder in backend.project_registry.load_projects_folders() {
            if folder_settings::has_layout_setting(&saved_folder) {
                backend.load_projects_from_folder(&saved_folder, "");
            } else if backend.pending_projects_folder_setup.is_empty() {
                backend.pending_projects_folder_setup = saved_folder;
                backend.push_event(BackendEvent::PendingProjectsFolderSetupChanged);
            }
        }
        if !backend.workspaces.is_empty() {
            // Each restored folder made itself active as it was added; the
            // first one wins, and the visible list has to follow.
            backend.active_workspace = 0;
            backend.rebuild_visible_projects();
            backend.apply_active_folder_hue();
            backend.prioritize_active_folder_scan();
            backend.refresh_status_for_active_folder();
            backend.push_event(BackendEvent::ActiveFolderChanged);
        }

        backend.run_startup_self_check();
        backend
    }

    // ---- Workspaces (projects-folder tabs) --------------------------------

    fn active(&self) -> Option<&FolderWorkspace> {
        usize::try_from(self.active_workspace)
            .ok()
            .and_then(|i| self.workspaces.get(i))
    }

    fn active_mut(&mut self) -> Option<&mut FolderWorkspace> {
        usize::try_from(self.active_workspace)
            .ok()
            .and_then(|i| self.workspaces.get_mut(i))
    }

    fn active_root(&self) -> &str {
        self.active().map(|w| w.root.as_str()).unwrap_or("")
    }

    fn active_layout(&self) -> ProjectsFolderLayout {
        self.active()
            .map(|w| w.layout)
            .unwrap_or(ProjectsFolderLayout::Bundles)
    }

    fn active_projects(&self) -> &[DiscoveredProject] {
        self.active()
            .map(|w| w.discovered_projects.as_slice())
            .unwrap_or(&[])
    }

    fn workspace_index_by_root(&self, root: &str) -> Option<usize> {
        self.workspaces
            .iter()
            .position(|w| path_equals(&w.root, root))
    }

    fn is_active_folder(&self, folder_root: &str) -> bool {
        self.active()
            .is_some_and(|w| path_equals(&w.root, folder_root))
    }

    /// Status line for work happening in one folder. Background folders keep
    /// their progress out of the status bar — it describes the tab on screen.
    fn set_status_for_folder(&mut self, folder_root: &str, message: String) {
        if !self.is_active_folder(folder_root) || self.status_message == message {
            return;
        }
        self.status_message = message;
        self.push_event(BackendEvent::StatusMessageChanged);
    }

    /// Progress for the folder's own tab title. Unlike the status line this
    /// is kept per folder, so a background scan still shows on its own tab.
    fn set_scan_progress_for_folder(&mut self, folder_root: &str, progress: String) {
        let Some(index) = self.workspace_index_by_root(folder_root) else {
            return;
        };
        if self.workspaces[index].scan_progress == progress {
            return;
        }
        self.workspaces[index].scan_progress = progress;
        self.push_event(BackendEvent::FoldersChanged);
    }

    /// Rebuilds the status line from the active folder's own state (used when
    /// the active tab changes, so it never inherits another folder's message).
    fn refresh_status_for_active_folder(&mut self) {
        let Some(workspace) = self.active() else {
            self.status_message = strings::STATUS_SELECT_PROJECTS_FOLDER.to_string();
            self.push_event(BackendEvent::StatusMessageChanged);
            return;
        };
        let root = workspace.root.clone();
        let scanning = workspace.scanning;
        let watched = workspace.watcher_ready;
        let count = workspace.discovered_projects.len();
        let queued = self.scan_queue.iter().any(|q| path_equals(q, &root));

        let message = if scanning {
            format!("Scanning {root} for projects...")
        } else if queued {
            format!("Waiting to scan {root}...")
        } else if count == 0 {
            "No supported project files found.".to_string()
        } else if watched {
            format!("Monitoring: {root}")
        } else {
            format!("Found {count} project folders.")
        };
        if self.status_message != message {
            self.status_message = message;
            self.push_event(BackendEvent::StatusMessageChanged);
        }
    }

    /// The project the given visible-list index points at (active workspace).
    fn visible_project(&self, visible_index: i32) -> Option<&DiscoveredProject> {
        if visible_index < 0 {
            return None;
        }
        let discovered_index = *self.visible_project_indexes.get(visible_index as usize)?;
        self.active_projects().get(discovered_index)
    }

    pub fn folder_tabs(&self) -> Vec<FolderTab> {
        self.workspaces
            .iter()
            .map(|w| FolderTab {
                path: w.root.clone(),
                name: if !w.display_name.is_empty() {
                    w.display_name.clone()
                } else {
                    let name = file_name(&w.root);
                    if name.is_empty() {
                        w.root.clone()
                    } else {
                        name.to_string()
                    }
                },
                layout: layout_display_name(w.layout),
                is_scanning: w.scanning || self.scan_queue.iter().any(|q| path_equals(q, &w.root)),
                status: if w.scanning {
                    if w.scan_progress.is_empty() {
                        "scanning\u{2026}".to_string()
                    } else {
                        format!("scanning {}", w.scan_progress)
                    }
                } else if self.scan_queue.iter().any(|q| path_equals(q, &w.root)) {
                    "queued".to_string()
                } else {
                    String::new()
                },
            })
            .collect()
    }

    /// Renames a folder tab (empty name restores the folder's own name); the
    /// custom name is persisted in the registry.
    pub fn rename_projects_folder(&mut self, index: i32, name: &str) {
        if index < 0 || index as usize >= self.workspaces.len() {
            return;
        }
        let trimmed = name.trim().to_string();
        let workspace = &mut self.workspaces[index as usize];
        if workspace.display_name == trimmed {
            return;
        }
        workspace.display_name = trimmed.clone();
        let root = workspace.root.clone();
        self.project_registry
            .save_projects_folder_name(&root, &trimmed);
        folder_settings::save_name(&root, &trimmed);
        self.log_config_change(&format!("folder tab renamed to \"{trimmed}\" for {root}"));
        self.push_event(BackendEvent::FoldersChanged);
    }

    pub fn active_folder_index(&self) -> i32 {
        self.active_workspace
    }

    /// Moves a folder tab to another position in the strip. The active tab
    /// stays active wherever it lands, and the new order is remembered.
    pub fn move_folder_tab(&mut self, from: i32, to: i32) {
        let count = self.workspaces.len() as i32;
        if from < 0 || to < 0 || from >= count || to >= count || from == to {
            return;
        }
        let active_root = self.active().map(|workspace| workspace.root.clone());
        let workspace = self.workspaces.remove(from as usize);
        self.workspaces.insert(to as usize, workspace);
        if let Some(root) = active_root
            && let Some(index) = self.workspace_index_by_root(&root)
        {
            self.active_workspace = index as i32;
        }
        self.persist_projects_folders();
        self.push_event(BackendEvent::FoldersChanged);
        self.push_event(BackendEvent::ActiveFolderChanged);
    }

    pub fn set_active_folder_index(&mut self, index: i32) {
        if index < 0 || index as usize >= self.workspaces.len() || index == self.active_workspace {
            return;
        }
        self.active_workspace = index;
        self.set_selected_project_index(-1);
        // A filter typed for one folder must not hide another folder's list.
        if !self.search_text.is_empty() {
            self.search_text.clear();
            self.push_event(BackendEvent::SearchTextChanged);
        }
        self.rebuild_visible_projects();
        self.apply_active_folder_hue();
        // Whatever the user just switched to gets the machine's attention: its
        // scan jumps the queue (preempting a background one if need be) and its
        // versioning init moves to the front.
        self.prioritize_active_folder_scan();
        self.prioritize_active_folder_monitoring();
        self.refresh_status_for_active_folder();
        self.push_event(BackendEvent::ActiveFolderChanged);
        self.push_event(BackendEvent::ProjectsFolderChanged);
        self.push_event(BackendEvent::ProjectsFolderLayoutChanged);
    }

    /// Adds a projects folder (or re-activates/rescans an existing one).
    /// The wizard and the tab bar's "+" both land here.
    pub fn add_projects_folder(&mut self, folder_path: &str, layout: &str) {
        self.confirm_projects_folder(folder_path, layout);
    }

    /// Removes a folder tab: stops watching/monitoring it and forgets it in
    /// the registry. On-disk version history (.musit) is left untouched.
    pub fn remove_projects_folder(&mut self, index: i32) {
        if index < 0 || index as usize >= self.workspaces.len() {
            return;
        }
        let root = self.workspaces[index as usize].root.clone();
        self.scan_queue.retain(|q| !path_equals(q, &root));
        if path_equals(&self.active_scan_folder, &root) {
            self.cancel_project_scan();
        }
        self.stop_monitoring_for_folder(&root);
        self.workspaces.remove(index as usize);
        // The caches hold entries keyed by paths under the removed folder;
        // they are stamp-validated, so a wholesale clear is safe and refills
        // on demand for the folders that remain.
        self.file_hash_cache.borrow_mut().clear();
        self.parsed_log_cache.borrow_mut().clear();
        self.persist_projects_folders();
        self.project_registry.save_projects_folder_name(&root, "");

        if self.workspaces.is_empty() {
            self.active_workspace = -1;
        } else if self.active_workspace >= index {
            self.active_workspace = (self.active_workspace - 1).max(0);
        }
        self.set_selected_project_index(-1);
        self.rebuild_visible_projects();
        // The tab that takes over brings its own colour, exactly as if the
        // user had clicked it.
        self.apply_active_folder_hue();
        self.prioritize_active_folder_scan();
        self.prioritize_active_folder_monitoring();
        self.refresh_status_for_active_folder();
        self.append_activity(&format!(
            "[{}] removed projects folder {}",
            now_human(),
            root
        ));
        self.push_event(BackendEvent::FoldersChanged);
        self.push_event(BackendEvent::ActiveFolderChanged);
        self.push_event(BackendEvent::ProjectsFolderChanged);
    }

    fn persist_projects_folders(&mut self) {
        let folders: Vec<String> = self.workspaces.iter().map(|w| w.root.clone()).collect();
        self.project_registry.save_projects_folders(&folders);
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
        while let Ok(msg) = self.msg_rx.try_recv() {
            self.handle_msg(msg);
        }

        if self
            .theme_hue_persist_deadline
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            self.theme_hue_persist_deadline = None;
            self.persist_theme_hue();
            let hue = self.theme_hue;
            self.log_config_change(&format!("theme hue set to {hue:.0}"));
        }
        if self
            .theme_saturation_persist_deadline
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            self.theme_saturation_persist_deadline = None;
            self.persist_app_settings();
            let percent = self.theme_saturation * 100.0;
            self.log_config_change(&format!("theme saturation set to {percent:.0}%"));
        }

        self.advance_monitoring_init();
    }

    /// Persists any debounced settings immediately; call on app shutdown,
    /// since the debounce timer won't fire again.
    pub fn flush_pending_persist(&mut self) {
        if self.theme_hue_persist_deadline.take().is_some() {
            self.persist_theme_hue();
        }
        if self.theme_saturation_persist_deadline.take().is_some() {
            self.persist_app_settings();
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
                directories_scanned,
                folder_path,
                scan_generation,
            } => {
                self.handle_projects_updated(directories_scanned, &folder_path, scan_generation);
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
            BackendMsg::WatcherEvent { watch_root, event } => {
                self.note_primary_file_change(&watch_root, &event);
                // Resolve a newly-created nested project before an ancestor
                // project gets a chance to claim the event.
                let mut owned_by_nested_project = false;
                for project_root in self.canonical_root_by_key.values() {
                    if !path_equals(project_root, &watch_root)
                        && path_is_under_root(&event.absolute_path, project_root)
                    {
                        owned_by_nested_project = true;
                        break;
                    }
                }
                if !owned_by_nested_project
                    && self.try_discover_project_from_event(&watch_root, &event)
                {
                    return;
                }
                self.dispatch_file_event(&event);
            }
            BackendMsg::WatcherReady { root_path } => {
                if let Some(index) = self
                    .workspace_index_by_root(&root_path)
                    .filter(|&index| self.workspaces[index].watcher.is_some())
                {
                    self.workspaces[index].watcher_ready = true;
                    self.set_status_for_folder(&root_path, format!("Monitoring: {root_path}"));
                    self.append_activity(&format!(
                        "[{}] monitoring started for {}",
                        Local::now().format("%Y-%m-%dT%H:%M:%S"),
                        root_path
                    ));
                }
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
                        self.request_selected_versions();
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
            BackendMsg::Versioning(reply) => self.handle_versioning_reply(reply),
        }
    }

    fn handle_versioning_reply(&mut self, reply: VersioningReply) {
        match reply {
            VersioningReply::InitDone {
                folder_root,
                generation,
                project_root,
                ok,
                seeded,
                migrated,
            } => {
                // A reply from before the folder's monitoring was torn down
                // (rescan, removal) must not resurrect it: the worker has
                // already dropped that service again.
                let current_generation = self
                    .workspace_index_by_root(&folder_root)
                    .map(|index| self.workspaces[index].monitoring_generation);
                if current_generation != Some(generation) {
                    self.pending_adoptions.remove(&path_key(&project_root));
                    self.advance_monitoring_init();
                    return;
                }
                if self
                    .monitoring_init_in_flight
                    .as_ref()
                    .is_some_and(|(_, root)| path_equals(root, &project_root))
                {
                    self.monitoring_init_in_flight = None;
                }
                if migrated > 0 {
                    self.append_activity(&format!(
                        "[{}] migrated {} snapshot entries into {}",
                        now_human(),
                        migrated,
                        project_root
                    ));
                }
                if ok {
                    self.canonical_root_by_key
                        .insert(path_key(&project_root), project_root.clone());
                    self.folder_by_root_key
                        .insert(path_key(&project_root), folder_root.clone());
                    if seeded {
                        self.append_debug_activity(&format!(
                            "[{}] seeded initial version v1 for {}",
                            now_human(),
                            project_root
                        ));
                    }
                    if path_equals(&project_root, &self.project_root) {
                        self.request_selected_versions();
                    }
                } else {
                    self.append_debug_activity(&format!(
                        "[{}] failed to initialize versioning for {}",
                        now_human(),
                        project_root
                    ));
                }
                if let Some((adoption_folder, adopted)) =
                    self.pending_adoptions.remove(&path_key(&project_root))
                {
                    match self.workspace_index_by_root(&adoption_folder) {
                        Some(index) if ok => {
                            self.publish_adopted_project(index, adopted);
                        }
                        _ => {
                            // Not a debug detail: the project the user just
                            // created will not appear in the list, and they
                            // need to know why.
                            self.append_activity(&format!(
                                "[{}] could not start versioning for {}; it is not being watched",
                                now_human(),
                                adopted.root_path
                            ));
                        }
                    }
                }

                // A folder is watchable as soon as *its* projects are done —
                // waiting for the whole queue would leave healthy folders
                // unwatched behind one that is crawling over the network.
                let folder_pending = self
                    .monitoring_init_queue
                    .iter()
                    .any(|(root, _)| path_equals(root, &folder_root));
                if !folder_pending {
                    self.start_watcher_for_folder(&folder_root);
                }
                if self.monitoring_init_queue.is_empty() {
                    self.monitoring_init_total = 0;
                    self.monitoring_init_done = 0;
                    self.start_watchers_when_ready();
                } else {
                    self.advance_monitoring_init();
                }
            }
            VersioningReply::Versions {
                project_root,
                primary_file,
                versions,
            } => {
                let key = project_key(&project_root, &primary_file);
                self.version_loads_in_flight.remove(&key);
                let changed = self
                    .versions_by_project
                    .get(&key)
                    .is_none_or(|existing| existing != &versions);
                self.versions_by_project.insert(key.clone(), versions);
                if changed && key == self.selected_project_key {
                    self.push_event(BackendEvent::SelectedProjectVersionGraphChanged);
                }
            }
            VersioningReply::RestoreDone {
                project_root,
                project_name,
                version_id,
                result,
            } => match result {
                Ok(()) => {
                    self.status_message =
                        format!("Restored {project_name} to version {version_id}");
                    self.push_event(BackendEvent::StatusMessageChanged);
                    self.append_activity(&format!(
                        "[{}] restored {} to version {}",
                        now_human(),
                        project_name,
                        version_id
                    ));
                    // Hand the restored file to the DAW. The project is
                    // found by the root the reply carries, so a selection
                    // that moved while the job ran does not send the wrong
                    // one (or none at all).
                    if let Some(index) =
                        self.visible_project_indexes.iter().position(|&discovered| {
                            self.active_projects()
                                .get(discovered)
                                .is_some_and(|p| path_equals(&p.root_path, &project_root))
                        })
                    {
                        self.open_project(index as i32);
                    }
                    self.request_selected_versions();
                }
                Err(message) => {
                    self.append_activity(&format!("[{}] {}", now_human(), message));
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
        !self.active_projects().is_empty()
    }

    pub fn has_projects_folder(&self) -> bool {
        !self.workspaces.is_empty()
    }

    pub fn projects_folder_path(&self) -> &str {
        self.active_root()
    }

    /// Whether the ACTIVE folder is still being scanned (what the list UI
    /// cares about); other folders scan in the background.
    pub fn is_scanning_projects(&self) -> bool {
        self.active().is_some_and(|w| w.scanning)
            || self
                .active()
                .is_some_and(|w| self.scan_queue.iter().any(|q| path_equals(q, &w.root)))
    }

    /// Whether any folder is scanning or queued to scan.
    pub fn is_scanning_any_folder(&self) -> bool {
        !self.scan_queue.is_empty() || self.workspaces.iter().any(|w| w.scanning)
    }

    pub fn activity(&self) -> &[String] {
        &self.activity
    }

    pub fn theme_hue(&self) -> f64 {
        self.theme_hue
    }

    /// Adopts the active tab's remembered colour (the UI eases into it).
    fn apply_active_folder_hue(&mut self) {
        let Some(hue) = self.active().and_then(|w| w.hue) else {
            return;
        };
        if (self.theme_hue - hue).abs() < 1e-9 {
            return;
        }
        // Assigned rather than routed through set_theme_hue: a tab's colour is
        // that folder's, and must not overwrite the app-wide default.
        self.theme_hue = hue;
        self.push_event(BackendEvent::ThemeHueChanged);
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
        // The colour belongs to the tab being looked at; it is written to that
        // folder's settings.json when the debounce fires.
        if let Some(workspace) = self.active_mut() {
            workspace.hue = Some(wrapped);
        }
        self.push_event(BackendEvent::ThemeHueChanged);
        self.theme_hue_persist_deadline = Some(Instant::now() + THEME_HUE_PERSIST_DELAY);
    }

    /// Writes the debounced hue to the app config and to the active folder's
    /// settings.json, so a re-added folder comes back the same colour.
    fn persist_theme_hue(&mut self) {
        self.persist_app_settings();
        let hue = self.theme_hue;
        if let Some(root) = self.active().map(|w| w.root.clone()) {
            folder_settings::save_hue(&root, hue);
        }
    }

    pub fn theme_saturation(&self) -> f64 {
        self.theme_saturation
    }

    /// Sets the palette's chroma multiplier (`0.0..=MAX_THEME_SATURATION`).
    /// App-wide: unlike the hue it is not a property of the active tab.
    pub fn set_theme_saturation(&mut self, value: f64) {
        let clamped = clamp_theme_saturation(value);
        if (self.theme_saturation - clamped).abs() < 1e-9 {
            return;
        }
        self.theme_saturation = clamped;
        self.push_event(BackendEvent::ThemeSaturationChanged);
        self.theme_saturation_persist_deadline = Some(Instant::now() + THEME_HUE_PERSIST_DELAY);
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
        self.versioning.send(VersioningJob::SetRetention {
            retention: self.snapshot_retention,
            compact_existing,
        });
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
        if let Some(project) = self.visible_project(visible_index).cloned() {
            self.project_root = project.root_path.clone();
            self.selected_project_key = project_key_of(&project);
            // The list for this project may be stale or missing; the graph
            // draws what is cached and updates when the rebuild lands.
            self.request_versions(&project);
            self.selected_project_note = self
                .project_notes
                .get(&self.selected_project_key)
                .cloned()
                .unwrap_or_default();
            self.selected_project_primary_file = project.primary_project_file.clone();
        } else {
            self.project_root.clear();
            self.selected_project_key.clear();
            self.selected_project_note.clear();
            self.selected_project_primary_file.clear();
        }

        self.push_event(BackendEvent::SelectedProjectNoteChanged);
        self.push_event(BackendEvent::SelectedProjectFilesChanged);
        self.push_event(BackendEvent::SelectedProjectPrimaryFileChanged);
        self.push_event(BackendEvent::SelectedProjectVersionGraphChanged);
    }

    pub fn selected_project_files(&self) -> Vec<String> {
        self.visible_project(self.selected_project_index)
            .map(|project| project.project_files.clone())
            .unwrap_or_default()
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
            let Some(project) = self.active_projects().get(discovered_index) else {
                return;
            };
            if trimmed.is_empty() || !project_contains_file(project, &trimmed) {
                return;
            }
            if project.primary_project_file == trimmed {
                return;
            }
        }

        let (root_path, project_name, previous_primary);
        {
            let Some(workspace) = self.active_mut() else {
                return;
            };
            let project = &mut workspace.discovered_projects[discovered_index];
            previous_primary = project.primary_project_file.clone();
            project.primary_project_file = trimmed.clone();
            root_path = project.root_path.clone();
            project_name = project.name.clone();
        }
        // The picker changes which file the project is, so its identity
        // moves with it and its note and open time follow.
        let previous_key = project_key(&root_path, &previous_primary);
        let new_key = project_key(&root_path, &trimmed);
        if let Some(note) = self.project_notes.remove(&previous_key) {
            self.project_notes.insert(new_key.clone(), note);
        }
        if let Some(opened) = self.last_opened_at_by_project.remove(&previous_key) {
            self.last_opened_at_by_project
                .insert(new_key.clone(), opened);
        }
        self.project_primary_files.remove(&previous_key);
        self.project_primary_files
            .insert(new_key.clone(), trimmed.clone());
        self.selected_project_key = new_key;
        self.selected_project_primary_file = trimmed.clone();

        if self
            .project_registry
            .save_project_primary_file(&root_path, &previous_primary, &trimmed)
            && let Some(project) = self.active_projects().get(discovered_index).cloned()
        {
            self.project_registry.save_project(&project);
        }

        self.rebuild_visible_projects();
        self.push_event(BackendEvent::SelectedProjectPrimaryFileChanged);
        self.request_selected_versions();
        self.log_config_change(&format!(
            "primary project file for {project_name} set to {trimmed}"
        ));
    }

    pub fn projects_folder_layout(&self) -> &'static str {
        layout_display_name(self.active_layout())
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
        let Some(project) = self.visible_project(self.selected_project_index) else {
            return;
        };
        let root_path = project.root_path.clone();
        let key = project_key_of(project);
        let primary_file = project.primary_project_file.clone();
        let trimmed = value.trim().to_string();
        if self.project_notes.get(&key).cloned().unwrap_or_default() == trimmed {
            return;
        }

        if self
            .project_registry
            .save_project_note(&root_path, &primary_file, &trimmed)
        {
            self.project_notes.insert(key, trimmed.clone());
            self.selected_project_note = trimmed;
            self.push_event(BackendEvent::SelectedProjectNoteChanged);
        }
    }

    // ---- Folder selection / scanning --------------------------------------

    pub fn reselect_projects_folder_layout(&mut self, layout: &str) {
        let root = self.active_root().to_string();
        if root.is_empty() {
            return;
        }
        self.confirm_projects_folder(&root, layout);
    }

    pub fn confirm_projects_folder(&mut self, folder_path: &str, layout: &str) {
        let clean = normalize_folder_path(folder_path);
        if clean.is_empty() {
            return;
        }

        // An empty layout means "whatever this folder already says it is":
        // the type lives in the folder's settings.json, so a folder that has
        // been set up before is never re-typed (or re-asked about).
        let saved = folder_settings::load(&clean);
        let folder_layout = if layout.is_empty() {
            saved.layout
        } else {
            layout_from_display_name(layout)
        };
        if folder_layout != saved.layout || !folder_settings::has_layout_setting(&clean) {
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
        }

        if self.pending_projects_folder_setup == clean {
            self.pending_projects_folder_setup.clear();
            self.push_event(BackendEvent::PendingProjectsFolderSetupChanged);
        }

        self.load_projects_from_folder(&clean, layout_display_name(folder_layout));
    }

    fn apply_saved_primary_file_overrides(&self, projects: &mut [DiscoveredProject]) {
        if projects.is_empty() {
            return;
        }
        // One read of the registry for the batch, however many projects.
        let records = self.project_registry.load_project_records();
        for project in projects.iter_mut() {
            let Some(saved_primary) = records
                .get(&project_key_of(project))
                .map(|record| record.primary_project_file.as_str())
                .filter(|saved| !saved.is_empty())
            else {
                continue;
            };
            if !project_contains_file(project, saved_primary) {
                continue;
            }
            project.primary_project_file = saved_primary.to_string();
        }
    }

    /// Adds/updates the folder as a workspace tab, activates it, and queues
    /// its discovery scan (scans run one at a time on a worker thread).
    pub fn load_projects_from_folder(&mut self, folder_path: &str, layout_override: &str) {
        if folder_path.is_empty() {
            return;
        }

        let clean = normalize_folder_path(folder_path);
        if clean.is_empty() {
            return;
        }

        let layout = if !layout_override.is_empty() {
            layout_from_display_name(layout_override)
        } else {
            folder_settings::load_layout(&clean)
        };

        let (index, added) = match self.workspace_index_by_root(&clean) {
            Some(i) => (i, false),
            None => {
                let mut workspace = FolderWorkspace::new(clean.clone(), layout);
                // Re-adding a folder restores the session it saved in its own
                // .immersion/settings.json (tab name, tab colour); the
                // registry is the fallback for folders saved before that.
                let saved = folder_settings::load(&clean);
                workspace.display_name = saved.name.clone();
                workspace.hue = saved.hue;
                if workspace.display_name.is_empty() {
                    let names = self.project_registry.load_projects_folder_names();
                    if let Some((_, name)) =
                        names.iter().find(|(path, _)| path_equals(path, &clean))
                    {
                        workspace.display_name = name.clone();
                    }
                }
                // A folder with no colour of its own adopts the current one,
                // so every tab has a remembered colour from the start.
                if workspace.hue.is_none() {
                    workspace.hue = Some(self.theme_hue);
                    folder_settings::save_hue(&clean, self.theme_hue);
                }
                self.workspaces.push(workspace);
                (self.workspaces.len() - 1, true)
            }
        };
        self.workspaces[index].layout = layout;
        if added {
            self.persist_projects_folders();
            self.push_event(BackendEvent::FoldersChanged);
        }
        if self.active_workspace != index as i32 {
            self.active_workspace = index as i32;
            // Re-adding a folder brings its colour back with it.
            self.apply_active_folder_hue();
            self.push_event(BackendEvent::ActiveFolderChanged);
            self.push_event(BackendEvent::ProjectsFolderChanged);
        }
        self.push_event(BackendEvent::ProjectsFolderLayoutChanged);

        // A stale filter from a previous folder must not hide the new projects.
        if !self.search_text.is_empty() {
            self.search_text.clear();
            self.push_event(BackendEvent::SearchTextChanged);
        }

        self.log_config_change(&format!("projects folder set to {clean}"));
        self.queue_scan(&clean);
    }

    fn queue_scan(&mut self, folder: &str) {
        let scan_in_flight = !self.active_scan_folder.is_empty();
        if scan_in_flight && path_equals(&self.active_scan_folder, folder) {
            // Restart the in-flight scan of this same folder (e.g. a layout
            // change while scanning).
            if let Some(flag) = &self.scan_cancel_flag {
                flag.store(true, Ordering::Relaxed);
            }
            self.begin_scan(folder.to_string());
            return;
        }
        if scan_in_flight {
            if !self.scan_queue.iter().any(|q| path_equals(q, folder)) {
                self.scan_queue.push(folder.to_string());
                self.push_event(BackendEvent::FoldersChanged);
                self.push_event(BackendEvent::IsScanningProjectsChanged);
            }
            return;
        }
        self.begin_scan(folder.to_string());
    }

    fn start_next_queued_scan(&mut self) {
        self.active_scan_folder.clear();
        self.active_scan_started_at = None;
        self.scan_cancel_flag = None;
        while !self.scan_queue.is_empty() {
            let next = self.scan_queue.remove(0);
            if self.workspace_index_by_root(&next).is_some() {
                self.begin_scan(next);
                return;
            }
        }
        self.push_event(BackendEvent::IsScanningProjectsChanged);
        self.push_event(BackendEvent::FoldersChanged);
    }

    /// Makes the active folder's pending scan the one that actually runs. A
    /// folder on an unresponsive share can hold the single scan slot for
    /// minutes; the tab the user just opened should not wait behind it.
    fn prioritize_active_folder_scan(&mut self) {
        let Some(root) = self.active().map(|w| w.root.clone()) else {
            return;
        };
        if path_equals(&self.active_scan_folder, &root) {
            return;
        }
        let Some(position) = self.scan_queue.iter().position(|q| path_equals(q, &root)) else {
            return;
        };
        // A local scan finishes in well under a second; let it, rather than
        // restarting scans every time the user flicks between tabs.
        if !self.active_scan_folder.is_empty()
            && self
                .active_scan_started_at
                .is_some_and(|started| started.elapsed() < Duration::from_millis(750))
        {
            return;
        }
        let queued = self.scan_queue.remove(position);

        if !self.active_scan_folder.is_empty() {
            // Park the background scan at the head of the queue and drop its
            // results; its generation is stale from here on.
            let preempted = self.active_scan_folder.clone();
            if let Some(flag) = &self.scan_cancel_flag {
                flag.store(true, Ordering::Relaxed);
            }
            if let Some(index) = self.workspace_index_by_root(&preempted) {
                self.workspaces[index].scanning = false;
                self.workspaces[index].scan_progress.clear();
            }
            self.append_activity(&format!(
                "[{}] project scan deferred: {} (switched to {})",
                now_human(),
                preempted,
                root
            ));
            self.scan_queue.insert(0, preempted);
        }
        self.begin_scan(queued);
    }

    fn begin_scan(&mut self, clean: String) {
        let Some(index) = self.workspace_index_by_root(&clean) else {
            return;
        };
        let layout = self.workspaces[index].layout;

        self.active_scan_folder = clean.clone();
        self.active_scan_started_at = Some(Instant::now());
        self.active_scan_generation += 1;
        let scan_generation = self.active_scan_generation;
        self.last_scan_status_directories = 0;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.scan_cancel_flag = Some(cancel_flag.clone());
        let partial_slot: Arc<Mutex<Option<Vec<DiscoveredProject>>>> = Arc::new(Mutex::new(None));
        self.scan_partial_slot = partial_slot.clone();

        self.stop_monitoring_for_folder(&clean);
        // The previous results stay on screen until this scan delivers its
        // own: clearing up front leaves the tab blank for as long as the scan
        // takes, which on a network share can be forever.
        self.workspaces[index].scanning = true;
        self.workspaces[index].scan_progress.clear();
        if index as i32 == self.active_workspace {
            self.rebuild_visible_projects();
        }
        self.push_event(BackendEvent::IsScanningProjectsChanged);
        self.push_event(BackendEvent::FoldersChanged);

        self.set_status_for_folder(&clean, format!("Scanning {clean} for projects..."));
        self.append_activity(&format!(
            "[{}] project scan started: {}",
            now_human(),
            clean
        ));

        // Discovery worker thread (ProjectDiscoveryScanWorker::scan port).
        let tx = self.msg_tx.clone();
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
                    // One slot rather than a queue of cumulative copies: a
                    // stalled UI reads the newest partial and the older ones
                    // are already gone.
                    if let Ok(mut slot) = partial_slot.lock() {
                        *slot = Some(partial.to_vec());
                    }
                    let _ = progress_tx.send(BackendMsg::ProjectsUpdated {
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

        if self.active_scan_folder.is_empty() {
            return;
        }

        let folder = self.active_scan_folder.clone();
        if let Some(index) = self.workspace_index_by_root(&folder) {
            self.workspaces[index].scanning = false;
            self.workspaces[index].scan_progress.clear();
        }
        self.append_activity(&format!(
            "[{}] project scan cancelled: {}",
            now_human(),
            folder
        ));
        self.push_event(BackendEvent::IsScanningProjectsChanged);
        self.push_event(BackendEvent::FoldersChanged);
        self.start_next_queued_scan();
    }

    fn handle_project_scan_directory(&mut self, directory_path: &str, directories_scanned: i32) {
        self.append_debug_activity(&format!(
            "[{}] scan folder: {}",
            now_human(),
            directory_path
        ));

        if self.active_scan_folder.is_empty() {
            return;
        }

        self.last_scan_status_directories = directories_scanned;
        if directories_scanned == 1 || directories_scanned % 25 == 0 {
            let folder = self.active_scan_folder.clone();
            self.set_scan_progress_for_folder(&folder, format!("{directories_scanned} folders"));
            self.set_status_for_folder(
                &folder,
                format!("Scanning {folder} ({directories_scanned} folders)..."),
            );
        }
    }

    fn handle_projects_updated(
        &mut self,
        directories_scanned: i32,
        folder_path: &str,
        scan_generation: i32,
    ) {
        if scan_generation != self.active_scan_generation {
            return;
        }
        // The newest partial only: stale ones were overwritten in the slot
        // while this thread was busy, so nothing accumulates.
        let partial_projects = self
            .scan_partial_slot
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
            .unwrap_or_default();

        let Some(index) = self.workspace_index_by_root(folder_path) else {
            return;
        };
        if !path_equals(folder_path, &self.active_scan_folder) || !self.workspaces[index].scanning {
            return;
        }

        // Partials are cumulative, so an empty one says nothing — don't let it
        // wipe the results this tab is currently showing.
        let partial_count = partial_projects.len();
        if partial_count > 0 || self.workspaces[index].discovered_projects.is_empty() {
            self.apply_discovered_projects(index, partial_projects);
        }

        self.last_scan_status_directories = directories_scanned;
        let folder = self.active_scan_folder.clone();
        self.set_scan_progress_for_folder(
            &folder,
            if partial_count == 0 {
                format!("{directories_scanned} folders")
            } else {
                format!("{partial_count} found")
            },
        );
        let message = if partial_count == 0 {
            format!("Scanning {folder} ({directories_scanned} folders)...")
        } else {
            format!(
                "Scanning {folder} ({directories_scanned} folders, {partial_count} projects)..."
            )
        };
        self.set_status_for_folder(&folder, message);
    }

    fn apply_discovered_projects(
        &mut self,
        workspace_index: usize,
        projects: Vec<DiscoveredProject>,
    ) {
        let mut merged = projects;
        self.apply_saved_primary_file_overrides(&mut merged);
        self.workspaces[workspace_index].discovered_projects = merged;
        if workspace_index as i32 == self.active_workspace {
            self.rebuild_visible_projects();
        }
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

        self.append_activity(&format!(
            "[{}] project scan finished: {} folders in {} ms ({} projects)",
            now_human(),
            directories_scanned,
            elapsed_ms,
            projects.len()
        ));

        self.finish_loading_projects(projects, folder_path.to_string());
        self.start_next_queued_scan();
    }

    fn finish_loading_projects(&mut self, projects: Vec<DiscoveredProject>, clean_path: String) {
        let Some(workspace_index) = self.workspace_index_by_root(&clean_path) else {
            return;
        };
        self.workspaces[workspace_index].scanning = false;
        self.workspaces[workspace_index].scan_progress.clear();
        self.push_event(BackendEvent::IsScanningProjectsChanged);
        self.push_event(BackendEvent::FoldersChanged);
        self.apply_discovered_projects(workspace_index, projects);

        let project_count = self.workspaces[workspace_index].discovered_projects.len();
        if project_count == 0 {
            self.stop_monitoring_for_folder(&clean_path);
            self.set_status_for_folder(
                &clean_path,
                "No supported project files found.".to_string(),
            );
            self.append_activity(&format!(
                "[{}] first-day setup found no project files under {}",
                now_human(),
                clean_path
            ));
            self.push_event(BackendEvent::ProjectsFolderScanFinished {
                folder: clean_path.clone(),
                found_projects: false,
            });
            return;
        }

        self.persist_projects_folders();
        // One registry write for the whole scan and one read for the notes,
        // not one of each per project.
        self.project_registry
            .save_projects(&self.workspaces[workspace_index].discovered_projects);
        let records = self.project_registry.load_project_records();
        for project in &self.workspaces[workspace_index].discovered_projects {
            let note = records
                .get(&project_key_of(project))
                .map(|record| record.note.clone())
                .unwrap_or_default();
            self.project_notes.insert(project_key_of(project), note);
            self.project_primary_files.insert(
                project_key_of(project),
                project.primary_project_file.clone(),
            );
        }

        self.set_status_for_folder(
            &clean_path,
            format!("Found {project_count} project folders."),
        );

        self.append_activity(&format!(
            "[{}] discovered {} projects from {}",
            now_human(),
            project_count,
            clean_path
        ));

        self.start_monitoring_for_folder(&clean_path);
        self.push_event(BackendEvent::ProjectsFolderScanFinished {
            folder: clean_path.clone(),
            found_projects: true,
        });
    }

    // ---- Opening / selecting ----------------------------------------------

    pub fn open_project(&mut self, visible_index: i32) {
        if visible_index < 0 || visible_index as usize >= self.visible_project_indexes.len() {
            return;
        }

        self.set_selected_project_index(visible_index);

        let Some(project) = self.visible_project(visible_index).cloned() else {
            return;
        };
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

            self.last_opened_at_by_project
                .insert(project_key_of(&project), SystemTime::now());
            // The list carries preformatted dates, so it has to be rebuilt
            // whatever the sort mode, or the row keeps showing the old one.
            self.rebuild_visible_projects();

            self.append_activity(&format!(
                "[{}] file association unavailable; opened folder {}",
                now_human(),
                fallback_path
            ));
            return;
        }

        self.last_opened_at_by_project
            .insert(project_key_of(&project), SystemTime::now());
        self.append_activity(&format!("[{}] opened {}", now_human(), project_file_path));
        self.rebuild_visible_projects();
    }

    pub fn manage_project_versions(&mut self, visible_index: i32) {
        if visible_index < 0 || visible_index as usize >= self.visible_project_indexes.len() {
            return;
        }
        self.set_selected_project_index(visible_index);
    }

    // ---- Versions -----------------------------------------------------------

    /// Monotonic age stamp shared by both bounded caches.
    fn next_cache_use(&self) -> u64 {
        let next = self.cache_use_counter.get() + 1;
        self.cache_use_counter.set(next);
        next
    }

    /// SHA-256 of a file, reused from cache while (mtime, len) is unchanged.
    fn cached_file_sha256(&self, path: &str) -> String {
        let Ok(meta) = fs::metadata(path) else {
            return String::new();
        };
        let stamp = (meta.modified().ok(), meta.len());
        if let Some(entry) = self.file_hash_cache.borrow_mut().get_mut(path)
            && entry.stamp == stamp
        {
            entry.last_used = self.next_cache_use();
            return entry.value.clone();
        }
        let hash = sha256_file_hex(path);
        let mut cache = self.file_hash_cache.borrow_mut();
        // Evict only when the insert would grow the map: a stale-stamp
        // replacement at the cap must not purge unrelated entries.
        if !cache.contains_key(path) && cache.len() >= FILE_HASH_CACHE_CAP {
            evict_lru_half(&mut cache);
        }
        cache.insert(
            path.to_string(),
            CachedByStamp {
                stamp,
                last_used: self.next_cache_use(),
                value: hash.clone(),
            },
        );
        hash
    }

    pub fn get_project_versions(&self, visible_index: i32) -> Vec<VersionEntry> {
        let Some(project) = self.visible_project(visible_index) else {
            return Vec::new();
        };
        let log_path = join_path(&project.root_path, strings::MUSIT_VERSION_LOG_RELATIVE_PATH);
        let mut versions = self.parsed_versions_for(&log_path, &project.primary_project_file);
        self.mark_current_version(project, &mut versions);
        mark_compressed_versions(project, &mut versions);
        versions
    }

    /// Parses one project's version entries from its log, cached against the
    /// log file's (mtime, len) so unchanged logs cost two stat calls.
    fn parsed_versions_for(&self, log_path: &str, primary_file: &str) -> Vec<VersionEntry> {
        let Ok(meta) = fs::metadata(log_path) else {
            return Vec::new();
        };
        let stamp = (meta.modified().ok(), meta.len());
        let cache_key = format!("{log_path}\u{0}{primary_file}");
        if let Some(entry) = self.parsed_log_cache.borrow_mut().get_mut(&cache_key)
            && entry.stamp == stamp
        {
            entry.last_used = self.next_cache_use();
            return entry.value.clone();
        }

        let versions = parse_versions_from_log(log_path, primary_file);

        let mut cache = self.parsed_log_cache.borrow_mut();
        // As above: never evict for a replacement of an existing key.
        if !cache.contains_key(&cache_key) && cache.len() >= PARSED_LOG_CACHE_CAP {
            evict_lru_half(&mut cache);
        }
        cache.insert(
            cache_key,
            CachedByStamp {
                stamp,
                last_used: self.next_cache_use(),
                value: versions.clone(),
            },
        );
        versions
    }

    /// A version is "current" when every file it captured matches what is
    /// on disk right now (by object hash, or by content against the staged
    /// copy). Disk hashes come from the (mtime, len)-validated cache.
    fn mark_current_version(&self, project: &DiscoveredProject, versions: &mut [VersionEntry]) {
        mark_current_version_with(project, versions, |path| self.cached_file_sha256(path));
    }

    /// The selected project's versions as a tree in depth-first order:
    /// parents before their children, siblings in version order, top-level
    /// versions as roots. A node whose parent no longer exists (deleted by an
    /// older release) becomes a root rather than vanishing with its subtree.
    /// Queues a rebuild of a project's version list on the versioning
    /// thread. Cheap to call repeatedly: one rebuild is in flight per
    /// project at a time.
    pub(crate) fn request_versions(&mut self, project: &DiscoveredProject) {
        let key = project_key_of(project);
        if !self.version_loads_in_flight.insert(key) {
            return;
        }
        self.versioning.send(VersioningJob::LoadVersions {
            project: project.clone(),
        });
    }

    /// Queues a rebuild for whichever project is selected.
    fn request_selected_versions(&mut self) {
        if let Some(project) = self.visible_project(self.selected_project_index).cloned() {
            self.request_versions(&project);
        }
    }

    /// The selected project's versions as the graph should draw them. Reads
    /// the list the versioning thread built; empty until the first one
    /// arrives, which is a blank graph for a moment rather than a window
    /// that stops responding.
    pub fn selected_project_version_graph(&self) -> Vec<VersionGraphNode> {
        if self.selected_project_index < 0
            || self.selected_project_index as usize >= self.visible_project_indexes.len()
        {
            return Vec::new();
        }

        let versions = self
            .versions_by_project
            .get(&self.selected_project_key)
            .cloned()
            .unwrap_or_default();
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
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(id.clone());
        }

        let mut graph: Vec<VersionGraphNode> = Vec::new();

        fn append_children(
            parent_id: &str,
            depth: i32,
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
                });
                append_children(
                    &child_id,
                    depth + 1,
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
            &mut graph,
            &children_by_parent,
            &node_by_id,
            &resolved_parent,
        );
        graph
    }

    // ---- Restore / notes / delete -------------------------------------------

    /// Restores a version. The file work runs on the versioning thread;
    /// `true` means the restore was accepted, and its outcome arrives as an
    /// activity line, a status message and a graph refresh.
    pub fn restore_version_by_id(&mut self, version_id_str: &str) -> bool {
        if version_id_str.is_empty() {
            return false;
        }
        let Some(project) = self.visible_project(self.selected_project_index).cloned() else {
            return false;
        };
        if !self
            .canonical_root_by_key
            .contains_key(&path_key(&project.root_path))
        {
            self.append_activity(&format!(
                "[{}] restore failed: versioning is still initializing for {}",
                now_human(),
                project.name
            ));
            self.status_message =
                format!("Still preparing {}; try again in a moment.", project.name);
            self.push_event(BackendEvent::StatusMessageChanged);
            return false;
        }

        let versions = self.get_project_versions(self.selected_project_index);
        let Some(mut version) = versions.iter().find(|v| v.id == version_id_str).cloned() else {
            return false;
        };
        // A version records only the files that changed in that save, so
        // restoring it alone would leave a multi-file project (a Logic or
        // GarageBand bundle) mixing old and new parts. Rebuild the artifact's
        // full state as of that version: the newest entry for each file up to
        // and including it.
        let mut state_at_version: Vec<VersionFileEntry> = Vec::new();
        for candidate in &versions {
            for file in &candidate.files {
                match state_at_version
                    .iter_mut()
                    .find(|existing| existing.path == file.path)
                {
                    Some(existing) => *existing = file.clone(),
                    None => state_at_version.push(file.clone()),
                }
            }
            if candidate.id == version.id {
                break;
            }
        }
        if !state_at_version.is_empty() {
            version.files = state_at_version;
        }
        // Every content hash this artifact has ever had. The worker compares
        // the file on disk against these at the moment it overwrites, so a
        // save made while this job waited in the queue is still captured
        // first.
        let known_object_hashes: Vec<String> = versions
            .iter()
            .flat_map(|version| version.files.iter())
            .filter(|file| !file.object_hash.is_empty())
            .map(|file| file.object_hash.clone())
            .collect();
        self.versioning.send(VersioningJob::Restore {
            project,
            version,
            known_object_hashes,
        });
        true
    }

    pub fn save_version_note(&mut self, version_id_str: &str, note: &str) -> bool {
        if version_id_str.is_empty() {
            return false;
        }
        let Some(project) = self.visible_project(self.selected_project_index).cloned() else {
            return false;
        };
        let log_path = join_path(&project.root_path, strings::MUSIT_VERSION_LOG_RELATIVE_PATH);

        // As in `delete_version_by_id`: one guard over the read and the
        // rewrite.
        let _log_guard = crate::metadata_store::log_write_guard();
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

        self.request_selected_versions();
        true
    }

    pub fn delete_version_by_id(&mut self, version_id_str: &str) -> bool {
        if version_id_str.is_empty() {
            return false;
        }
        let Some(project) = self.visible_project(self.selected_project_index).cloned() else {
            return false;
        };
        let log_path = join_path(&project.root_path, strings::MUSIT_VERSION_LOG_RELATIVE_PATH);

        // Held across the read and the rewrite: the versioning thread must
        // not append a snapshot into the copy this is about to replace.
        let _log_guard = crate::metadata_store::log_write_guard();
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
            if crate::object_store::is_staged_file(
                &join_path(&project.root_path, ".musit"),
                staged_path,
            ) && fs::remove_file(staged_path).is_ok()
            {
                remove_empty_parent_dirs(&parent_path(staged_path), &staged_root);
            }
        }

        // Remove compressed objects no other version references (objects are
        // content-addressed and can be shared between versions).
        let object_store = ObjectStore::new(join_path(&project.root_path, ".musit"));
        for object_hash in &deleted_object_hashes {
            if still_referenced_hashes.contains(object_hash) {
                continue;
            }
            if let Some(object_path) = object_store.object_path_for_hash(object_hash)
                && Path::new(&object_path).exists()
            {
                let _ = fs::remove_file(&object_path);
            }
        }

        self.append_activity(&format!(
            "[{}] deleted staged variation v{} for {}",
            now_human(),
            version_id_str,
            project.name
        ));
        self.request_selected_versions();
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

        self.workspaces.clear();
        self.active_workspace = -1;
        self.scan_queue.clear();
        self.pending_projects_folder_setup.clear();
        self.active_scan_folder.clear();
        self.project_root.clear();
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
        self.last_opened_at_by_project.clear();
        self.file_hash_cache.borrow_mut().clear();
        self.parsed_log_cache.borrow_mut().clear();

        self.theme_hue = 280.0;
        self.theme_hue_persist_deadline = None;
        self.theme_saturation = DEFAULT_THEME_SATURATION;
        self.theme_saturation_persist_deadline = None;
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
            BackendEvent::FoldersChanged,
            BackendEvent::ActiveFolderChanged,
            BackendEvent::ThemeHueChanged,
            BackendEvent::ThemeSaturationChanged,
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
            theme_saturation: self.theme_saturation,
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
        self.persist_theme_hue();
    }

    pub fn flush_pending_theme_saturation_persist(&mut self) {
        if self.theme_saturation_persist_deadline.take().is_some() {
            self.persist_app_settings();
        }
    }

    fn append_activity_with_level(&mut self, line: &str, level: LogLevel) {
        self.activity_all.push(line.to_string());
        self.activity_levels.push(level);
        let trimmed = self.trim_activity_log();
        let visible = !(self.log_level == LogLevel::Info && level == LogLevel::Debug);
        if trimmed {
            self.rebuild_visible_activity();
        } else if visible {
            // Append incrementally instead of re-cloning the filtered list.
            self.activity.push(line.to_string());
            self.push_event(BackendEvent::ActivityChanged);
        }
        // A filtered-out debug line changes nothing user-visible: no event,
        // so idle watcher chatter doesn't force re-renders.
    }

    fn log_config_change(&mut self, detail: &str) {
        self.append_debug_activity(&format!("[{}] config: {}", now_human(), detail));
    }

    fn trim_activity_log(&mut self) -> bool {
        if self.activity_all.len() <= MAX_ACTIVITY_LINES {
            return false;
        }

        let excess = self.activity_all.len() - MAX_ACTIVITY_LINES;
        self.activity_all.drain(..excess);
        if self.activity_levels.len() > excess {
            self.activity_levels.drain(..excess);
        } else {
            self.activity_levels.clear();
        }
        true
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
            issues.push(format!("Could not create config directory: {app_data_dir}"));
        } else {
            let probe_path = join_path(&app_data_dir, ".musit_write_probe");
            if fs::write(&probe_path, b"ok").is_err() {
                issues.push(format!("Config directory is not writable: {app_data_dir}"));
            } else {
                let _ = fs::remove_file(&probe_path);
            }
        }

        for saved_folder in self.project_registry.load_projects_folders() {
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
                    issues.push(format!(".musit storage is not writable in: {saved_folder}"));
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

    /// When the project was last opened: the time recorded when it was
    /// opened from here, else the primary file's modified time as the scan
    /// saw it (kept fresh by the watcher). Never touches the filesystem,
    /// which matters when the folder is on a network share.
    fn effective_last_opened(&self, project: &DiscoveredProject) -> Option<SystemTime> {
        if let Some(explicit) = self.last_opened_at_by_project.get(&project_key_of(project)) {
            return Some(*explicit);
        }
        project.primary_file_modified
    }

    /// Keeps the list's modified times fresh without stat calls: the watcher
    /// already knows the new time of every file it reports.
    fn note_primary_file_change(&mut self, watch_root: &str, event: &FileEvent) {
        if event.modified_ms <= 0 {
            return;
        }
        let Some(index) = self.workspace_index_by_root(watch_root) else {
            return;
        };
        let modified = std::time::UNIX_EPOCH + Duration::from_millis(event.modified_ms as u64);
        let mut changed = false;
        for project in &mut self.workspaces[index].discovered_projects {
            if project.primary_project_file.is_empty() {
                continue;
            }
            let primary = join_path(&project.root_path, &project.primary_project_file);
            if path_equals(&primary, &event.absolute_path)
                && project.primary_file_modified != Some(modified)
            {
                project.primary_file_modified = Some(modified);
                changed = true;
            }
        }
        if changed && index as i32 == self.active_workspace {
            self.rebuild_visible_projects();
        }
    }

    fn rebuild_visible_projects(&mut self) {
        let query = self.search_text.trim().to_lowercase();

        let projects: Vec<DiscoveredProject> = self.active_projects().to_vec();
        let mut visible: Vec<usize> = Vec::with_capacity(projects.len());
        for (i, project) in projects.iter().enumerate() {
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
        // No filesystem access here: modified times came with the scan and
        // the watcher keeps them current, so rebuilding the list is free
        // even when the folder sits on a slow share.
        let opened: Vec<Option<SystemTime>> = projects
            .iter()
            .map(|project| self.effective_last_opened(project))
            .collect();
        visible.sort_by(|&left, &right| {
            let l = &projects[left];
            let r = &projects[right];

            if sort_mode == SortMode::LastOpened {
                match (opened[left], opened[right]) {
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
        if !self.selected_project_key.is_empty() {
            let mut new_index = -1i32;
            for (i, &discovered_index) in self.visible_project_indexes.iter().enumerate() {
                if project_key_of(&projects[discovered_index]) == self.selected_project_key {
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
            let project = &projects[index];
            let effective_last_opened = opened[index];
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

    /// startMonitoringDeferred for one folder: queues per-project init,
    /// advanced in time-sliced batches from `process_pending`.
    fn start_monitoring_for_folder(&mut self, folder_root: &str) {
        let Some(index) = self.workspace_index_by_root(folder_root) else {
            return;
        };
        if self.workspaces[index].discovered_projects.is_empty() {
            return;
        }

        if let Some(watcher) = self.workspaces[index].watcher.take() {
            watcher.stop_watching();
        }
        self.workspaces[index].watcher_ready = false;
        self.drop_services_under(folder_root);

        let root = self.workspaces[index].root.clone();
        let projects = self.workspaces[index].discovered_projects.clone();
        self.monitoring_init_total += projects.len();
        if index as i32 == self.active_workspace {
            // The folder on screen goes to the head of the line: its versioning
            // must come up first even if a background folder queued hundreds of
            // projects on a slow share.
            for project in projects.into_iter().rev() {
                self.monitoring_init_queue
                    .push_front((root.clone(), project));
            }
        } else {
            for project in projects {
                self.monitoring_init_queue
                    .push_back((root.clone(), project));
            }
        }
        self.advance_monitoring_init();
    }

    /// Moves the active folder's pending init items to the front of the queue.
    fn prioritize_active_folder_monitoring(&mut self) {
        let Some(root) = self.active().map(|w| w.root.clone()) else {
            return;
        };
        if !self
            .monitoring_init_queue
            .iter()
            .any(|(folder, _)| path_equals(folder, &root))
        {
            return;
        }
        let (mine, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.monitoring_init_queue)
            .into_iter()
            .partition(|(folder, _)| path_equals(folder, &root));
        self.monitoring_init_queue = mine.into_iter().chain(others).collect();
    }

    /// Forgets the projects this folder brought in. Ownership is tracked
    /// explicitly, so tabs that happen to nest do not tear down each other's
    /// versioning.
    fn drop_services_under(&mut self, folder_root: &str) {
        let doomed: Vec<String> = self
            .folder_by_root_key
            .iter()
            .filter(|(_, owner)| path_equals(owner, folder_root))
            .map(|(key, _)| key.clone())
            .collect();
        for key in doomed {
            self.canonical_root_by_key.remove(&key);
            self.folder_by_root_key.remove(&key);
        }
        self.versioning.send(VersioningJob::DropUnder {
            folder_root: folder_root.to_string(),
        });
    }

    /// Hands the next queued project to the versioning thread, one at a
    /// time so the queue order (the active folder first) still holds; the
    /// reply drives the next step and the status line.
    fn advance_monitoring_init(&mut self) {
        if self.monitoring_init_in_flight.is_some() {
            return;
        }
        let Some((folder_root, project)) = self.monitoring_init_queue.pop_front() else {
            return;
        };
        self.monitoring_init_done += 1;
        let done = self.monitoring_init_done;
        let total = self.monitoring_init_total.max(done);
        self.set_status_for_folder(
            &folder_root,
            format!("Initializing versioning ({done}/{total})..."),
        );
        self.monitoring_init_in_flight = Some((folder_root.clone(), project.root_path.clone()));
        if !self.send_init_job(&folder_root, &project) {
            // Nothing will reply, so the queue would sit here forever.
            self.monitoring_init_in_flight = None;
            self.monitoring_init_queue.clear();
            self.monitoring_init_total = 0;
            self.monitoring_init_done = 0;
            self.set_status_for_folder(
                &folder_root,
                "Versioning stopped unexpectedly; restart Immersion.".to_string(),
            );
            self.append_activity(&format!(
                "[{}] versioning worker is not running; no further versions will be recorded",
                now_human()
            ));
        }
    }

    /// Queues one project's versioning set-up on the worker: history
    /// migration from enclosing projects, store init and seeding, all of
    /// which touch the (possibly remote) project folder.
    fn send_init_job(&mut self, folder_root: &str, project: &DiscoveredProject) -> bool {
        let mut ancestor_roots: Vec<String> = vec![folder_root.to_string()];
        if let Some(index) = self.workspace_index_by_root(folder_root) {
            for candidate in &self.workspaces[index].discovered_projects {
                if !path_equals(&candidate.root_path, &project.root_path)
                    && path_is_under_root(&project.root_path, &candidate.root_path)
                    && !ancestor_roots.contains(&candidate.root_path)
                {
                    ancestor_roots.push(candidate.root_path.clone());
                }
            }
        }
        ancestor_roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
        let generation = self
            .workspace_index_by_root(folder_root)
            .map(|index| self.workspaces[index].monitoring_generation)
            .unwrap_or(0);
        self.versioning.send(VersioningJob::Init {
            folder_root: folder_root.to_string(),
            generation,
            project: project.clone(),
            ancestor_roots,
            retention: self.snapshot_retention,
        })
    }

    /// Starts a watcher for every workspace that has monitored projects but
    /// no watcher yet (called when the init queue drains).
    fn start_watchers_when_ready(&mut self) {
        let roots: Vec<String> = self
            .workspaces
            .iter()
            .filter(|w| w.watcher.is_none() && !w.discovered_projects.is_empty())
            .map(|w| w.root.clone())
            .collect();
        for root in roots {
            self.start_watcher_for_folder(&root);
        }
    }

    fn start_watcher_for_folder(&mut self, folder_root: &str) {
        if self
            .workspace_index_by_root(folder_root)
            .is_none_or(|index| self.workspaces[index].watcher.is_some())
        {
            return;
        }
        let has_services = self
            .canonical_root_by_key
            .values()
            .any(|root| path_is_under_root(root, folder_root));
        if !has_services {
            self.set_status_for_folder(folder_root, "Failed to initialize versioning".to_string());
            self.append_activity(&format!(
                "[{}] versioning could not be initialized for {}",
                now_human(),
                folder_root
            ));
            return;
        }

        let event_tx = self.msg_tx.clone();
        let log_tx = self.msg_tx.clone();
        let ready_tx = self.msg_tx.clone();
        let watch_root = folder_root.to_string();
        let watcher = HybridFileWatcher::new(
            move |event| {
                let _ = event_tx.send(BackendMsg::WatcherEvent {
                    watch_root: watch_root.clone(),
                    event,
                });
            },
            Some(Box::new(
                move |scan_kind, root_path, item_count, elapsed| {
                    let _ = log_tx.send(BackendMsg::WatcherScanLog {
                        scan_kind: scan_kind.to_string(),
                        root_path: root_path.to_string(),
                        item_count,
                        elapsed_ms: elapsed,
                    });
                },
            )),
            Box::new(move |root_path| {
                let _ = ready_tx.send(BackendMsg::WatcherReady {
                    root_path: root_path.to_string(),
                });
            }),
        );

        if !watcher.start_watching(folder_root) {
            self.set_status_for_folder(folder_root, "Failed to start watcher".to_string());
            self.append_activity(&format!(
                "[{}] failed to start watcher for {}",
                now_human(),
                folder_root
            ));
            self.drop_services_under(folder_root);
            return;
        }
        if let Some(index) = self.workspace_index_by_root(folder_root) {
            self.workspaces[index].watcher = Some(watcher);
            self.workspaces[index].watcher_ready = false;
        }
    }

    fn stop_monitoring_for_folder(&mut self, folder_root: &str) {
        if let Some(index) = self.workspace_index_by_root(folder_root) {
            if let Some(watcher) = self.workspaces[index].watcher.take() {
                watcher.stop_watching();
            }
            self.workspaces[index].watcher_ready = false;
            self.workspaces[index].monitoring_generation += 1;
        }
        if self
            .monitoring_init_in_flight
            .as_ref()
            .is_some_and(|(folder, _)| path_equals(folder, folder_root))
        {
            self.monitoring_init_in_flight = None;
        }
        self.pending_adoptions
            .retain(|_, (folder, _)| !path_equals(folder, folder_root));
        let before = self.monitoring_init_queue.len();
        self.monitoring_init_queue
            .retain(|(root, _)| !path_equals(root, folder_root));
        self.monitoring_init_total = self
            .monitoring_init_total
            .saturating_sub(before - self.monitoring_init_queue.len());
        if self.monitoring_init_queue.is_empty() {
            self.monitoring_init_total = 0;
            self.monitoring_init_done = 0;
        }
        self.drop_services_under(folder_root);
        // Another folder's init may have been waiting behind the cancelled one.
        self.advance_monitoring_init();
    }

    fn stop_monitoring(&mut self) {
        for workspace in &mut self.workspaces {
            if let Some(watcher) = workspace.watcher.take() {
                watcher.stop_watching();
            }
            workspace.watcher_ready = false;
        }

        self.monitoring_init_queue.clear();
        self.monitoring_init_total = 0;
        self.monitoring_init_done = 0;
        for workspace in &mut self.workspaces {
            workspace.monitoring_generation += 1;
        }
        self.monitoring_init_in_flight = None;
        self.pending_adoptions.clear();
        self.canonical_root_by_key.clear();
        self.folder_by_root_key.clear();
        self.versioning.send(VersioningJob::DropAll);
        self.append_activity(&format!(
            "[{}] monitoring stopped",
            Local::now().format("%Y-%m-%dT%H:%M:%S")
        ));
    }

    fn contains_discovered_project(
        &self,
        workspace_index: usize,
        candidate: &DiscoveredProject,
    ) -> bool {
        let workspace = &self.workspaces[workspace_index];
        for project in &workspace.discovered_projects {
            if !path_equals(&project.root_path, &candidate.root_path) {
                continue;
            }
            if workspace.layout == ProjectsFolderLayout::Bundles
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

    /// A project the watcher found after the scan. Its versioning is set up
    /// on the worker first; the project is published (listed, saved) once
    /// that is done, so the main view never sees a project without its
    /// baseline. Returns true when the event was consumed.
    fn adopt_discovered_project(
        &mut self,
        workspace_index: usize,
        project: &DiscoveredProject,
    ) -> bool {
        if self.contains_discovered_project(workspace_index, project) {
            return false;
        }

        let mut adopted_projects = vec![project.clone()];
        self.apply_saved_primary_file_overrides(&mut adopted_projects);
        let adopted = adopted_projects.remove(0);
        let folder_root = self.workspaces[workspace_index].root.clone();
        let key = path_key(&adopted.root_path);

        if self.canonical_root_by_key.contains_key(&key) {
            return self.publish_adopted_project(workspace_index, adopted);
        }
        if self.pending_adoptions.contains_key(&key) {
            // Already on its way; the events that follow will be routed once
            // the project is published.
            return true;
        }
        self.pending_adoptions
            .insert(key, (folder_root.clone(), adopted.clone()));
        self.monitoring_init_total += 1;
        self.monitoring_init_queue.push_back((folder_root, adopted));
        self.advance_monitoring_init();
        true
    }

    /// Lists, saves and announces a project whose versioning is ready.
    fn publish_adopted_project(
        &mut self,
        workspace_index: usize,
        adopted: DiscoveredProject,
    ) -> bool {
        self.workspaces[workspace_index]
            .discovered_projects
            .push(adopted.clone());
        self.project_registry.save_project(&adopted);
        self.project_notes.insert(
            project_key_of(&adopted),
            self.project_registry
                .load_project_note(&adopted.root_path, &adopted.primary_project_file),
        );
        self.project_primary_files.insert(
            project_key_of(&adopted),
            adopted.primary_project_file.clone(),
        );
        if workspace_index as i32 == self.active_workspace {
            self.rebuild_visible_projects();
        }

        self.append_activity(&format!(
            "[{}] new project detected: {} ({})",
            now_human(),
            adopted.name,
            adopted.root_path
        ));
        true
    }

    fn try_discover_project_from_event(&mut self, watch_root: &str, event: &FileEvent) -> bool {
        let Some(workspace_index) = self.workspace_index_by_root(watch_root) else {
            return false;
        };
        if self.workspaces[workspace_index].scanning {
            return false;
        }

        if !matches!(
            event.event_type,
            FileEventType::Created | FileEventType::Modified
        ) {
            return false;
        }

        let Some(discovered) = project_discovery::discover_project_for_changed_path(
            &self.workspaces[workspace_index].root,
            &event.absolute_path,
            self.workspaces[workspace_index].layout,
        ) else {
            return false;
        };

        if self.contains_discovered_project(workspace_index, &discovered) {
            return false;
        }

        self.adopt_discovered_project(workspace_index, &discovered)
    }

    fn dispatch_file_event(&mut self, event: &FileEvent) -> bool {
        let normalized_absolute_path = normalize_absolute_path(&event.absolute_path);

        let mut matched_root_key = String::new();
        let mut matched_length = -1i64;
        // Only projects whose versioning is up (the worker reported InitDone)
        // are routing targets.
        for (root_key, canonical_root) in &self.canonical_root_by_key {
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
        self.versioning.send(VersioningJob::FileEvent {
            root_key: matched_root_key,
            event: translated_event,
        });
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
        self.flush_pending_theme_saturation_persist();
        self.cancel_project_scan();
        for workspace in &mut self.workspaces {
            if let Some(watcher) = workspace.watcher.take() {
                watcher.stop_watching();
            }
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
