//! Append-only JSONL version log under `.musit/versions/log.jsonl`.
//! Port of `qt-legacy/src/persistence/MetadataStore.{h,cpp}`.

use crate::file_event::{FileEvent, FileEventType};
use crate::path_cleanup::{artifact_equals, join_path};
use crate::version_id;
use chrono::Utc;
use serde_json::{Map, Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

fn top_level_version(version_id: &str) -> i32 {
    let first_segment = version_id.split('.').next().unwrap_or("");
    first_segment.parse::<i32>().unwrap_or(-1)
}

fn direct_child_version(version_id: &str, branch_base_version: &str) -> i32 {
    if branch_base_version.is_empty() {
        return -1;
    }

    let prefix = format!("{branch_base_version}.");
    let Some(remainder) = version_id.strip_prefix(&prefix) else {
        return -1;
    };
    if remainder.contains('.') {
        return -1;
    }
    remainder.parse::<i32>().unwrap_or(-1)
}

/// Artifact recorded on a log line, with fallback for legacy lines that
/// predate the artifact field (they were always single-file versions).
pub fn artifact_of_log_line(obj: &Map<String, Value>) -> String {
    let artifact = obj.get("artifact").and_then(|v| v.as_str()).unwrap_or("");
    if !artifact.is_empty() {
        return artifact.to_string();
    }
    obj.get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[derive(Clone, Debug, Default)]
pub struct MetadataStore {
    musit_root: String,
}

impl MetadataStore {
    pub fn new(musit_root: impl Into<String>) -> Self {
        Self {
            musit_root: musit_root.into(),
        }
    }

    pub fn set_musit_root(&mut self, musit_root: &str) {
        self.musit_root = musit_root.to_string();
    }

    pub fn init(&self) -> bool {
        if self.musit_root.is_empty() {
            return false;
        }
        fs::create_dir_all(join_path(&self.musit_root, "versions")).is_ok()
    }

    pub fn log_path(&self) -> String {
        join_path(&self.musit_root, "versions/log.jsonl")
    }

    /// Iterates parsed JSON objects of the log, in append order.
    pub fn read_log_lines(&self) -> Vec<Map<String, Value>> {
        let log_path = self.log_path();
        if !Path::new(&log_path).exists() {
            return Vec::new();
        }
        // Read as bytes and decode per line: one torn append with invalid
        // UTF-8 must lose only that line, not silently empty the whole log
        // (which would restart version numbering and corrupt the graph).
        let Ok(contents) = fs::read(&log_path) else {
            return Vec::new();
        };
        contents
            .split(|byte| *byte == b'\n')
            .filter_map(|line| std::str::from_utf8(line).ok())
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter_map(|value| match value {
                Value::Object(map) => Some(map),
                _ => None,
            })
            .collect()
    }

    /// Versions are numbered per artifact (a project file, or a bundle root
    /// like "Song.logicx" whose internal files share one version per save).
    pub fn next_version_id_for_artifact(
        &self,
        artifact: &str,
        branch_base_version: &str,
    ) -> String {
        let mut running_ordinal = 0i64;
        let mut max_top_level = 0i32;
        let mut max_child_for_base = 0i32;

        for obj in self.read_log_lines() {
            if !artifact_equals(&artifact_of_log_line(&obj), artifact) {
                continue;
            }

            let staged_path = obj.get("staged").and_then(|v| v.as_str()).unwrap_or("");
            if staged_path.is_empty() {
                continue;
            }

            running_ordinal += 1;
            let mut version_id = obj
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if version_id.is_empty() {
                version_id = running_ordinal.to_string();
            }

            let top_level = top_level_version(&version_id);
            if top_level > max_top_level {
                max_top_level = top_level;
            }

            let child = direct_child_version(&version_id, branch_base_version);
            if child > max_child_for_base {
                max_child_for_base = child;
            }
        }

        if !branch_base_version.is_empty() {
            return format!("{branch_base_version}.{}", max_child_for_base + 1);
        }
        (max_top_level + 1).to_string()
    }

    /// Appends one snapshot line. version_id/parent_version are pre-allocated
    /// by the caller so several files of one save can share a version.
    pub fn append_snapshot_event(
        &self,
        event: &FileEvent,
        artifact: &str,
        object_hash: &str,
        staged_path: &str,
        version_id: &str,
        parent_version: &str,
    ) -> bool {
        if self.musit_root.is_empty() {
            return false;
        }

        let type_value = match event.event_type {
            FileEventType::Created => "created",
            FileEventType::Modified => "modified",
            FileEventType::Deleted => "deleted",
            FileEventType::Renamed => "renamed",
        };

        let mut line = Map::new();
        line.insert(
            "ts".to_string(),
            json!(Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        );
        line.insert("type".to_string(), json!(type_value));
        line.insert("path".to_string(), json!(event.relative_path));
        if !artifact.is_empty() && artifact != event.relative_path {
            line.insert("artifact".to_string(), json!(artifact));
        }
        line.insert("object".to_string(), json!(object_hash));
        line.insert("staged".to_string(), json!(staged_path));

        if !staged_path.is_empty() && !version_id.is_empty() {
            line.insert("version".to_string(), json!(version_id));
            if !parent_version.is_empty() {
                line.insert("parent".to_string(), json!(parent_version));
            }
        }

        let Ok(mut log_file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())
        else {
            return false;
        };

        let encoded = format!(
            "{}\n",
            serde_json::to_string(&Value::Object(line)).expect("json line serializes")
        );
        log_file.write_all(encoded.as_bytes()).is_ok()
    }

    /// Staged paths (as stored in the log) of all but the `keep_count` most
    /// recently appended versions of a path; used to compact old versions
    /// down to their compressed objects.
    pub fn staged_paths_beyond_newest(&self, relative_path: &str, keep_count: i32) -> Vec<String> {
        if self.musit_root.is_empty() || relative_path.is_empty() || keep_count < 0 {
            return Vec::new();
        }

        // Append order == chronological order, so "newest" is the tail.
        let staged_paths_in_order: Vec<String> = self
            .read_log_lines()
            .into_iter()
            .filter(|obj| obj.get("path").and_then(|v| v.as_str()) == Some(relative_path))
            .filter_map(|obj| {
                let staged = obj.get("staged").and_then(|v| v.as_str()).unwrap_or("");
                if staged.is_empty() {
                    None
                } else {
                    Some(staged.to_string())
                }
            })
            .collect();

        let excess = staged_paths_in_order.len() as i64 - keep_count as i64;
        if excess <= 0 {
            return Vec::new();
        }
        staged_paths_in_order[..excess as usize].to_vec()
    }

    pub fn snapshot_paths(&self) -> Vec<String> {
        if self.musit_root.is_empty() {
            return Vec::new();
        }

        let mut paths: Vec<String> = Vec::new();
        for obj in self.read_log_lines() {
            let path = obj.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if !path.is_empty() && !paths.iter().any(|existing| existing == path) {
                paths.push(path.to_string());
            }
        }
        paths
    }

    pub fn latest_staged_version_for_artifact(&self, artifact: &str) -> String {
        if self.musit_root.is_empty() || artifact.is_empty() {
            return String::new();
        }

        let mut running_ordinal = 0i64;
        let mut latest_version_id = String::new();
        for obj in self.read_log_lines() {
            if !artifact_equals(&artifact_of_log_line(&obj), artifact) {
                continue;
            }

            let staged_path = obj.get("staged").and_then(|v| v.as_str()).unwrap_or("");
            if staged_path.is_empty() {
                continue;
            }

            running_ordinal += 1;
            let candidate_version = obj
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| running_ordinal.to_string());
            if latest_version_id.is_empty()
                || version_id::less_than(&latest_version_id, &candidate_version)
            {
                latest_version_id = candidate_version;
            }
        }

        latest_version_id
    }
}
