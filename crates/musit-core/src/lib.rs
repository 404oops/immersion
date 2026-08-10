//! musit-core: domain logic for Immersion's automatic project versioning.
//!
//! Faithful Rust port of the Qt/C++ core (`qt-legacy/src/core` and
//! `qt-legacy/src/persistence`). On-disk formats (`.musit` object store,
//! `versions/log.jsonl`, `branch-state.json`, app config JSON) are byte-level
//! compatible with the Qt build so existing histories keep working.

pub mod backend;
pub mod backup_template;
pub mod file_event;
pub mod folder_settings;
pub mod metadata_store;
pub mod object_store;
pub mod path_cleanup;
pub mod project_config;
pub mod project_discovery;
pub mod project_registry;
pub mod qcompress;
pub mod snapshot_service;
pub mod version_id;
pub mod watcher;

pub use backup_template::{BackupCategory, BackupTemplate, ProjectKind};
pub use file_event::{FileEvent, FileEventType};
pub use folder_settings::ProjectsFolderLayout;
pub use project_discovery::DiscoveredProject;
pub use snapshot_service::{SnapshotNotice, SnapshotService};
