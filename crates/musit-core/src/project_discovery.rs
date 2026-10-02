//! Scans a projects folder for supported project files/bundles.

use crate::backup_template::{self, ProjectKind};
use crate::folder_settings::ProjectsFolderLayout;
use crate::path_cleanup::{
    clean_path, complete_base_name, file_name, join_path, path_equals, path_is_under_root,
    relative_file_path,
};
use crate::project_config::{self, ProjectConfig};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

#[derive(Clone, Debug, Default)]
pub struct DiscoveredProject {
    pub name: String,
    pub root_path: String,
    pub kind: ProjectKind,
    pub primary_project_file: String,
    pub project_files: Vec<String>,
    pub type_counts: HashMap<ProjectKind, i32>,
    pub total_project_files: i32,
    /// Modified time of the primary file as the scan saw it (kept fresh by
    /// the watcher afterwards), so listing and sorting never stat files.
    pub primary_file_modified: Option<SystemTime>,
}

/// Filesystem metadata for an entry, as much as classifyPath needs.
#[derive(Clone, Debug)]
struct EntryInfo {
    absolute_path: String,
    file_name: String,
    is_dir: bool,
    is_file: bool,
    modified: Option<SystemTime>,
}

fn read_entries(dir: &str) -> Vec<EntryInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "." || name == ".." {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            let absolute_path = clean_path(&entry.path().to_string_lossy());
            Some(EntryInfo {
                absolute_path,
                file_name: name,
                is_dir: metadata.is_dir(),
                is_file: metadata.is_file(),
                modified: metadata.modified().ok(),
            })
        })
        .collect()
}

fn classify_path(info: &EntryInfo) -> ProjectKind {
    let kind = backup_template::kind_for_file_name(&info.file_name);
    if kind == ProjectKind::Unknown {
        return kind;
    }

    if backup_template::kind_is_bundle(kind) != info.is_dir {
        return ProjectKind::Unknown;
    }

    kind
}

#[derive(Default)]
struct ScanState {
    counts_by_folder: HashMap<String, HashMap<ProjectKind, i32>>,
    files_by_folder: HashMap<String, Vec<String>>,
    files_by_folder_and_kind: HashMap<String, HashMap<ProjectKind, Vec<String>>>,
    file_infos_by_folder: HashMap<String, HashMap<String, EntryInfo>>,
}

fn register_marker_project(root_folder: &str, marker_file: &EntryInfo, state: &mut ScanState) {
    let relative_path = relative_file_path(root_folder, &marker_file.absolute_path);
    let kind = backup_template::kind_for_root_marker_path(&relative_path);
    if kind == ProjectKind::Unknown {
        return;
    }

    let mut marker_pattern = "";
    'outer: for tpl in backup_template::all() {
        if tpl.kind != kind {
            continue;
        }
        for marker in tpl.project_root_markers {
            if relative_path.eq_ignore_ascii_case(marker) {
                marker_pattern = marker;
                break 'outer;
            }
        }
    }
    if marker_pattern.is_empty() {
        return;
    }

    let project_root =
        backup_template::project_root_for_marker(marker_pattern, &marker_file.absolute_path);
    let name = marker_file.file_name.clone();

    *state
        .counts_by_folder
        .entry(project_root.clone())
        .or_default()
        .entry(kind)
        .or_insert(0) += 1;
    state
        .files_by_folder
        .entry(project_root.clone())
        .or_default()
        .push(name.clone());
    state
        .files_by_folder_and_kind
        .entry(project_root.clone())
        .or_default()
        .entry(kind)
        .or_default()
        .push(name.clone());
    state
        .file_infos_by_folder
        .entry(project_root)
        .or_default()
        .insert(name, marker_file.clone());
}

static VERSION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:^|[ _-])v(\d+)(?:$|[ _.-])").expect("version regex is valid"));

fn extract_version_number(file_name_str: &str) -> i32 {
    let stem = complete_base_name(file_name_str);
    match VERSION_REGEX.captures(stem) {
        Some(caps) => caps
            .get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or(-1),
        None => -1,
    }
}

