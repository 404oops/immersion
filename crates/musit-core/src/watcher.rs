//! Hybrid file watcher: OS notifications (via the `notify` crate; FSEvents /
//! ReadDirectoryChangesW / inotify) marking paths dirty, a 400ms debounce
//! with size+mtime stabilization, and a 5s full-tree safety scan.
//!
//! Files that stabilize in the same debounce tick share a scan sequence,
//! which SnapshotService uses to group bundle saves.

use crate::file_event::{FileEvent, FileEventType};
use crate::path_cleanup::{clean_path, join_path, path_equals, relative_file_path};
use crate::project_config::{self, ProjectConfig};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, UNIX_EPOCH};

const DEBOUNCE: Duration = Duration::from_millis(400);
const SAFETY_SCAN: Duration = Duration::from_millis(5000);

/// (scan_kind, root_path, item_count, elapsed_ms)
pub type ScanLogFn = Box<dyn Fn(&str, &str, usize, u128) + Send>;
pub type EventFn = Box<dyn Fn(FileEvent) + Send>;
pub type ReadyFn = Box<dyn Fn(&str) + Send>;

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileState {
    size: u64,
    msecs_since_epoch: i64,
}

/// What a look at a path found.
enum Observed {
    File(FileState),
    /// The path is gone (or is no longer a file).
    Missing,
    /// The look itself failed: the volume may be busy or disconnected, and
    /// the path must not be treated as deleted on that basis.
    Unknown,
}

fn state_of(path: &Path) -> Observed {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Observed::Missing,
        Err(_) => return Observed::Unknown,
    };
    if !metadata.is_file() {
        return Observed::Missing;
    }
    let msecs = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Observed::File(FileState {
        size: metadata.len(),
        msecs_since_epoch: msecs,
    })
}

/// Collects the state of every tracked file under `current`. Returns false
/// when any directory could not be read, so callers can tell a genuinely
/// empty tree from one the filesystem would not show them.
fn scan_tree_state(
    root: &str,
    current: &str,
    config: &ProjectConfig,
    out: &mut HashMap<String, FileState>,
) -> bool {
    let Ok(entries) = fs::read_dir(current) else {
        return false;
    };
    let mut complete = true;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        // NoSymLinks parity: skip symlinks entirely.
        if file_type.is_symlink() {
            continue;
        }
        let absolute = clean_path(&entry.path().to_string_lossy());
        if file_type.is_dir() {
            if project_config::is_ignored_directory_name(&name) {
                continue;
            }
            complete &= scan_tree_state(root, &absolute, config, out);
            continue;
        }
        let relative = relative_file_path(root, &absolute);
        // Only tracked files are worth remembering: `mark_dirty_relative`
        // gates every event on `should_track`, so an untracked entry can
        // never produce output — it would only hold memory (a projects
        // folder full of samples/renders dwarfs its tracked files).
        if !config.should_track(&relative) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let msecs = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        out.insert(
            relative,
            FileState {
                size: metadata.len(),
                msecs_since_epoch: msecs,
            },
        );
    }
    complete
}

enum Msg {
    Start(String),
    Stop,
    Fs(Vec<std::path::PathBuf>),
    Shutdown,
}

struct WorkerState {
    root_path: String,
    /// Symlink-resolved form of `root_path` when they differ ("" otherwise).
    /// FSEvents reports resolved paths, so events for a root behind a
    /// symlinked prefix (/tmp -> /private/tmp) arrive in this domain.
    canonical_root: String,
    project_config: ProjectConfig,
    scan_sequence: i64,
    /// The next batch continues the save the last one was still watching
    /// settle, so it keeps the same sequence.
    continuing_batch: bool,
    dirty_relative: HashSet<String>,
    /// Last observed state per TRACKED file (scan_tree_state filters by
    /// should_track, and only tracked paths ever get marked dirty), so the
    /// map stays proportional to project files, not folder contents.
    previous: HashMap<String, FileState>,
    pending: HashMap<String, FileState>,
    debounce_deadline: Option<Instant>,
    safety_deadline: Option<Instant>,
    on_event: EventFn,
    log_scan: Option<ScanLogFn>,
}

impl WorkerState {
    fn log(&self, kind: &str, item_count: usize, elapsed_ms: u128) {
        if let Some(log) = &self.log_scan {
            log(kind, &self.root_path, item_count, elapsed_ms);
        }
    }

