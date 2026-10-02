//! Creates content-addressed snapshots for file events, groups bundle saves
//! into single versions, compacts old staged copies and tracks branch state.

use crate::backend::sha256_file_hex;
use crate::backup_template::{
    self, MAX_UNCOMPRESSED_RECENT_VERSIONS, MIN_UNCOMPRESSED_RECENT_VERSIONS,
    UNCOMPRESSED_RECENT_VERSIONS,
};
use crate::file_event::{FileEvent, FileEventType};
use crate::metadata_store::MetadataStore;
use crate::object_store::{ObjectStore, is_staged_file, write_atomically};
use crate::path_cleanup::{join_path, parent_path, relative_file_path, remove_empty_parent_dirs};
use crate::project_config::ProjectConfig;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

/// Notifications about snapshot activity, delivered through a callback sink.
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

const BUNDLE_SAVE_WINDOW_MS: i64 = 2_000;

/// How long an artifact must go unwritten before the startup check trusts
/// what is on disk, so a save still being written is not recorded torn.
const UNSEEN_QUIET: Duration = Duration::from_millis(1_000);
/// How long the startup check waits for a save in progress. A file still
/// changing by then is changing after the watcher's baseline, so the
/// watcher records it once it settles.
const UNSEEN_WAIT_LIMIT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
struct ActiveGroup {
    scan_sequence: i64,
    modified_ms: i64,
    paths: HashSet<String>,
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
        self.uncompressed_recent_versions = keep_count.clamp(
            MIN_UNCOMPRESSED_RECENT_VERSIONS,
            MAX_UNCOMPRESSED_RECENT_VERSIONS,
        );
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

        self.object_store
            .set_musit_root(&self.project_config.musit_path());
        self.metadata_store
            .set_musit_root(&self.project_config.musit_path());

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
        join_path(
            &self.project_config.musit_path(),
            "versions/branch-state.json",
        )
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
        root.insert(
            "branch_base_by_path".to_string(),
            Value::Object(by_artifact),
        );

        let encoded = serde_json::to_string(&Value::Object(root)).expect("branch state serializes");
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