fn choose_primary_project_file(
    candidate_files: &[String],
    file_infos_by_name: &HashMap<String, EntryInfo>,
) -> String {
    if candidate_files.is_empty() {
        return String::new();
    }

    let mut best_file = String::new();
    let mut best_version = -1i32;
    let mut best_modified: Option<SystemTime> = None;

    for file_name_str in candidate_files {
        let version = extract_version_number(file_name_str);
        let modified = file_infos_by_name
            .get(file_name_str)
            .and_then(|info| info.modified);

        let is_better_version = version > best_version;
        let is_same_version_newer = version == best_version && modified > best_modified;
        let is_same_version_same_time_lexicographically_earlier = version == best_version
            && modified == best_modified
            && (best_file.is_empty() || file_name_str < &best_file);

        if best_file.is_empty()
            || is_better_version
            || is_same_version_newer
            || is_same_version_same_time_lexicographically_earlier
        {
            best_file = file_name_str.clone();
            best_version = version;
            best_modified = modified;
        }
    }

    best_file
}

fn dominant_kind(counts: &HashMap<ProjectKind, i32>) -> ProjectKind {
    let mut dominant = ProjectKind::Unknown;
    let mut dominant_count = -1i32;

    for candidate in known_kinds() {
        let count = counts.get(&candidate).copied().unwrap_or(0);
        if count > dominant_count {
            dominant = candidate;
            dominant_count = count;
        }
    }

    dominant
}

fn build_projects_from_state(state: &ScanState) -> Vec<DiscoveredProject> {
    let mut projects: Vec<DiscoveredProject> = Vec::new();
    for (folder_path, counts) in &state.counts_by_folder {
        let dominant = dominant_kind(counts);
        if dominant == ProjectKind::Unknown {
            continue;
        }

        let mut project = DiscoveredProject {
            root_path: folder_path.clone(),
            name: {
                let name = file_name(folder_path).to_string();
                if name.is_empty() {
                    folder_path.clone()
                } else {
                    name
                }
            },
            kind: dominant,
            type_counts: counts.clone(),
            project_files: state
                .files_by_folder
                .get(folder_path)
                .cloned()
                .unwrap_or_default(),
            ..Default::default()
        };
        project.project_files.dedup();
        {
            // removeDuplicates preserves first occurrences even non-adjacent.
            let mut seen: Vec<String> = Vec::new();
            for f in &project.project_files {
                if !seen.contains(f) {
                    seen.push(f.clone());
                }
            }
            project.project_files = seen;
        }
        project.project_files.sort();

        let empty_infos = HashMap::new();
        let infos = state
            .file_infos_by_folder
            .get(folder_path)
            .unwrap_or(&empty_infos);
        let dominant_files = state
            .files_by_folder_and_kind
            .get(folder_path)
            .and_then(|by_kind| by_kind.get(&dominant))
            .cloned()
            .unwrap_or_default();
        let selected_primary = choose_primary_project_file(
            if dominant_files.is_empty() {
                &project.project_files
            } else {
                &dominant_files
            },
            infos,
        );
        project.primary_project_file = if selected_primary.is_empty() {
            project.project_files.first().cloned().unwrap_or_default()
        } else {
            selected_primary
        };
        project.primary_file_modified = infos
            .get(&project.primary_project_file)
            .and_then(|info| info.modified);
        project.total_project_files = project.project_files.len() as i32;

        projects.push(project);
    }

    projects.sort_by(|left, right| {
        if left.name == right.name {
            left.root_path.cmp(&right.root_path)
        } else {
            left.name.cmp(&right.name)
        }
    });

    projects
}