    fn start(&mut self, root_path: &str) {
        self.root_path = clean_path(root_path);
        self.canonical_root = fs::canonicalize(&self.root_path)
            .map(|p| clean_path(&p.to_string_lossy()))
            .ok()
            .filter(|canonical| !path_equals(canonical, &self.root_path))
            .unwrap_or_default();
        self.project_config.set_root_path(&self.root_path);
        self.previous.clear();
        self.pending.clear();
        self.dirty_relative.clear();
        self.scan_sequence = 0;
        self.continuing_batch = false;

        let timer = Instant::now();
        let mut baseline = HashMap::new();
        let complete = scan_tree_state(
            &self.root_path,
            &self.root_path,
            &self.project_config,
            &mut baseline,
        );
        self.previous = baseline;
        // An unreadable tree would look like an empty project and turn every
        // file into a "created" event later; reconcile again shortly instead.
        if !complete {
            self.debounce_deadline = Some(Instant::now() + DEBOUNCE);
        }
        self.log("baseline", self.previous.len(), timer.elapsed().as_millis());

        self.safety_deadline = Some(Instant::now() + SAFETY_SCAN);
        self.debounce_deadline = None;
    }

    fn stop(&mut self) {
        self.root_path.clear();
        self.canonical_root.clear();
        self.previous.clear();
        self.pending.clear();
        self.dirty_relative.clear();
        self.debounce_deadline = None;
        self.safety_deadline = None;
    }

    fn mark_dirty_relative(&mut self, relative_path: String) {
        if relative_path.is_empty() || relative_path.starts_with("..") {
            return;
        }
        if !self.project_config.should_track(&relative_path) {
            return;
        }

        self.dirty_relative.insert(relative_path);
        // Keep the first deadline for this batch. Continuous filesystem
        // notifications must not postpone it forever; observe_path handles
        // files that are still changing by scheduling a stabilization pass.
        if self.debounce_deadline.is_none() {
            self.debounce_deadline = Some(Instant::now() + DEBOUNCE);
        }
    }

    /// Remaps a symlink-resolved event path back into the configured root's
    /// path domain, so lexical prefix matching downstream keeps working.
    fn to_root_domain(&self, absolute: &str) -> String {
        if self.canonical_root.is_empty()
            || !relative_file_path(&self.root_path, absolute).starts_with("..")
        {
            return absolute.to_string();
        }
        let canonical_relative = relative_file_path(&self.canonical_root, absolute);
        if canonical_relative.starts_with("..") {
            return absolute.to_string();
        }
        if canonical_relative.is_empty() {
            return self.root_path.clone();
        }
        join_path(&self.root_path, &canonical_relative)
    }

    fn has_previous_under(&self, relative_dir: &str) -> bool {
        if relative_dir.is_empty() {
            return false;
        }
        let prefix = format!("{relative_dir}/");
        self.previous.keys().any(|key| key.starts_with(&prefix))
    }

    fn on_fs_paths(&mut self, paths: Vec<std::path::PathBuf>) {
        if self.root_path.is_empty() {
            return;
        }
        for path in paths {
            let absolute = self.to_root_domain(&clean_path(&path.to_string_lossy()));
            let relative = relative_file_path(&self.root_path, &absolute);
            if relative.starts_with("..") {
                continue;
            }

            if Path::new(&absolute).is_dir() {
                self.queue_subtree_reconcile(&absolute);
            } else if !Path::new(&absolute).exists() && self.has_previous_under(&relative) {
                // A directory vanished (moved/deleted): reconcile its subtree
                // now so the deletions are recorded immediately instead of on
                // the next safety scan.
                self.queue_subtree_reconcile(&absolute);
            } else {
                self.mark_dirty_relative(relative);
            }
        }
    }

    fn queue_subtree_reconcile(&mut self, dir_path: &str) {
        let normalized_dir = clean_path(dir_path);
        let dir_prefix = relative_file_path(&self.root_path, &normalized_dir);
        if dir_prefix.starts_with("..") {
            return;
        }

        let prefix = if dir_prefix.is_empty() {
            String::new()
        } else {
            format!("{dir_prefix}/")
        };

        let mut current_subtree = HashMap::new();
        let complete = scan_tree_state(
            &self.root_path,
            &normalized_dir,
            &self.project_config,
            &mut current_subtree,
        );

        let keys: Vec<String> = current_subtree.keys().cloned().collect();
        for key in keys {
            self.mark_dirty_relative(key);
        }

        // A listing that failed part-way says nothing about what is missing.
        if !complete {
            return;
        }

        let missing: Vec<String> = self
            .previous
            .keys()
            .filter(|key| prefix.is_empty() || key.starts_with(&prefix))
            .filter(|key| !current_subtree.contains_key(*key))
            .cloned()
            .collect();
        for key in missing {
            self.mark_dirty_relative(key);
        }
    }

