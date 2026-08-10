//! Creates content-addressed snapshots for file events, groups bundle saves
//! into single versions, compacts old staged copies and tracks branch state.
//! Port of `qt-legacy/src/core/SnapshotService.{h,cpp}`.

use crate::backup_template::{
    self, MAX_UNCOMPRESSED_RECENT_VERSIONS, MIN_UNCOMPRESSED_RECENT_VERSIONS,
    UNCOMPRESSED_RECENT_VERSIONS,
};
use crate::file_event::{FileEvent, FileEventType};
use crate::metadata_store::MetadataStore;
use crate::object_store::{write_atomically, ObjectStore};
use crate::path_cleanup::{join_path, parent_path, relative_file_path, remove_empty_parent_dirs};
use crate::project_config::ProjectConfig;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Signals the Qt version emitted; delivered through a callback sink here.
#[derive(Clone, Debug)]
pub enum SnapshotNotice {
    /// A snapshot (or deletion record) was written. Human-readable message.
    Created(String),
    /// Emitted once per user save (not baselines/seeds), including the first
    /// file of a grouped bundle save.
    SaveRecorded {
        version_id: String,
        relative_path: String,
    },
    Skipped(String),
    Error(String),
}

fn resolve_branch_base_version(explicit_branch_base: &str, latest_staged_version: &str) -> String {
    let mut branch_base_version = explicit_branch_base.to_string();
    if branch_base_version.is_empty() {
        branch_base_version = latest_staged_version.to_string();
    }

    // If we are at the latest version, continue normal top-level numbering.
    if !branch_base_version.is_empty()
        && !latest_staged_version.is_empty()
        && branch_base_version == latest_staged_version
    {
        branch_base_version.clear();
    }

    branch_base_version
}

#[derive(Clone, Copy, Debug)]
struct ActiveGroup {
    scan_sequence: i64,
    // version_id / parent_version are stored alongside in the map value.
}

#[derive(Clone, Debug)]
struct ActiveGroupEntry {
    group: ActiveGroup,
    version_id: String,
    parent_version: String,
}

pub struct SnapshotService {
    project_config: ProjectConfig,
    object_store: ObjectStore,
    metadata_store: MetadataStore,
    suppressed_events_by_path: HashMap<String, i32>,
    branch_base_by_artifact: HashMap<String, String>,
    active_group_by_artifact: HashMap<String, ActiveGroupEntry>,
    synthetic_scan_sequence: i64,
    uncompressed_recent_versions: i32,
    notice_sink: Option<Box<dyn Fn(SnapshotNotice) + Send>>,
}

impl SnapshotService {
    pub fn new() -> Self {
        Self {
            project_config: ProjectConfig::default(),
            object_store: ObjectStore::default(),
            metadata_store: MetadataStore::default(),
            suppressed_events_by_path: HashMap::new(),
            branch_base_by_artifact: HashMap::new(),
            active_group_by_artifact: HashMap::new(),
            synthetic_scan_sequence: 0,
            uncompressed_recent_versions: UNCOMPRESSED_RECENT_VERSIONS,
            notice_sink: None,
        }
    }

    pub fn set_notice_sink(&mut self, sink: impl Fn(SnapshotNotice) + Send + 'static) {
        self.notice_sink = Some(Box::new(sink));
    }

    fn emit(&self, notice: SnapshotNotice) {
        if let Some(sink) = &self.notice_sink {
            sink(notice);
        }
    }

    pub fn set_uncompressed_recent_versions(&mut self, keep_count: i32) {
        self.uncompressed_recent_versions = keep_count
            .clamp(MIN_UNCOMPRESSED_RECENT_VERSIONS, MAX_UNCOMPRESSED_RECENT_VERSIONS);
    }

    pub fn uncompressed_recent_versions(&self) -> i32 {
        self.uncompressed_recent_versions
    }

    pub fn project_root(&self) -> &str {
        self.project_config.root_path()
    }