fn expand_loose_file_projects(projects: &[DiscoveredProject]) -> Vec<DiscoveredProject> {
    let mut expanded: Vec<DiscoveredProject> = Vec::new();

    for project in projects {
        let loose_files: Vec<String> = project
            .project_files
            .iter()
            .filter(|file_name_str| {
                let kind = backup_template::kind_for_file_name(file_name_str);
                kind != ProjectKind::Unknown && !backup_template::kind_is_bundle(kind)
            })
            .cloned()
            .collect();

        if loose_files.is_empty() {
            continue;
        }

        if loose_files.len() == 1 {
            let mut single = project.clone();
            single.project_files = loose_files.clone();
            single.primary_project_file = loose_files[0].clone();
            single.total_project_files = 1;
            single.name = complete_base_name(&loose_files[0]).to_string();
            if single.name.is_empty() {
                single.name = loose_files[0].clone();
            }
            expanded.push(single);
            continue;
        }

        for file_name_str in &loose_files {
            let mut single = project.clone();
            let file_kind = backup_template::kind_for_file_name(file_name_str);
            single.project_files = vec![file_name_str.clone()];
            single.primary_project_file = file_name_str.clone();
            single.total_project_files = 1;
            single.kind = file_kind;
            single.type_counts.clear();
            single.type_counts.insert(file_kind, 1);
            single.name = complete_base_name(file_name_str).to_string();
            if single.name.is_empty() {
                single.name = file_name_str.clone();
            }
            expanded.push(single);
        }
    }

    expanded.sort_by(|left, right| {
        if left.name == right.name {
            left.root_path.cmp(&right.root_path)
        } else {
            left.name.cmp(&right.name)
        }
    });

    expanded
}

fn resolve_project_root_absolute(selected_folder: &str, relative_path: &str) -> String {
    let normalized_rel = relative_path.replace('\\', "/");

    for tpl in backup_template::all() {
        for marker in tpl.project_root_markers {
            if normalized_rel.eq_ignore_ascii_case(marker) {
                let absolute_path = join_path(selected_folder, relative_path);
                return clean_path(&backup_template::project_root_for_marker(
                    marker,
                    &absolute_path,
                ));
            }
        }
    }

    let artifact_rel = backup_template::artifact_for_path(relative_path);
    if artifact_rel.is_empty() {
        return String::new();
    }

    let artifact_abs = join_path(selected_folder, &artifact_rel);
    clean_path(&crate::path_cleanup::parent_path(&artifact_abs))
}

fn discover_project_at(
    selected_folder: &str,
    absolute_project_root: &str,
) -> Option<DiscoveredProject> {
    let clean_root = clean_path(absolute_project_root);
    if !path_is_under_root(&clean_root, selected_folder) {
        return None;
    }

    if !Path::new(&clean_root).is_dir() {
        return None;
    }

    let mut state = ScanState::default();

    for entry in read_entries(&clean_root) {
        if entry.is_file {
            register_marker_project(selected_folder, &entry, &mut state);
        }

        let kind = classify_path(&entry);
        if kind == ProjectKind::Unknown {
            continue;
        }

        let folder_path = clean_path(&crate::path_cleanup::parent_path(&entry.absolute_path));
        if !path_equals(&folder_path, &clean_root) {
            continue;
        }

        *state
            .counts_by_folder
            .entry(clean_root.clone())
            .or_default()
            .entry(kind)
            .or_insert(0) += 1;
        state
            .files_by_folder
            .entry(clean_root.clone())
            .or_default()
            .push(entry.file_name.clone());
        state
            .files_by_folder_and_kind
            .entry(clean_root.clone())
            .or_default()
            .entry(kind)
            .or_default()
            .push(entry.file_name.clone());
        state
            .file_infos_by_folder
            .entry(clean_root.clone())
            .or_default()
            .insert(entry.file_name.clone(), entry.clone());
    }

    let projects = build_projects_from_state(&state);
    projects
        .into_iter()
        .find(|project| path_equals(&project.root_path, &clean_root))
}

pub type ScanCallback<'a> = &'a mut dyn FnMut(&str);
pub type ProgressCallback<'a> = &'a mut dyn FnMut(&[DiscoveredProject], i32);

pub fn known_kinds() -> Vec<ProjectKind> {
    backup_template::known_kinds()
}

pub fn kind_to_string(kind: ProjectKind) -> &'static str {
    kind.to_display_string()
}