    fn on_safety_scan(&mut self) {
        let timer = Instant::now();
        let before_dirty = self.dirty_relative.len();
        self.queue_full_tree_reconcile();
        self.log("safety", before_dirty, timer.elapsed().as_millis());
        self.safety_deadline = Some(Instant::now() + SAFETY_SCAN);
    }

    fn process_due_timers(&mut self) {
        let now = Instant::now();
        if self
            .debounce_deadline
            .is_some_and(|deadline| deadline <= now)
        {
            self.process_dirty_batch();
        }
        if self.safety_deadline.is_some_and(|deadline| deadline <= now) {
            self.on_safety_scan();
        }
    }

    fn queue_full_tree_reconcile(&mut self) {
        if self.root_path.is_empty() {
            return;
        }

        let mut current = HashMap::new();
        let complete = scan_tree_state(
            &self.root_path,
            &self.root_path,
            &self.project_config,
            &mut current,
        );

        // Both maps hold only tracked paths (filtered during the scan), so
        // no per-entry should_track re-check is needed here.
        let changed: Vec<String> = current
            .iter()
            .filter(|(path, state)| self.previous.get(*path) != Some(*state))
            .map(|(path, _)| path.clone())
            .collect();
        for path in changed {
            self.mark_dirty_relative(path);
        }

        // Only a complete listing can tell us something is gone: a share
        // that stalled would otherwise look like every file being deleted.
        if !complete {
            return;
        }
        let deleted: Vec<String> = self
            .previous
            .keys()
            .filter(|path| !current.contains_key(*path))
            .cloned()
            .collect();
        for path in deleted {
            self.mark_dirty_relative(path);
        }
    }

    fn process_dirty_batch(&mut self) {
        self.debounce_deadline = None;
        if self.root_path.is_empty() || self.dirty_relative.is_empty() {
            return;
        }

        let timer = Instant::now();

        // A file still being written re-arms the batch, and the files that
        // settled earlier belong to the same save as the ones that settle in
        // that follow-up pass. Keeping the sequence for a continuation is
        // what lets the snapshot service record them as one version instead
        // of splitting a project across two.
        if !self.continuing_batch {
            self.scan_sequence += 1;
        }
        let sequence = self.scan_sequence;
        let batch: Vec<String> = self.dirty_relative.drain().collect();
        let batch_len = batch.len();

        let mut needs_another_pass = false;
        for relative_path in batch {
            if self.observe_path(&relative_path, sequence) {
                needs_another_pass = true;
            }
        }

        self.log("debounce", batch_len, timer.elapsed().as_millis());

        self.continuing_batch = needs_another_pass;
        if needs_another_pass {
            self.debounce_deadline = Some(Instant::now() + DEBOUNCE);
        }
    }

    /// Returns true when the path still needs another stabilization pass.
    fn observe_path(&mut self, relative_path: &str, sequence: i64) -> bool {
        let absolute_path = join_path(&self.root_path, relative_path);
        let state = match state_of(Path::new(&absolute_path)) {
            Observed::File(state) => state,
            Observed::Missing => {
                if self.previous.remove(relative_path).is_some() {
                    self.pending.remove(relative_path);
                    self.emit_change(FileEventType::Deleted, relative_path, sequence, 0);
                }
                return false;
            }
            // The volume would not answer. Say nothing and look again on the
            // next pass rather than record a deletion that never happened.
            Observed::Unknown => return true,
        };

        if self.previous.get(relative_path) == Some(&state) {
            self.pending.remove(relative_path);
            return false;
        }

        if self.pending.get(relative_path) == Some(&state) {
            let is_new = !self.previous.contains_key(relative_path);
            let modified_ms = state.msecs_since_epoch;
            self.previous.insert(relative_path.to_string(), state);
            self.pending.remove(relative_path);
            self.emit_change(
                if is_new {
                    FileEventType::Created
                } else {
                    FileEventType::Modified
                },
                relative_path,
                sequence,
                modified_ms,
            );
            return false;
        }

        self.pending.insert(relative_path.to_string(), state);
        self.dirty_relative.insert(relative_path.to_string());
        true
    }

    fn emit_change(
        &self,
        event_type: FileEventType,
        relative_path: &str,
        sequence: i64,
        modified_ms: i64,
    ) {
        let event = FileEvent {
            event_type,
            relative_path: relative_path.to_string(),
            absolute_path: join_path(&self.root_path, relative_path),
            scan_sequence: sequence,
            modified_ms,
        };
        (self.on_event)(event);
    }
}