        // Stored from the staged copy, not from the original: the DAW may
        // write again while this runs, and a version whose object holds
        // different bytes than its staged copy would restore differently
        // before and after compaction.
        let Some(object_hash) = self.object_store.store_file(&staged_path) else {
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

        // Most files of one save share a watcher scan sequence. On busy
        // filesystems a bundle's next file can settle in a later scan; keep
        // it in the same version if its write time is close and that path
        // has not already appeared in the group. A repeated path starts a
        // new save even when saves happen quickly.
        let group_hit = event.scan_sequence != 0
            && self
                .active_group_by_artifact
                .get(&artifact)
                .is_some_and(|entry| {
                    entry.group.scan_sequence == event.scan_sequence
                        || (event.scan_sequence > 0
                            && entry.group.scan_sequence > 0
                            && event.modified_ms > 0
                            && entry.group.modified_ms > 0
                            && (event.modified_ms - entry.group.modified_ms).abs()
                                <= BUNDLE_SAVE_WINDOW_MS
                            && !entry.group.paths.contains(&event.relative_path))
                });

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
            // A log that cannot be read in full would produce a version id
            // that is already in use, so the snapshot is abandoned with the
            // staged copy removed; the next save tries again.
            let Some((latest_staged_version, last_object)) = self
                .metadata_store
                .latest_version_and_last_object(&artifact, &event.relative_path)
            else {
                discard_staged_copy(&staged_path, &self.project_config.musit_path());
                self.emit(SnapshotNotice::Error(format!(
                    "Could not read the version log for {}; save not recorded",
                    event.relative_path
                )));
                return false;
            };

            // A save whose content its last entry already holds adds
            // nothing, as when the watcher reports a save the startup check
            // has just recorded. Only standalone files: a bundle's version
            // is made of the files saved together, and dropping one would
            // leave it out.
            let unchanged = !is_baseline
                && artifact == event.relative_path
                && last_object.is_some_and(|last| last.eq_ignore_ascii_case(&object_hash));
            if unchanged {
                discard_staged_copy(&staged_path, &self.project_config.musit_path());
                self.emit(SnapshotNotice::Skipped(format!(
                    "Skipped {} (unchanged since its last version)",
                    event.relative_path
                )));
                return false;
            }
            let branch_base_version =
                resolve_branch_base_version(&explicit_branch_base, &latest_staged_version);
            if !explicit_branch_base.is_empty() && explicit_branch_base == latest_staged_version {
                self.branch_base_by_artifact.remove(&artifact);
                self.save_branch_state();
            }

            let Some(next_version_id) = self
                .metadata_store
                .next_version_id_for_artifact(&artifact, &branch_base_version)
            else {
                discard_staged_copy(&staged_path, &self.project_config.musit_path());
                self.emit(SnapshotNotice::Error(format!(
                    "Could not read the version log for {}; save not recorded",
                    event.relative_path
                )));
                return false;
            };
            version_id = next_version_id;
            parent_version = branch_base_version;
            self.active_group_by_artifact.insert(
                artifact.clone(),
                ActiveGroupEntry {
                    group: ActiveGroup {
                        scan_sequence: event.scan_sequence,
                        modified_ms: event.modified_ms,
                        paths: HashSet::new(),
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

        if let Some(entry) = self.active_group_by_artifact.get_mut(&artifact) {
            entry.group.paths.insert(event.relative_path.clone());
        }

        self.compact_staged_copies(&event.relative_path);

        self.emit(SnapshotNotice::Created(format!(
            "{} {} -> {} (v{})",
            if is_baseline {
                "Baseline snapshot"
            } else {
                "Snapshot"
            },
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
                || (cfg!(windows) && staged_path.chars().nth(1) == Some(':'))
            {
                staged_path.clone()
            } else {
                join_path(root, &staged_path)
            };

            if is_staged_file(&self.project_config.musit_path(), &absolute_staged_path)
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
                modified_ms: 0,
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
                modified_ms: 0,
            };
            any_succeeded = self.create_snapshot(&event, true) || any_succeeded;
        }

        any_succeeded
    }

    /// Records a save the watcher could not have seen: Immersion was closed,
    /// or the save landed before the watcher's baseline scan, which takes
    /// whatever is on disk as unchanged. A file of the artifact (every
    /// tracked file of a bundle) whose content matches no object recorded
    /// for its path becomes one new version. Content matching an older
    /// version, such as one restored, is left alone.
    pub fn record_unseen_changes(&mut self, absolute_path: &str, relative_path: &str) -> bool {
        let Some(known) = self.metadata_store.object_hashes_by_path() else {
            return false;
        };
        let Some(files) = self.settled_contents(absolute_path, relative_path) else {
            return false;
        };

        // One synthetic sequence, so the changed files share one version.
        self.synthetic_scan_sequence -= 1;
        let scan_sequence = self.synthetic_scan_sequence;
        let mut recorded = false;
        for (absolute, relative, on_disk) in files {
            let captured = on_disk.is_empty()
                || known
                    .get(&relative)
                    .is_some_and(|hashes| hashes.contains(&on_disk));
            if captured {
                continue;
            }
            let event = FileEvent {
                event_type: FileEventType::Modified,
                absolute_path: absolute,
                relative_path: relative,
                scan_sequence,
                modified_ms: 0,
            };
            recorded = self.create_snapshot(&event, false) || recorded;
        }
        recorded
    }

    /// The tracked files of an artifact with their content hashes, once no
    /// file has been written for [`UNSEEN_QUIET`] and none changed while it
    /// was read. None when the artifact is gone or did not settle within
    /// [`UNSEEN_WAIT_LIMIT`].
    fn settled_contents(
        &self,
        absolute_path: &str,
        relative_path: &str,
    ) -> Option<Vec<(String, String, String)>> {
        let deadline = Instant::now() + UNSEEN_WAIT_LIMIT;
        loop {
            let files = self.artifact_files(absolute_path, relative_path)?;
            let before = stamps_of(&files);
            // A modification time ahead of the clock is skew, not a write.
            let recently_written = before.as_ref().is_some_and(|stamps| {
                stamps.iter().any(|(_, modified)| {
                    SystemTime::now()
                        .duration_since(*modified)
                        .is_ok_and(|age| age < UNSEEN_QUIET)
                })
            });
            if before.is_some() && !recently_written {
                let contents: Vec<(String, String, String)> = files
                    .iter()
                    .map(|(absolute, relative)| {
                        (
                            absolute.clone(),
                            relative.clone(),
                            sha256_file_hex(absolute),
                        )
                    })
                    .collect();
                let unchanged = self
                    .artifact_files(absolute_path, relative_path)
                    .is_some_and(|after| after == files && stamps_of(&after) == before);
                if unchanged {
                    return Some(contents);
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(UNSEEN_QUIET / 4);
        }
    }

    /// The tracked files of an artifact: the file itself, or every tracked
    /// file inside a bundle, as (absolute, relative) paths.
    fn artifact_files(
        &self,
        absolute_path: &str,
        relative_path: &str,
    ) -> Option<Vec<(String, String)>> {
        let path = Path::new(absolute_path);
        if path.is_dir() {
            let mut files: Vec<(String, String)> = walk_files(absolute_path)
                .into_iter()
                .map(|file| {
                    let inner = relative_file_path(absolute_path, &file);
                    (file, join_path(relative_path, &inner))
                })
                .filter(|(_, relative)| self.project_config.should_track(relative))
                .collect();
            files.sort();
            Some(files)
        } else if path.is_file() {
            Some(vec![(absolute_path.to_string(), relative_path.to_string())])
        } else {
            None
        }
    }

    pub fn has_version_for_artifact(&self, artifact: &str) -> bool {
        if artifact.is_empty() {
            return false;
        }
        // An unreadable log is treated as "has history", so seeding cannot
        // add a duplicate first version on top of one that already exists.
        self.metadata_store
            .latest_staged_version_for_artifact(artifact)
            .is_none_or(|latest| !latest.is_empty())
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
        // Deletions count too: replacing a file briefly removes it, and that
        // gap must not be recorded as the user deleting their project.
        {
            if let Some(&pending) = self.suppressed_events_by_path.get(&event.relative_path)
                && pending > 0
            {
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

/// Size and modification time of each file, or None when one cannot be
/// read (it was replaced or removed in between).
fn stamps_of(files: &[(String, String)]) -> Option<Vec<(u64, SystemTime)>> {
    files
        .iter()
        .map(|(absolute, _)| {
            let metadata = fs::metadata(absolute).ok()?;
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            Some((metadata.len(), modified))
        })
        .collect()
}

/// Recursive file walk including hidden files.
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
            // Never follow symlinks:
            // a symlinked dir inside a bundle could form a cycle and hang.
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(path_str);
            } else if file_type.is_file() {
                out.push(path_str);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_files_from_adjacent_scans_share_one_save() {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path().join("MySong");
        let root = project_root.to_string_lossy().to_string();
        let mut service = SnapshotService::new();
        assert!(service.set_project_root(&root));

        let files = [
            "Song.logicx/Metadata.plist",
            "Song.logicx/Alternatives/000/ProjectData",
        ];
        for save in 1..=2 {
            for (index, relative) in files.iter().enumerate() {
                let path = project_root.join(relative);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, format!("save {save}, file {index}")).unwrap();
                service.on_file_event(&FileEvent {
                    event_type: FileEventType::Modified,
                    absolute_path: path.to_string_lossy().to_string(),
                    relative_path: relative.to_string(),
                    scan_sequence: save * 10 + index as i64,
                    modified_ms: save * 3_000 + index as i64 * 800,
                });
            }
        }

        let log = join_path(
            &root,
            crate::backend::strings::MUSIT_VERSION_LOG_RELATIVE_PATH,
        );
        let versions = crate::backend::parse_versions_from_log(&log, "Song.logicx");
        assert_eq!(versions.len(), 2);
        assert!(versions.iter().all(|version| version.files.len() == 2));
    }

    /// Writes a file as a save that finished a while ago, so the startup
    /// check does not wait for it to settle.
    fn write_settled(path: &Path, content: &str) {
        fs::write(path, content).unwrap();
        let earlier = SystemTime::now() - Duration::from_secs(60);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
    }

    #[test]
    fn startup_check_waits_for_a_save_in_progress() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_string_lossy().to_string();
        let song = temp.path().join("Song.als");
        let song_path = song.to_string_lossy().to_string();

        write_settled(&song, "first");
        let mut service = SnapshotService::new();
        assert!(service.set_project_root(&root));
        assert!(service.snapshot_path_now(&song_path, "Song.als"));

        // The DAW is mid-save: part written now, the rest shortly after.
        fs::write(&song, "sec").unwrap();
        let writer = {
            let song = song.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                fs::write(&song, "second").unwrap();
            })
        };
        assert!(service.record_unseen_changes(&song_path, "Song.als"));
        writer.join().unwrap();

        let versions = versions_of(&root, "Song.als");
        assert_eq!(versions.len(), 2);
        let finished = sha256_file_hex(&song_path);
        assert!(
            versions
                .iter()
                .any(|version| { version.files[0].object_hash.eq_ignore_ascii_case(&finished) })
        );
    }

    fn versions_of(root: &str, artifact: &str) -> Vec<crate::backend::VersionEntry> {
        let log = join_path(
            root,
            crate::backend::strings::MUSIT_VERSION_LOG_RELATIVE_PATH,
        );
        crate::backend::parse_versions_from_log(&log, artifact)
    }

    #[test]
    fn save_made_while_closed_is_recorded_once() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_string_lossy().to_string();
        let song = temp.path().join("Song.als");
        let song_path = song.to_string_lossy().to_string();

        write_settled(&song, "first");
        let mut service = SnapshotService::new();
        assert!(service.set_project_root(&root));
        assert!(service.snapshot_path_now(&song_path, "Song.als"));

        // Saved while Immersion was not running; a new session starts.
        write_settled(&song, "second");
        let mut service = SnapshotService::new();
        assert!(service.set_project_root(&root));
        assert!(service.record_unseen_changes(&song_path, "Song.als"));
        assert!(!service.record_unseen_changes(&song_path, "Song.als"));
        assert_eq!(versions_of(&root, "Song.als").len(), 2);

        // Content of an earlier version (a restore) is already captured.
        write_settled(&song, "first");
        assert!(!service.record_unseen_changes(&song_path, "Song.als"));
        assert_eq!(versions_of(&root, "Song.als").len(), 2);
    }

    #[test]
    fn watcher_report_of_a_recorded_save_is_not_recorded_again() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_string_lossy().to_string();
        let song = temp.path().join("Song.als");
        let song_path = song.to_string_lossy().to_string();
        let watcher_event = |sequence| FileEvent {
            event_type: FileEventType::Modified,
            absolute_path: song_path.clone(),
            relative_path: "Song.als".to_string(),
            scan_sequence: sequence,
            modified_ms: 0,
        };

        write_settled(&song, "first");
        let mut service = SnapshotService::new();
        assert!(service.set_project_root(&root));
        assert!(service.snapshot_path_now(&song_path, "Song.als"));

        // Saved during startup: the content check records it, then the
        // watcher reports the same save.
        write_settled(&song, "second");
        assert!(service.record_unseen_changes(&song_path, "Song.als"));
        service.on_file_event(&watcher_event(1));
        assert_eq!(versions_of(&root, "Song.als").len(), 2);

        // A real change after it is recorded as usual.
        write_settled(&song, "third");
        service.on_file_event(&watcher_event(2));
        assert_eq!(versions_of(&root, "Song.als").len(), 3);

        // As is going back to earlier content.
        write_settled(&song, "first");
        service.on_file_event(&watcher_event(3));
        assert_eq!(versions_of(&root, "Song.als").len(), 4);
    }

    #[test]
    fn unseen_bundle_changes_share_one_version() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_string_lossy().to_string();
        let bundle = temp.path().join("Song.logicx");
        let bundle_path = bundle.to_string_lossy().to_string();
        let files = ["Metadata.plist", "Alternatives/000/ProjectData"];
        let write_all = |save: &str| {
            for file in files {
                let path = bundle.join(file);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                write_settled(&path, &format!("{save} {file}"));
            }
        };

        write_all("first");
        let mut service = SnapshotService::new();
        assert!(service.set_project_root(&root));
        assert!(service.snapshot_path_now(&bundle_path, "Song.logicx"));
        assert!(!service.record_unseen_changes(&bundle_path, "Song.logicx"));

        write_all("second");
        assert!(service.record_unseen_changes(&bundle_path, "Song.logicx"));
        let versions = versions_of(&root, "Song.logicx");
        assert_eq!(versions.len(), 2);
        assert!(versions.iter().all(|version| version.files.len() == 2));
    }
}