struct LooseFileScan<'a> {
    projects: Vec<DiscoveredProject>,
    on_directory_scanned: &'a mut dyn FnMut(&str),
    cancelled: Option<&'a AtomicBool>,
    on_progress: &'a mut dyn FnMut(&[DiscoveredProject], i32),
    progress_interval: i32,
    directories_scanned: i32,
}

impl LooseFileScan<'_> {
    fn scan(&mut self, current_folder: &str) {
        if self
            .cancelled
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return;
        }

        (self.on_directory_scanned)(current_folder);
        self.directories_scanned += 1;

        for entry in read_entries(current_folder) {
            if self
                .cancelled
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                return;
            }

            let kind = classify_path(&entry);
            if entry.is_dir {
                // Bundle project formats (for example .logicx and .band) are
                // intentionally excluded from Files mode, including their
                // internal files.
                if kind != ProjectKind::Unknown && backup_template::kind_is_bundle(kind) {
                    continue;
                }
                if project_config::is_ignored_directory_name(&entry.file_name) {
                    continue;
                }
                self.scan(&entry.absolute_path);
                continue;
            }

            // Files mode is extension-driven: only supported regular project
            // files are accepted, and formats declared as bundles are ignored.
            if !entry.is_file
                || kind == ProjectKind::Unknown
                || backup_template::kind_is_bundle(kind)
            {
                continue;
            }

            let mut project = DiscoveredProject {
                root_path: crate::path_cleanup::parent_path(&entry.absolute_path),
                name: complete_base_name(&entry.file_name).to_string(),
                kind,
                primary_file_modified: entry.modified,
                ..Default::default()
            };
            if project.name.is_empty() {
                project.name = entry.file_name.clone();
            }
            project.type_counts.insert(kind, 1);
            project.project_files = vec![entry.file_name.clone()];
            project.primary_project_file = entry.file_name.clone();
            project.total_project_files = 1;
            self.projects.push(project);
        }

        if self.directories_scanned % self.progress_interval == 0 {
            (self.on_progress)(&self.projects, self.directories_scanned);
        }
    }
}