pub struct HybridFileWatcher {
    tx: Sender<Msg>,
    handle: Option<JoinHandle<()>>,
}

impl HybridFileWatcher {
    pub fn new(
        on_event: impl Fn(FileEvent) + Send + 'static,
        log_scan: Option<ScanLogFn>,
        on_ready: ReadyFn,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<Msg>();
        let fs_tx = tx.clone();

        let handle = std::thread::Builder::new()
            .name("musit-watcher".to_string())
            .spawn(move || {
                let mut state = WorkerState {
                    root_path: String::new(),
                    canonical_root: String::new(),
                    project_config: ProjectConfig::default(),
                    scan_sequence: 0,
                    continuing_batch: false,
                    dirty_relative: HashSet::new(),
                    previous: HashMap::new(),
                    pending: HashMap::new(),
                    debounce_deadline: None,
                    safety_deadline: None,
                    on_event: Box::new(on_event),
                    log_scan,
                };
                let mut os_watcher: Option<RecommendedWatcher> = None;

                loop {
                    // A busy OS event queue must not starve debounce or the
                    // periodic full-tree scan. recv_timeout only reports a
                    // timeout when no message is already queued.
                    state.process_due_timers();
                    let now = Instant::now();
                    let deadline = [state.debounce_deadline, state.safety_deadline]
                        .into_iter()
                        .flatten()
                        .min();
                    let timeout = deadline
                        .map(|d| d.saturating_duration_since(now))
                        .unwrap_or(Duration::from_secs(3600));

                    match rx.recv_timeout(timeout) {
                        Ok(Msg::Start(root)) => {
                            // Replace any prior OS watcher with one for the new root.
                            drop(os_watcher.take());
                            state.start(&root);
                            let event_tx = fs_tx.clone();
                            let mut created = notify::recommended_watcher(
                                move |result: Result<notify::Event, notify::Error>| {
                                    if let Ok(event) = result
                                        && !event.paths.is_empty()
                                        && signals_change(&event.kind)
                                    {
                                        let _ = event_tx.send(Msg::Fs(event.paths));
                                    }
                                },
                            )
                            .ok();
                            if let Some(watcher) = created.as_mut() {
                                let _ = watcher
                                    .watch(Path::new(&state.root_path), RecursiveMode::Recursive);
                            }
                            os_watcher = created;
                            on_ready(&state.root_path);
                        }
                        Ok(Msg::Stop) => {
                            os_watcher = None;
                            state.stop();
                        }
                        Ok(Msg::Fs(paths)) => {
                            if !state.root_path.is_empty() {
                                state.on_fs_paths(paths);
                            }
                        }
                        Ok(Msg::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                            drop(os_watcher);
                            break;
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                }
            })
            .expect("watcher thread spawns");

        Self {
            tx,
            handle: Some(handle),
        }
    }

    pub fn start_watching(&self, root_path: &str) -> bool {
        if root_path.is_empty() {
            return false;
        }
        self.tx.send(Msg::Start(clean_path(root_path))).is_ok()
    }

    pub fn stop_watching(&self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

/// Whether an OS event can mean content changed. inotify also reports
/// opens and read-only closes, and the watcher's own directory listings and
/// file reads produce those, so forwarding them rescans forever. A close
/// after writing is kept: it is how inotify reports a finished save.
fn signals_change(kind: &notify::EventKind) -> bool {
    use notify::event::{AccessKind, AccessMode, EventKind};
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

impl Drop for HybridFileWatcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_not_changes() {
        use notify::event::{AccessKind, AccessMode, CreateKind, EventKind};
        assert!(!signals_change(&EventKind::Access(AccessKind::Open(
            AccessMode::Any
        ))));
        assert!(!signals_change(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(signals_change(&EventKind::Access(AccessKind::Close(
            AccessMode::Write
        ))));
        assert!(signals_change(&EventKind::Create(CreateKind::File)));
    }

    #[test]
    fn baseline_scan_holds_only_tracked_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = clean_path(&dir.path().to_string_lossy());
        std::fs::write(dir.path().join("song.als"), b"project").unwrap();
        std::fs::write(dir.path().join("bounce.wav"), b"audio").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"text").unwrap();

        let config = ProjectConfig::new(root.clone());
        let mut out = HashMap::new();
        scan_tree_state(&root, &root, &config, &mut out);

        assert!(out.contains_key("song.als"));
        // Untracked files can never emit events (mark_dirty_relative filters
        // by should_track), so the baseline must not spend memory on them.
        assert!(!out.contains_key("bounce.wav"));
        assert!(!out.contains_key("notes.txt"));
    }
}
