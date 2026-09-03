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

use crate::backend::{VersionEntry, migrate_misrouted_history, resolve_staged_path};
use crate::file_event::FileEvent;
use crate::object_store::ObjectStore;
use crate::path_cleanup::{join_path, parent_path, path_is_under_root, path_key};
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
    /// Restore `version` of `project`, snapshotting the on-disk state first
    /// when no version matches it.
    Restore {
        project: DiscoveredProject,
        version: VersionEntry,
        needs_pre_snapshot: bool,
    },
    SetRetention {
        retention: i32,
        compact_existing: bool,
    },
    /// Forget every project under `folder_root`.
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
    tx: Sender<Job>,
}

impl Worker {
    pub fn spawn(on_reply: impl Fn(Reply) + Send + 'static, on_notice: NoticeSink) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("musit-versioning".to_string())
            .spawn(move || {
                let mut state = State {
                    services: HashMap::new(),
                    roots: HashMap::new(),
                    on_notice,
                };
                state.run(rx, on_reply);
            })
            .expect("versioning thread spawns");
        Self { tx }
    }

    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }
}

struct State {
    /// Services by `path_key` of the project root.
    services: HashMap<String, SnapshotService>,
    /// Canonical project root by the same key.
    roots: HashMap<String, String>,
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
                    needs_pre_snapshot,
                } => {
                    let result = self.restore(&project, &version, needs_pre_snapshot);
                    on_reply(Reply::RestoreDone {
                        project_root: project.root_path,
                        project_name: project.name,
                        version_id: version.id,
                        result,
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
                        .roots
                        .iter()
                        .filter(|(_, root)| path_is_under_root(root, &folder_root))
                        .map(|(key, _)| key.clone())
                        .collect();
                    for key in doomed {
                        self.services.remove(&key);
                        self.roots.remove(&key);
                    }
                }
                Job::DropAll => {
                    self.services.clear();
                    self.roots.clear();
                }
            }
        }
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
        needs_pre_snapshot: bool,
    ) -> Result<(), String> {
        let artifact_path = join_path(&project.root_path, &project.primary_project_file);
        let key = path_key(&project.root_path);

        // If the on-disk state matches no snapshot (e.g. the watcher has not
        // seen the latest save yet), snapshot it now so the restore cannot
        // silently destroy the user's most recent work.
        if needs_pre_snapshot
            && Path::new(&artifact_path).exists()
            && let Some(service) = self.services.get_mut(&key)
        {
            service.snapshot_path_now(&artifact_path, &project.primary_project_file);
        }

        let object_store = ObjectStore::new(join_path(&project.root_path, ".musit"));

        struct Pending {
            destination_path: String,
            temp_path: String,
            relative_path: String,
        }
        let mut pending: Vec<Pending> = Vec::new();
        let cleanup_temps = |pending: &[Pending]| {
            for item in pending {
                let _ = fs::remove_file(&item.temp_path);
            }
        };

        for file_entry in &version.files {
            let relative_path = file_entry.path.clone();
            let destination_path = join_path(&project.root_path, &relative_path);

            if fs::create_dir_all(parent_path(&destination_path)).is_err() {
                cleanup_temps(&pending);
                return Err(format!(
                    "restore failed: could not create folder for {destination_path}"
                ));
            }

            let temp_path = format!("{destination_path}.musit-restore.tmp");
            let _ = fs::remove_file(&temp_path);

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
                relative_path,
            });
        }

        if pending.is_empty() {
            return Err(format!(
                "restore failed: version v{} of {} has no files",
                version.id, project.name
            ));
        }

        // All temps are ready; swap them in.
        for item in &pending {
            if Path::new(&item.destination_path).exists()
                && fs::remove_file(&item.destination_path).is_err()
            {
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
                    return Err(format!(
                        "restore failed: copy failed from {} to {}",
                        item.temp_path, item.destination_path
                    ));
                }
                let _ = fs::remove_file(&item.temp_path);
            }
        }

        if let Some(service) = self.services.get_mut(&key) {
            for item in &pending {
                service.suppress_next_events_for_path(&item.relative_path, 1);
            }
            service.set_branch_base_for_artifact(&project.primary_project_file, &version.id);
        }
        Ok(())
    }
}
