//! Versioning worker: owns every project's [`SnapshotService`] and does all
//! snapshot, seeding and restore work on its own thread. The UI thread only
//! queues jobs and reads the replies, so a project file crossing a slow
//! share or a spinning disk never stalls the window.
//!
//! Memory stays where it was: the services live here instead of in the
//! backend rather than being duplicated, jobs are small descriptors, and
//! file data still streams through the object store's fixed buffer.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::backend::{
    VersionEntry, mark_compressed_versions, mark_current_version_with, migrate_misrouted_history,
    parse_versions_from_log, resolve_staged_path, sha256_file_hex,
};
use crate::file_event::FileEvent;
use crate::object_store::ObjectStore;
use crate::path_cleanup::{join_path, parent_path, path_equals, path_key};
use crate::project_discovery::DiscoveredProject;
use crate::snapshot_service::{SnapshotNotice, SnapshotService};

/// Work for the versioning thread.
pub enum Job {
    /// Set up versioning for one project: migrate misrouted history from
    /// `ancestor_roots`, open its `.musit` store and seed a first version if
    /// it has none.
    Init {
        folder_root: String,
        /// The folder's monitoring generation when queued; a reply from an
        /// older generation is stale (the folder was rescanned or removed).
        generation: u64,
        project: DiscoveredProject,
        ancestor_roots: Vec<String>,
        retention: i32,
    },
    /// A watcher event already routed to the project it belongs to.
    FileEvent {
        root_key: String,
        event: FileEvent,
    },
    /// Restore `version` of `project`. `known_object_hashes` holds every
    /// object hash the artifact's history contains, so the worker can decide
    /// for itself, at the moment it overwrites, whether what is on disk has
    /// ever been captured.
    Restore {
        project: DiscoveredProject,
        version: VersionEntry,
        known_object_hashes: Vec<String>,
    },
    /// Build a project's version list: parsing its log, hashing the file on
    /// disk to find which version is current, and checking which versions
    /// still have staged copies. That is all filesystem work, and on a share
    /// or an external disk it is far too slow to do while the window draws.
    LoadVersions {
        project: DiscoveredProject,
    },
    SetRetention {
        retention: i32,
        compact_existing: bool,
    },
    /// Forget every project this folder brought in. Ownership is recorded
    /// when the project is initialized rather than derived from the paths,
    /// so a folder tab nested inside another does not silently drop the
    /// other's services.
    DropUnder {
        folder_root: String,
    },
    DropAll,
}

/// What the thread reports back.
pub enum Reply {
    InitDone {
        folder_root: String,
        generation: u64,
        project_root: String,
        ok: bool,
        seeded: bool,
        migrated: usize,
    },
    /// A project's version list, ready for the graph.
    Versions {
        project_root: String,
        primary_file: String,
        versions: Vec<VersionEntry>,
    },
    RestoreDone {
        project_root: String,
        project_name: String,
        version_id: String,
        result: Result<(), String>,
    },
}

/// Receives each service's notices along with the project root they belong
/// to. Shared by every service, hence `Sync`.
pub type NoticeSink = Arc<dyn Fn(String, SnapshotNotice) + Send + Sync>;

/// Handle to the worker thread. Dropping it (with the backend) closes the
/// queue; the thread finishes what it has and exits.
pub struct Worker {
    tx: Option<Sender<Job>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(on_reply: impl Fn(Reply) + Send + 'static, on_notice: NoticeSink) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("musit-versioning".to_string())
            .spawn(move || {
                let mut state = State {
                    services: HashMap::new(),
                    roots: HashMap::new(),
                    folders: HashMap::new(),
                    hashes: HashMap::new(),
                    on_notice,
                };
                state.run(rx, on_reply);
            })
            .expect("versioning thread spawns");
        Self {
            tx: Some(tx),
            handle: Some(handle),
        }
    }

