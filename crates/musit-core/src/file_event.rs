//! File change events produced by the watcher and consumed by the snapshot
//! service.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileEventType {
    Created,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Clone, Debug)]
pub struct FileEvent {
    pub event_type: FileEventType,
    /// Absolute path, normalized with '/' separators.
    pub absolute_path: String,
    /// Path relative to the watched/project root, '/' separators.
    pub relative_path: String,
    /// Watcher scan tick that produced this event. Files that stabilize in
    /// the same tick (e.g. several files inside a .logicx bundle written by
    /// one save) are grouped into a single version. 0 = no grouping info.
    pub scan_sequence: i64,
    /// Modified time of the file when the watcher observed it, in
    /// milliseconds since the Unix epoch; 0 when unknown (or deleted).
    pub modified_ms: i64,
}

impl FileEvent {
    pub fn modified(absolute_path: impl Into<String>, relative_path: impl Into<String>) -> Self {
        Self {
            event_type: FileEventType::Modified,
            absolute_path: absolute_path.into(),
            relative_path: relative_path.into(),
            scan_sequence: 0,
            modified_ms: 0,
        }
    }
}