pub fn discover_all(
    selected_folder: &str,
    mut on_directory_scanned: Option<ScanCallback<'_>>,
    cancelled: Option<&AtomicBool>,
    mut on_progress: Option<ProgressCallback<'_>>,
    progress_every_directories: i32,
    layout: ProjectsFolderLayout,
) -> Vec<DiscoveredProject> {
    if selected_folder.is_empty() {
        return Vec::new();
    }

    let root_folder = clean_path(selected_folder);
    let progress_interval = progress_every_directories.max(1);

    if layout == ProjectsFolderLayout::Files {
        let mut noop_scan = |_: &str| {};
        let mut noop_progress = |_: &[DiscoveredProject], _: i32| {};
        let mut scan = LooseFileScan {
            projects: Vec::new(),
            on_directory_scanned: on_directory_scanned
                .as_deref_mut()
                .unwrap_or(&mut noop_scan),
            cancelled,
            on_progress: on_progress.as_deref_mut().unwrap_or(&mut noop_progress),
            progress_interval,
            directories_scanned: 0,
        };
        scan.scan(&root_folder);
        let mut projects = scan.projects;
        projects.sort_by(|left, right| {
            let by_name = left.name.to_lowercase().cmp(&right.name.to_lowercase());
            if by_name != std::cmp::Ordering::Equal {
                return by_name;
            }
            let by_root = left
                .root_path
                .to_lowercase()
                .cmp(&right.root_path.to_lowercase());
            if by_root != std::cmp::Ordering::Equal {
                return by_root;
            }
            left.primary_project_file
                .to_lowercase()
                .cmp(&right.primary_project_file.to_lowercase())
        });
        return projects;
    }

    let mut state = ScanState::default();
    let mut directories_scanned = 0i32;

    // Progress snapshots need read access to `state` while scanning mutates
    // it, so run the scan with an inline callback that rebuilds projects.
    struct ScanCtx<'a> {
        state: ScanState,
        directories_scanned: &'a mut i32,
        progress_interval: i32,
    }

    let mut ctx = ScanCtx {
        state,
        directories_scanned: &mut directories_scanned,
        progress_interval,
    };

    // Manual recursion to interleave progress with state access.
    fn scan_with_progress(
        root_folder: &str,
        current_folder: &str,
        ctx: &mut ScanCtx<'_>,
        on_directory_scanned: &mut Option<ScanCallback<'_>>,
        cancelled: Option<&AtomicBool>,
        on_progress: &mut Option<ProgressCallback<'_>>,
        layout: ProjectsFolderLayout,
    ) {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return;
        }

        if let Some(callback) = on_directory_scanned.as_deref_mut() {
            callback(current_folder);
        }
        *ctx.directories_scanned += 1;
        if let Some(progress) = on_progress.as_deref_mut()
            && *ctx.directories_scanned % ctx.progress_interval == 0
        {
            let partial = build_projects_from_state(&ctx.state);
            progress(&partial, *ctx.directories_scanned);
        }

        for entry in read_entries(current_folder) {
            if entry.is_file {
                register_marker_project(root_folder, &entry, &mut ctx.state);
            }

            let kind = classify_path(&entry);

            if entry.is_dir && kind == ProjectKind::Unknown {
                if project_config::is_ignored_directory_name(&entry.file_name) {
                    continue;
                }
                scan_with_progress(
                    root_folder,
                    &entry.absolute_path,
                    ctx,
                    on_directory_scanned,
                    cancelled,
                    on_progress,
                    layout,
                );
                continue;
            }

            if kind == ProjectKind::Unknown {
                continue;
            }

            if layout == ProjectsFolderLayout::Files && backup_template::kind_is_bundle(kind) {
                continue;
            }

            let folder_path = crate::path_cleanup::parent_path(&entry.absolute_path);
            *ctx.state
                .counts_by_folder
                .entry(folder_path.clone())
                .or_default()
                .entry(kind)
                .or_insert(0) += 1;
            ctx.state
                .files_by_folder
                .entry(folder_path.clone())
                .or_default()
                .push(entry.file_name.clone());
            ctx.state
                .files_by_folder_and_kind
                .entry(folder_path.clone())
                .or_default()
                .entry(kind)
                .or_default()
                .push(entry.file_name.clone());
            ctx.state
                .file_infos_by_folder
                .entry(folder_path)
                .or_default()
                .insert(entry.file_name.clone(), entry.clone());
        }
    }

    scan_with_progress(
        &root_folder,
        &root_folder,
        &mut ctx,
        &mut on_directory_scanned,
        cancelled,
        &mut on_progress,
        layout,
    );

    state = ctx.state;
    build_projects_from_state(&state)
}

/// Resolve a single project from a changed file path (for watcher rediscovery).
pub fn discover_project_for_changed_path(
    selected_folder: &str,
    absolute_changed_path: &str,
    layout: ProjectsFolderLayout,
) -> Option<DiscoveredProject> {
    let clean_folder = clean_path(selected_folder);
    let clean_changed = clean_path(absolute_changed_path);
    if !path_is_under_root(&clean_changed, &clean_folder) {
        return None;
    }

    let relative_path = relative_file_path(&clean_folder, &clean_changed);
    let config = ProjectConfig::new(clean_folder.clone());
    if !config.should_track(&relative_path) {
        return None;
    }

    let project_root = resolve_project_root_absolute(&clean_folder, &relative_path);
    if project_root.is_empty() {
        return None;
    }

    let discovered = discover_project_at(&clean_folder, &project_root)?;

    if layout != ProjectsFolderLayout::Files {
        return Some(discovered);
    }

    let expanded = expand_loose_file_projects(std::slice::from_ref(&discovered));
    if expanded.is_empty() {
        return None;
    }

    let changed_name = file_name(&clean_changed);
    for candidate in &expanded {
        if candidate.primary_project_file == changed_name {
            return Some(candidate.clone());
        }
    }

    Some(expanded[0].clone())
}
