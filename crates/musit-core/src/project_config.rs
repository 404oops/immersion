//! Port of `qt-legacy/src/core/ProjectConfig.{h,cpp}`.

use crate::backup_template;
use crate::path_cleanup::{clean_path, join_path};
use once_cell::sync::Lazy;
use std::collections::HashSet;

static IGNORED_DIRECTORIES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        ".musit",
        ".git",
        ".idea",
        ".vscode",
        ".svn",
        ".hg",
        ".counts",
        "auto-backups",
        "bounce",
        "samples",
        "recordings",
        "master-recordings",
        "multi-samples",
        "stems",
        "exports",
        "renders",
    ]
    .into_iter()
    .collect()
});

/// Directory names (single path components) that are never scanned or tracked.
pub fn is_ignored_directory_name(name: &str) -> bool {
    let lower_name = name.to_lowercase();
    if IGNORED_DIRECTORIES.contains(lower_name.as_str()) {
        return true;
    }

    // DAWs create auto-backup folders with varying names ("Backup", "Backups",
    // "Auto Backup", ...). Match per directory component, never on full paths,
    // so a projects root like "D:/Backups/Music" chosen by the user still works.
    lower_name.contains("backup")
}

fn contains_ignored_directory(normalized_lower_path: &str) -> bool {
    let parts: Vec<&str> = normalized_lower_path
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    // Only look at directory components; the last part is the file name.
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        if is_ignored_directory_name(part) {
            return true;
        }
    }
    false
}

#[derive(Clone, Debug, Default)]
pub struct ProjectConfig {
    root_path: String,
}

impl ProjectConfig {
    pub fn new(root_path: impl Into<String>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }

    pub fn set_root_path(&mut self, root_path: &str) {
        self.root_path = clean_path(root_path);
    }

    pub fn root_path(&self) -> &str {
        &self.root_path
    }

    pub fn musit_path(&self) -> String {
        if self.root_path.is_empty() {
            return String::new();
        }
        join_path(&self.root_path, ".musit")
    }

    pub fn is_ready(&self) -> bool {
        !self.root_path.is_empty()
    }

    pub fn should_track(&self, relative_path: &str) -> bool {
        if relative_path.is_empty() {
            return false;
        }

        let normalized = relative_path.replace('\\', "/").to_lowercase();
        if normalized.starts_with(".musit/") || normalized == ".musit" {
            return false;
        }

        if contains_ignored_directory(&normalized) {
            return false;
        }

        // The backup templates decide what gets versioned: main project files,
        // accompanying files/bundle internals per include patterns, and never
        // audio/temp files.
        backup_template::should_track_path(&normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_musit_and_backup_dirs() {
        let config = ProjectConfig::new("/root");
        assert!(!config.should_track(".musit/versions/log.jsonl"));
        assert!(!config.should_track("Backups/track.als"));
        assert!(!config.should_track("Auto Backup/track.als"));
        assert!(config.should_track("track.als"));
        // Only directory components are checked; a file named "backup.als" passes.
        assert!(config.should_track("backup.als"));
    }
}