    /// Queues a job. False means the worker thread is gone, which the
    /// caller must surface rather than wait forever for a reply that cannot
    /// arrive.
    pub fn send(&self, job: Job) -> bool {
        self.tx.as_ref().is_some_and(|tx| tx.send(job).is_ok())
    }
}

impl Drop for Worker {
    /// Closes the queue and waits for the job in hand. Quitting in the
    /// middle of a restore would otherwise leave a project file replaced by
    /// nothing, with its content only in a scratch file.
    fn drop(&mut self) {
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct State {
    /// Services by `path_key` of the project root.
    services: HashMap<String, SnapshotService>,
    /// Canonical project root by the same key.
    roots: HashMap<String, String>,
    /// Folder tab that owns each service, by the same key.
    folders: HashMap<String, String>,
    /// Content hashes of files on disk against their (modified time, length),
    /// so rebuilding a version list that has not changed costs a stat rather
    /// than reading the whole project file again.
    hashes: HashMap<String, (Option<std::time::SystemTime>, u64, String)>,
    on_notice: NoticeSink,
}

impl State {
    fn run(&mut self, rx: Receiver<Job>, on_reply: impl Fn(Reply)) {
        for job in rx {
            match job {
                Job::Init {
                    folder_root,
                    generation,
                    project,
                    ancestor_roots,
                    retention,
                } => {
                    let reply = self.init(
                        folder_root,
                        generation,
                        &project,
                        &ancestor_roots,
                        retention,
                    );
                    on_reply(reply);
                }
                Job::FileEvent { root_key, event } => {
                    if let Some(service) = self.services.get_mut(&root_key) {
                        service.on_file_event(&event);
                    }
                }
                Job::Restore {
                    project,
                    version,
                    known_object_hashes,
                } => {
                    let result = self.restore(&project, &version, &known_object_hashes);
                    on_reply(Reply::RestoreDone {
                        project_root: project.root_path,
                        project_name: project.name,
                        version_id: version.id,
                        result,
                    });
                }
                Job::LoadVersions { project } => {
                    let versions = self.load_versions(&project);
                    on_reply(Reply::Versions {
                        project_root: project.root_path,
                        primary_file: project.primary_project_file,
                        versions,
                    });
                }
                Job::SetRetention {
                    retention,
                    compact_existing,
                } => {
                    for service in self.services.values_mut() {
                        service.set_uncompressed_recent_versions(retention);
                        if compact_existing {
                            service.compact_all_staged_copies();
                        }
                    }
                }
                Job::DropUnder { folder_root } => {
                    let doomed: Vec<String> = self
                        .folders
                        .iter()
                        .filter(|(_, owner)| path_equals(owner, &folder_root))
                        .map(|(key, _)| key.clone())
                        .collect();
                    for key in doomed {
                        self.services.remove(&key);
                        self.roots.remove(&key);
                        self.folders.remove(&key);
                    }
                }
                Job::DropAll => {
                    self.services.clear();
                    self.roots.clear();
                    self.folders.clear();
                }
            }
        }
    }

    /// A project's version list, with the current version marked and the
    /// compacted ones flagged.
    fn load_versions(&mut self, project: &DiscoveredProject) -> Vec<VersionEntry> {
        let log_path = join_path(&project.root_path, ".musit/versions/log.jsonl");
        let mut versions = parse_versions_from_log(&log_path, &project.primary_project_file);
        // Bounded like the backend's own caches: a folder with thousands of
        // projects must not accumulate hashes forever.
        if self.hashes.len() > 4096 {
            self.hashes.clear();
        }
        let hashes = &mut self.hashes;
        mark_current_version_with(project, &mut versions, |path| {
            let stamp = fs::metadata(path)
                .map(|meta| (meta.modified().ok(), meta.len()))
                .unwrap_or((None, 0));
            if let Some(entry) = hashes.get(path)
                && (entry.0, entry.1) == stamp
            {
                return entry.2.clone();
            }
            let hash = sha256_file_hex(path);
            hashes.insert(path.to_string(), (stamp.0, stamp.1, hash.clone()));
            hash
        });
        mark_compressed_versions(project, &mut versions);
        versions
    }

    fn init(
        &mut self,
        folder_root: String,
        generation: u64,
        project: &DiscoveredProject,
        ancestor_roots: &[String],
        retention: i32,
    ) -> Reply {
        let mut migrated = 0usize;
        for ancestor_root in ancestor_roots {
            migrated += migrate_misrouted_history(ancestor_root, &project.root_path);
        }

        let mut service = SnapshotService::new();
        if !service.set_project_root(&project.root_path) {
            return Reply::InitDone {
                folder_root,
                generation,
                project_root: project.root_path.clone(),
                ok: false,
                seeded: false,
                migrated,
            };
        }
        service.set_uncompressed_recent_versions(retention);
        let notice = self.on_notice.clone();
        let notice_root = project.root_path.clone();
        service.set_notice_sink(move |n| notice(notice_root.clone(), n));

        let mut seeded = false;
        if !project.primary_project_file.is_empty()
            && !service.has_version_for_artifact(&project.primary_project_file)
        {
            let absolute = join_path(&project.root_path, &project.primary_project_file);
            seeded = service.snapshot_path_now(&absolute, &project.primary_project_file);
        }

        let key = path_key(&project.root_path);
        self.roots.insert(key.clone(), project.root_path.clone());
        self.folders.insert(key.clone(), folder_root.clone());
        self.services.insert(key, service);
        Reply::InitDone {
            folder_root,
            generation,
            project_root: project.root_path.clone(),
            ok: true,
            seeded,
            migrated,
        }
    }

    /// Materializes every file of the version next to its destination first,
    /// so nothing is touched unless the whole version is available; recent
    /// versions come from staged copies, compacted ones are decompressed from
    /// the object store. Then swaps them in.
    fn restore(
        &mut self,
        project: &DiscoveredProject,
        version: &VersionEntry,
        known_object_hashes: &[String],
    ) -> Result<(), String> {
        let artifact_path = join_path(&project.root_path, &project.primary_project_file);
        let key = path_key(&project.root_path);

        // If what is on disk right now matches no snapshot (the watcher has
        // not seen the latest save yet, or the user saved while this job
        // waited in the queue), capture it before overwriting. The check is
        // made here, immediately before the swap, because anything decided
        // earlier could be out of date by now. Anything the check cannot
        // establish — an unreadable file, a bundle directory — counts as
        // "not captured", so the safety snapshot is taken.
        if Path::new(&artifact_path).exists() {
            let on_disk = sha256_file_hex(&artifact_path);
            let captured = !on_disk.is_empty()
                && known_object_hashes
                    .iter()
                    .any(|hash| hash.eq_ignore_ascii_case(&on_disk));
            if !captured && let Some(service) = self.services.get_mut(&key) {
                service.snapshot_path_now(&artifact_path, &project.primary_project_file);
            }
        }

        let object_store = ObjectStore::new(join_path(&project.root_path, ".musit"));

        struct Pending {
            destination_path: String,
            temp_path: String,
            /// Where the file that was there has been parked, so a failure
            /// part-way can put it back.
            backup_path: String,
            relative_path: String,
        }
        let mut pending: Vec<Pending> = Vec::new();
        let cleanup_temps = |pending: &[Pending]| {
            for item in pending {
                let _ = fs::remove_file(&item.temp_path);
                let _ = fs::remove_file(&item.backup_path);
            }
        };

        // Scratch files live inside `.musit`, which is never tracked, so a
        // slow restore cannot end up versioning its own temporary files.
        let scratch_dir = join_path(&join_path(&project.root_path, ".musit"), "restore");
        if fs::create_dir_all(&scratch_dir).is_err() {
            return Err(format!("restore failed: could not create {scratch_dir}"));
        }
        let scratch_name =
            |suffix: &str, index: usize| join_path(&scratch_dir, &format!("{index}-{suffix}"));

        for (index, file_entry) in version.files.iter().enumerate() {
            let relative_path = file_entry.path.clone();
            let destination_path = join_path(&project.root_path, &relative_path);

            if fs::create_dir_all(parent_path(&destination_path)).is_err() {
                cleanup_temps(&pending);
                return Err(format!(
                    "restore failed: could not create folder for {destination_path}"
                ));
            }

            let temp_path = scratch_name("new", index);
            let backup_path = scratch_name("old", index);
            let _ = fs::remove_file(&temp_path);
            let _ = fs::remove_file(&backup_path);

            let staged_path = resolve_staged_path(&project.root_path, &file_entry.staged_path);
            let mut materialized = false;
            if !staged_path.is_empty() && Path::new(&staged_path).exists() {
                materialized = fs::copy(&staged_path, &temp_path).is_ok();
            }
            if !materialized && !file_entry.object_hash.is_empty() {
                materialized = object_store.extract_object(&file_entry.object_hash, &temp_path);
            }
            if !materialized {
                let _ = fs::remove_file(&temp_path);
                cleanup_temps(&pending);
                return Err(format!(
                    "restore failed: staged snapshot missing for {} (v{})",
                    project.name, version.id
                ));
            }

            pending.push(Pending {
                destination_path,
                temp_path,
                backup_path,
                relative_path,
            });
        }

        if pending.is_empty() {
            return Err(format!(
                "restore failed: version v{} of {} has no files",
                version.id, project.name
            ));
        }

        // Announce the writes before making them: a file swapped early can
        // stabilize and be picked up by the watcher while later files are
        // still being written, which would record the restore as a new
        // version of a half-restored project.
        if let Some(service) = self.services.get_mut(&key) {
            for item in &pending {
                service.suppress_next_events_for_path(&item.relative_path, 1);
            }
        }

        // All temps are ready; swap them in. Each file that is replaced is
        // parked first, so a failure part-way through a multi-file version
        // can put the project back as it was instead of leaving it half
        // restored, or a file missing entirely.
        let mut swapped: Vec<&Pending> = Vec::new();
        let roll_back = |swapped: &[&Pending]| {
            for item in swapped {
                let _ = fs::remove_file(&item.destination_path);
                if Path::new(&item.backup_path).exists() {
                    let _ = fs::rename(&item.backup_path, &item.destination_path);
                }
            }
        };

        for item in &pending {
            let had_destination = Path::new(&item.destination_path).exists();
            if had_destination && fs::rename(&item.destination_path, &item.backup_path).is_err() {
                roll_back(&swapped);
                cleanup_temps(&pending);
                return Err(format!(
                    "restore failed: could not replace {}",
                    item.destination_path
                ));
            }
            if fs::rename(&item.temp_path, &item.destination_path).is_err() {
                // The complete data is still in the temp file; try a plain
                // copy as a last resort before giving up.
                if fs::copy(&item.temp_path, &item.destination_path).is_err() {
                    if had_destination {
                        let _ = fs::rename(&item.backup_path, &item.destination_path);
                    }
                    roll_back(&swapped);
                    cleanup_temps(&pending);
                    return Err(format!(
                        "restore failed: copy failed from {} to {}",
                        item.temp_path, item.destination_path
                    ));
                }
                let _ = fs::remove_file(&item.temp_path);
            }
            swapped.push(item);
        }
        // Everything is in place; the parked originals are no longer needed.
        for item in &pending {
            let _ = fs::remove_file(&item.backup_path);
        }

        if let Some(service) = self.services.get_mut(&key) {
            service.set_branch_base_for_artifact(&project.primary_project_file, &version.id);
        }
        Ok(())
    }
}
