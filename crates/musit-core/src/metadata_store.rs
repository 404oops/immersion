//! Append-only JSONL version log under `.musit/versions/log.jsonl`.

use crate::file_event::{FileEvent, FileEventType};
use crate::path_cleanup::{artifact_equals, join_path};
use crate::version_id;
use chrono::Utc;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;

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

    /// Streams parsed JSON objects of the log, in append order, without
    /// materializing the whole file: the log is re-scanned on every version
    /// computation, so this pass must stay constant-memory.
    ///
    /// Reads bytes and decodes per line: one torn append with invalid UTF-8
    /// must lose only that line, not silently empty the whole log (which
    /// would restart version numbering and corrupt the graph).
    fn for_each_log_line(&self, mut visit: impl FnMut(Map<String, Value>)) {
        let Ok(file) = fs::File::open(self.log_path()) else {
            return;
        };
        let mut reader = std::io::BufReader::new(file);
        let mut raw: Vec<u8> = Vec::new();
        loop {
            raw.clear();
            match std::io::BufRead::read_until(&mut reader, b'\n', &mut raw) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            let Ok(text) = std::str::from_utf8(&raw) else {
                continue;
            };
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) {
                visit(map);
            }
        }
    }

    /// Parsed JSON objects of the log, in append order. Prefer
    /// `for_each_log_line` internally; this materializes every line.
    pub fn read_log_lines(&self) -> Vec<Map<String, Value>> {
        let mut lines = Vec::new();
        self.for_each_log_line(|obj| lines.push(obj));
        lines
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

        self.for_each_log_line(|obj| {
            if !artifact_equals(&artifact_of_log_line(&obj), artifact) {
                return;
            }

            let staged_path = obj.get("staged").and_then(|v| v.as_str()).unwrap_or("");
            if staged_path.is_empty() {
                return;
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
        });

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
        let mut staged_paths_in_order: Vec<String> = Vec::new();
        self.for_each_log_line(|obj| {
            if obj.get("path").and_then(|v| v.as_str()) != Some(relative_path) {
                return;
            }
            let staged = obj.get("staged").and_then(|v| v.as_str()).unwrap_or("");
            if !staged.is_empty() {
                staged_paths_in_order.push(staged.to_string());
            }
        });

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
        let mut seen: HashSet<String> = HashSet::new();
        self.for_each_log_line(|obj| {
            let path = obj.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if !path.is_empty() && seen.insert(path.to_string()) {
                paths.push(path.to_string());
            }
        });
        paths
    }

    pub fn latest_staged_version_for_artifact(&self, artifact: &str) -> String {
        if self.musit_root.is_empty() || artifact.is_empty() {
            return String::new();
        }

        let mut running_ordinal = 0i64;
        let mut latest_version_id = String::new();
        self.for_each_log_line(|obj| {
            if !artifact_equals(&artifact_of_log_line(&obj), artifact) {
                return;
            }

            let staged_path = obj.get("staged").and_then(|v| v.as_str()).unwrap_or("");
            if staged_path.is_empty() {
                return;
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
        });

        latest_version_id
    }
}