    pub fn musit_path(&self) -> String {
        self.project_config.musit_path()
    }

    pub fn metadata(&self) -> &MetadataStore {
        &self.metadata_store
    }

    pub fn objects(&self) -> &ObjectStore {
        &self.object_store
    }

    pub fn compact_all_staged_copies(&self) {
        for relative_path in self.metadata_store.snapshot_paths() {
            self.compact_staged_copies(&relative_path);
        }
    }

    pub fn set_project_root(&mut self, root_path: &str) -> bool {
        self.project_config.set_root_path(root_path);

        if !self.project_config.is_ready() {
            return false;
        }

        self.object_store.set_musit_root(&self.project_config.musit_path());
        self.metadata_store.set_musit_root(&self.project_config.musit_path());

        if !self.object_store.init() || !self.metadata_store.init() {
            return false;
        }

        self.load_branch_state();
        true
    }

    fn branch_state_file_path(&self) -> String {
        if !self.project_config.is_ready() {
            return String::new();
        }
        join_path(&self.project_config.musit_path(), "versions/branch-state.json")
    }

    fn load_branch_state(&mut self) {
        self.branch_base_by_artifact.clear();

        let file_path = self.branch_state_file_path();
        let Ok(bytes) = fs::read(&file_path) else {
            return;
        };
        let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
            return;
        };
        let Some(obj) = doc.get("branch_base_by_path").and_then(|v| v.as_object()) else {
            return;
        };
        for (key, value) in obj {
            let version_id = value.as_str().unwrap_or("");
            if !version_id.is_empty() {
                self.branch_base_by_artifact
                    .insert(key.clone(), version_id.to_string());
            }
        }
    }

    fn save_branch_state(&self) {
        let file_path = self.branch_state_file_path();
        if file_path.is_empty() {
            return;
        }

        let mut by_artifact = Map::new();
        for (key, value) in &self.branch_base_by_artifact {
            by_artifact.insert(key.clone(), Value::String(value.clone()));
        }

        let mut root = Map::new();
        root.insert("branch_base_by_path".to_string(), Value::Object(by_artifact));

        let encoded =
            serde_json::to_string(&Value::Object(root)).expect("branch state serializes");
        write_atomically(&file_path, encoded.as_bytes());
    }

    fn create_snapshot(&mut self, event: &FileEvent, is_baseline: bool) -> bool {
        let label = if is_baseline { "baseline " } else { "" };

        if !self.project_config.should_track(&event.relative_path) {
            self.emit(SnapshotNotice::Skipped(format!(
                "Skipped {label}{} (excluded)",
                event.relative_path
            )));
            return false;
        }

        let info = Path::new(&event.absolute_path);
        if !info.is_file() {
            self.emit(SnapshotNotice::Skipped(format!(
                "Skipped {label}{} (missing or not a file)",
                event.relative_path
            )));
            return false;
        }

        let Some(staged_path) = self
            .object_store
            .stage_file(&event.absolute_path, &event.relative_path)
        else {
            self.emit(SnapshotNotice::Error(format!(
                "Failed to stage {label}{}",
                event.relative_path
            )));
            return false;
        };

        let discard_staged_copy = |staged_path: &str, musit_path: &str| {
            if fs::remove_file(staged_path).is_err() {
                return;
            }
            let staging_root = join_path(musit_path, "staging");
            remove_empty_parent_dirs(&parent_path(staged_path), &staging_root);
        };

        let Some(object_hash) = self.object_store.store_file(&event.absolute_path) else {
            discard_staged_copy(&staged_path, &self.project_config.musit_path());
            self.emit(SnapshotNotice::Error(format!(
                "Failed to store {label}object for {}",
                event.relative_path
            )));
            return false;
        };

        // Store the staged path relative to the project root so version
        // history survives moving or renaming the project folder.
        let relative_staged_path =
            relative_file_path(self.project_config.root_path(), &staged_path);

        // Files of the same artifact stabilizing in the same watcher scan (or
        // the same synthetic batch) share one version id.
        let artifact = backup_template::artifact_for_path(&event.relative_path);
        let version_id: String;
        let parent_version: String;

        let group_hit = event.scan_sequence != 0
            && self
                .active_group_by_artifact
                .get(&artifact)
                .is_some_and(|entry| entry.group.scan_sequence == event.scan_sequence);

        if group_hit {
            let entry = &self.active_group_by_artifact[&artifact];
            version_id = entry.version_id.clone();
            parent_version = entry.parent_version.clone();
        } else {
            let explicit_branch_base = self
                .branch_base_by_artifact
                .get(&artifact)
                .cloned()
                .unwrap_or_default();
            let latest_staged_version =
                self.metadata_store.latest_staged_version_for_artifact(&artifact);
            let branch_base_version =
                resolve_branch_base_version(&explicit_branch_base, &latest_staged_version);
            if !explicit_branch_base.is_empty() && explicit_branch_base == latest_staged_version {
                self.branch_base_by_artifact.remove(&artifact);
                self.save_branch_state();
            }

            version_id = self
                .metadata_store
                .next_version_id_for_artifact(&artifact, &branch_base_version);
            parent_version = branch_base_version;
            self.active_group_by_artifact.insert(
                artifact.clone(),
                ActiveGroupEntry {
                    group: ActiveGroup {
                        scan_sequence: event.scan_sequence,
                    },
                    version_id: version_id.clone(),
                    parent_version: parent_version.clone(),
                },
            );

            if !is_baseline {
                self.emit(SnapshotNotice::SaveRecorded {
                    version_id: version_id.clone(),
                    relative_path: event.relative_path.clone(),
                });
            }
        }

        let appended = self.metadata_store.append_snapshot_event(
            event,
            &artifact,
            &object_hash,
            &relative_staged_path,
            &version_id,
            &parent_version,
        );
        if !appended {
            discard_staged_copy(&staged_path, &self.project_config.musit_path());
            self.emit(SnapshotNotice::Error(format!(
                "Failed to append {label}metadata for {}",
                event.relative_path
            )));
            return false;
        }

        self.compact_staged_copies(&event.relative_path);

        self.emit(SnapshotNotice::Created(format!(
            "{} {} -> {} (v{})",
            if is_baseline { "Baseline snapshot" } else { "Snapshot" },
            event.relative_path,
            &object_hash[..12.min(object_hash.len())],
            version_id
        )));
        true
    }

    fn compact_staged_copies(&self, relative_path: &str) {
        // Only the newest versions keep a fast uncompressed staged copy;
        // older versions live on solely as compressed objects and are
        // decompressed on demand when restored.
        let old_staged_paths = self
            .metadata_store
            .staged_paths_beyond_newest(relative_path, self.uncompressed_recent_versions);
        if old_staged_paths.is_empty() {
            return;
        }

        let root = self.project_config.root_path();
        let staging_root = join_path(&self.project_config.musit_path(), "staging");

        for staged_path in old_staged_paths {
            let absolute_staged_path = if staged_path.starts_with('/')
                || staged_path.chars().nth(1) == Some(':')
            {
                staged_path.clone()
            } else {
                join_path(root, &staged_path)
            };

            if Path::new(&absolute_staged_path).exists()
                && fs::remove_file(&absolute_staged_path).is_ok()
            {
                remove_empty_parent_dirs(&parent_path(&absolute_staged_path), &staging_root);
            }
        }
    }

    /// Snapshots a file, or every tracked file inside a directory bundle
    /// (.logicx/.band) as one grouped version. Used for baselines and for
    /// preserving unversioned state before a restore.
    pub fn snapshot_path_now(&mut self, absolute_path: &str, relative_path: &str) -> bool {
        let info = Path::new(absolute_path);

        if !info.is_dir() {
            self.synthetic_scan_sequence -= 1;
            let event = FileEvent {
                event_type: FileEventType::Modified,
                absolute_path: absolute_path.to_string(),
                relative_path: relative_path.to_string(),
                scan_sequence: self.synthetic_scan_sequence,
            };
            return self.create_snapshot(&event, true);
        }

        // Directory bundle: snapshot every tracked file inside it as one
        // grouped version (negative sequences never collide with watcher ones).
        self.synthetic_scan_sequence -= 1;
        let batch_sequence = self.synthetic_scan_sequence;
        let mut any_succeeded = false;

        for file_absolute_path in walk_files(absolute_path) {
            let inner_relative = relative_file_path(absolute_path, &file_absolute_path);
            let file_relative_path = join_path(relative_path, &inner_relative);

            if !self.project_config.should_track(&file_relative_path) {
                continue;
            }

            let event = FileEvent {
                event_type: FileEventType::Modified,
                absolute_path: file_absolute_path,
                relative_path: file_relative_path,
                scan_sequence: batch_sequence,
            };
            any_succeeded = self.create_snapshot(&event, true) || any_succeeded;
        }

        any_succeeded
    }

    pub fn has_version_for_artifact(&self, artifact: &str) -> bool {
        if artifact.is_empty() {
            return false;
        }
        !self
            .metadata_store
            .latest_staged_version_for_artifact(artifact)
            .is_empty()
    }

    pub fn suppress_next_events_for_path(&mut self, relative_path: &str, count: i32) {
        if relative_path.is_empty() || count <= 0 {
            return;
        }
        *self
            .suppressed_events_by_path
            .entry(relative_path.to_string())
            .or_insert(0) += count;
    }

    pub fn set_branch_base_for_artifact(&mut self, artifact: &str, version_id: &str) {
        if artifact.is_empty() {
            return;
        }

        if version_id.is_empty() {
            self.branch_base_by_artifact.remove(artifact);
        } else {
            self.branch_base_by_artifact
                .insert(artifact.to_string(), version_id.to_string());
        }
        self.save_branch_state();
    }

    pub fn on_file_event(&mut self, event: &FileEvent) {
        if matches!(
            event.event_type,
            FileEventType::Created | FileEventType::Modified
        ) {
            if let Some(&pending) = self.suppressed_events_by_path.get(&event.relative_path) {
                if pending > 0 {
                    let remaining = pending - 1;
                    if remaining > 0 {
                        self.suppressed_events_by_path
                            .insert(event.relative_path.clone(), remaining);
                    } else {
                        self.suppressed_events_by_path.remove(&event.relative_path);
                    }

                    self.emit(SnapshotNotice::Skipped(format!(
                        "Suppressed self-triggered update: {}",
                        event.relative_path
                    )));
                    return;
                }
            }
        }

        if !self.project_config.should_track(&event.relative_path) {
            self.emit(SnapshotNotice::Skipped(format!(
                "Skipped {} (excluded)",
                event.relative_path
            )));
            return;
        }

        if event.event_type == FileEventType::Deleted {
            let artifact = backup_template::artifact_for_path(&event.relative_path);
            let appended = self
                .metadata_store
                .append_snapshot_event(event, &artifact, "", "", "", "");
            if appended {
                self.emit(SnapshotNotice::Created(format!(
                    "Recorded deletion: {}",
                    event.relative_path
                )));
            } else {
                self.emit(SnapshotNotice::Error(format!(
                    "Failed to record deletion: {}",
                    event.relative_path
                )));
            }
            return;
        }

        self.create_snapshot(event, false);
    }
}

impl Default for SnapshotService {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursive file walk including hidden files (QDirIterator equivalent).
fn walk_files(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_string()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let path_str = path.to_string_lossy().replace('\\', "/");
            if path.is_dir() {
                stack.push(path_str);
            } else if path.is_file() {
                out.push(path_str);
            }
        }
    }
    out
}
