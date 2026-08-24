//! Backup templates: for each supported data type, WHAT gets versioned —
//! the main project file (or directory bundle) plus accompanying manifests,
//! minus bulky media (audio/video/cache) via include/exclude patterns.
//!
//! Port of `qt-legacy/src/core/BackupTemplate.{h,cpp}` and the `ProjectKind`
//! enum from `ProjectDiscovery.h`.
//!
//! Pattern syntax (case-insensitive, '/' separators):
//!   `**` matches anything including '/'
//!   `*`  matches within one path segment
//!   `?`  matches one non-'/' character

use crate::path_cleanup::parent_path;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProjectKind {
    // DawElectronic — clip/pattern-oriented
    Ableton,
    Bitwig,
    FLStudio,
    Reason,
    Renoise,
    Lmms,
    SunVox,
    MuLab,

    // DawTraditional — linear/recording-oriented
    Logic,
    GarageBand,
    Cubase,
    Nuendo,
    Reaper,
    ProTools,
    StudioOne,
    Cakewalk,
    Ardour,
    Samplitude,
    Tracktion,

    // Video — NLE timelines
    DaVinciResolve,
    FinalCutPro,
    PremierePro,
    MediaComposer,
    VegasPro,
    HitFilm,

    // MotionVfx — compositing & motion
    AfterEffects,
    Nuke,
    Fusion,

    // ThreeD — scenes & meshes
    Blender,
    Cinema4D,
    Houdini,
    Maya,
    Lightwave,

    // Photo — raster editing
    Photoshop,
    Gimp,
    Krita,
    AffinityPhoto,
    ClipStudioPaint,
    CaptureOne,

    // VectorIllustration — vector & layout art
    Illustrator,
    AffinityDesigner,
    Inkscape,
    CorelDraw,

    // Publishing — long-form documents
    InDesign,
    Scrivener,
    AffinityPublisher,
    LaTeX,

    // GameDev — engines & editor projects
    Unreal,
    Godot,
    Unity,

    Unknown,
}

impl ProjectKind {
    /// User-facing name; port of `ProjectDiscovery::kindToString`.
    pub fn to_display_string(self) -> &'static str {
        match self {
            ProjectKind::Ableton => "Ableton Live",
            ProjectKind::Bitwig => "Bitwig Studio",
            ProjectKind::FLStudio => "FL Studio",
            ProjectKind::Reason => "Reason",
            ProjectKind::Renoise => "Renoise",
            ProjectKind::Lmms => "LMMS",
            ProjectKind::SunVox => "SunVox",
            ProjectKind::MuLab => "MuLab",
            ProjectKind::Logic => "Logic Pro",
            ProjectKind::GarageBand => "GarageBand",
            ProjectKind::Cubase => "Cubase",
            ProjectKind::Nuendo => "Nuendo",
            ProjectKind::Reaper => "REAPER",
            ProjectKind::ProTools => "Pro Tools",
            ProjectKind::StudioOne => "Studio One",
            ProjectKind::Cakewalk => "Cakewalk",
            ProjectKind::Ardour => "Ardour",
            ProjectKind::Samplitude => "Samplitude",
            ProjectKind::Tracktion => "Tracktion",
            ProjectKind::DaVinciResolve => "DaVinci Resolve",
            ProjectKind::FinalCutPro => "Final Cut Pro",
            ProjectKind::PremierePro => "Premiere Pro",
            ProjectKind::MediaComposer => "Media Composer",
            ProjectKind::VegasPro => "VEGAS Pro",
            ProjectKind::HitFilm => "HitFilm",
            ProjectKind::AfterEffects => "After Effects",
            ProjectKind::Nuke => "Nuke",
            ProjectKind::Fusion => "Fusion",
            ProjectKind::Blender => "Blender",
            ProjectKind::Cinema4D => "Cinema 4D",
            ProjectKind::Houdini => "Houdini",
            ProjectKind::Maya => "Maya",
            ProjectKind::Lightwave => "LightWave 3D",
            ProjectKind::Photoshop => "Photoshop",
            ProjectKind::Gimp => "GIMP",
            ProjectKind::Krita => "Krita",
            ProjectKind::AffinityPhoto => "Affinity Photo",
            ProjectKind::ClipStudioPaint => "Clip Studio Paint",
            ProjectKind::CaptureOne => "Capture One",
            ProjectKind::Illustrator => "Illustrator",
            ProjectKind::AffinityDesigner => "Affinity Designer",
            ProjectKind::Inkscape => "Inkscape",
            ProjectKind::CorelDraw => "CorelDRAW",
            ProjectKind::InDesign => "InDesign",
            ProjectKind::Scrivener => "Scrivener",
            ProjectKind::AffinityPublisher => "Affinity Publisher",
            ProjectKind::LaTeX => "LaTeX",
            ProjectKind::Unreal => "Unreal Engine",
            ProjectKind::Godot => "Godot",
            ProjectKind::Unity => "Unity",
            ProjectKind::Unknown => "Unknown",
        }
    }

    /// Parse the display string back to a kind (used by the registry JSON).
    pub fn from_display_string(value: &str) -> ProjectKind {
        for template in all() {
            if template.kind.to_display_string() == value {
                return template.kind;
            }
        }
        ProjectKind::Unknown
    }
}

/// Category labels map to UI icon groups you can asset independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupCategory {
    DawElectronic,
    DawTraditional,
    Video,
    MotionVfx,
    ThreeD,
    Photo,
    VectorIllustration,
    Publishing,
    GameDev,
}

impl BackupCategory {
    pub fn to_display_string(self) -> &'static str {
        match self {
            BackupCategory::DawElectronic => "DawElectronic",
            BackupCategory::DawTraditional => "DawTraditional",
            BackupCategory::Video => "Video",
            BackupCategory::MotionVfx => "MotionVfx",
            BackupCategory::ThreeD => "ThreeD",
            BackupCategory::Photo => "Photo",
            BackupCategory::VectorIllustration => "VectorIllustration",
            BackupCategory::Publishing => "Publishing",
            BackupCategory::GameDev => "GameDev",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BackupTemplate {
    pub kind: ProjectKind,
    pub category: BackupCategory,
    pub project_file_extensions: &'static [&'static str],
    /// Marker file relative to project root
    /// (e.g. "project.godot", "ProjectSettings/ProjectVersion.txt").
    pub project_root_markers: &'static [&'static str],
    pub project_file_is_bundle: bool,
    pub include_patterns: &'static [&'static str],
    pub exclude_patterns: &'static [&'static str],
}

pub const UNCOMPRESSED_RECENT_VERSIONS: i32 = 5;
pub const MIN_UNCOMPRESSED_RECENT_VERSIONS: i32 = 1;
pub const MAX_UNCOMPRESSED_RECENT_VERSIONS: i32 = 50;

macro_rules! tpl {
    ($kind:expr, $category:expr, $exts:expr, $markers:expr, $bundle:expr, $inc:expr, $exc:expr) => {
        BackupTemplate {
            kind: $kind,
            category: $category,
            project_file_extensions: &$exts,
            project_root_markers: &$markers,
            project_file_is_bundle: $bundle,
            include_patterns: &$inc,
            exclude_patterns: &$exc,
        }
    };
}

static TEMPLATES: Lazy<Vec<BackupTemplate>> = Lazy::new(|| {
    use BackupCategory as C;
    use ProjectKind as K;
    const NONE: [&str; 0] = [];
    vec![
        // ── DawElectronic ────────────────────────────────────────────────
        tpl!(
            K::Ableton,
            C::DawElectronic,
            ["als"],
            NONE,
            false,
            ["**Ableton Project Info/**", "**/Ableton Project Info/**"],
            ["**/Backup/**", "**/Samples/**"]
        ),
        tpl!(
            K::Bitwig,
            C::DawElectronic,
            ["bwproject"],
            NONE,
            false,
            NONE,
            NONE
        ),
        tpl!(
            K::FLStudio,
            C::DawElectronic,
            ["flp"],
            NONE,
            false,
            NONE,
            NONE
        ),
        tpl!(
            K::Reason,
            C::DawElectronic,
            ["reason", "rsn"],
            NONE,
            true,
            ["**.reason/**", "**.rsn/**"],
            [
                "**.reason/Cache/**",
                "**.reason/Samples/**",
                "**.rsn/Cache/**"
            ]
        ),
        tpl!(
            K::Renoise,
            C::DawElectronic,
            ["xrns"],
            NONE,
            false,
            NONE,
            NONE
        ),
        tpl!(
            K::Lmms,
            C::DawElectronic,
            ["mmp", "mmpz"],
            NONE,
            false,
            NONE,
            NONE
        ),
        tpl!(
            K::SunVox,
            C::DawElectronic,
            ["sunvox"],
            NONE,
            false,
            NONE,
            NONE
        ),
        tpl!(
            K::MuLab,
            C::DawElectronic,
            ["muprj"],
            NONE,
            false,
            NONE,
            NONE
        ),
        // ── DawTraditional ───────────────────────────────────────────────
        tpl!(
            K::Logic,
            C::DawTraditional,
            ["logicx"],
            NONE,
            true,
            ["**.logicx/**"],
            [
                "**.logicx/Media/**",
                "**.logicx/Freeze Files/**",
                "**.logicx/Undo Data/**",
                "**.logicx/Alternatives/**/Undo Data/**"
            ]
        ),
        tpl!(
            K::GarageBand,
            C::DawTraditional,
            ["band"],
            NONE,
            true,
            ["**.band/**"],
            [
                "**.band/Media/**",
                "**.band/Output/**",
                "**.band/Freeze Files/**"
            ]
        ),
        tpl!(
            K::Cubase,
            C::DawTraditional,
            ["cpr"],
            NONE,
            false,
            ["**/*.cpr", "**/Images/**"],
            ["**/Audio/**", "**/Edits/**", "**/Track Pictures/**"]
        ),
        tpl!(
            K::Nuendo,
            C::DawTraditional,
            ["npr"],
            NONE,
            false,
            ["**/*.npr", "**/Images/**"],
            ["**/Audio/**", "**/Edits/**"]
        ),
        tpl!(
            K::Reaper,
            C::DawTraditional,
            ["rpp"],
            NONE,
            false,
            ["**/*.rpp", "**/*.RPP", "**/reaper_backups/**"],
            ["**/Media/**", "**/Peaks/**", "**/reaper_peaks/**"]
        ),
        tpl!(
            K::ProTools,
            C::DawTraditional,
            ["ptx", "ptf"],
            NONE,
            false,
            ["**/*.ptx", "**/*.ptf", "**/Session File Backups/**"],
            ["**/Audio Files/**", "**/Fade Files/**", "**/WaveCache/**"]
        ),
        tpl!(
            K::StudioOne,
            C::DawTraditional,
            ["song"],
            NONE,
            false,
            ["**/*.song", "**/History/**"],
            ["**/Media/**", "**/Cache/**"]
        ),
        tpl!(
            K::Cakewalk,
            C::DawTraditional,
            ["cwp"],
            NONE,
            false,
            NONE,
            ["**/Audio/**", "**/Bounce/**"]
        ),
        tpl!(
            K::Ardour,
            C::DawTraditional,
            ["ardour"],
            NONE,
            false,
            ["**/*.ardour", "**/instant.xml", "**/session.state"],
            ["**/interchange/**", "**/export/**", "**/peak/**"]
        ),
        tpl!(
            K::Samplitude,
            C::DawTraditional,
            ["sam"],
            NONE,
            false,
            NONE,
            ["**/Audio/**", "**/Cache/**"]
        ),
        tpl!(
            K::Tracktion,
            C::DawTraditional,
            ["tracktion"],
            NONE,
            false,
            NONE,
            ["**/Recordings/**", "**/Cache/**"]
        ),
        // ── Video ────────────────────────────────────────────────────────
        tpl!(
            K::DaVinciResolve,
            C::Video,
            ["drp"],
            NONE,
            false,
            ["**/*.drp", "**/Resolve Project Backups/**"],
            [
                "**/CacheClip/**",
                "**/Render Cache/**",
                "**/Proxy/**",
                "**/Gallery/**"
            ]
        ),
        tpl!(
            K::FinalCutPro,
            C::Video,
            ["fcpx", "fcpxbundle", "fcpbundle"],
            NONE,
            true,
            ["**.fcpx/**", "**.fcpxbundle/**", "**.fcpbundle/**"],
            [
                "**/Render Files/**",
                "**/Transcoded Media/**",
                "**/High Quality Media/**",
                "**/Proxy Media/**",
                "**/Original Media/**"
            ]
        ),
        tpl!(
            K::PremierePro,
            C::Video,
            ["prproj"],
            NONE,
            false,
            ["**/*.prproj", "**/Adobe Premiere Pro Auto-Save/**"],
            [
                "**/Adobe Premiere Pro Audio Previews/**",
                "**/Adobe Premiere Pro Video Previews/**",
                "**/Media Cache/**",
                "**/Peak Files/**",
                "**/Captured Audio/**",
                "**/Captured Video/**"
            ]
        ),
        tpl!(
            K::MediaComposer,
            C::Video,
            ["avp", "avb"],
            NONE,
            false,
            ["**/*.avp", "**/*.avb", "**/Avid Attic/**"],
            ["**/Avid MediaFiles/**", "**/Audio/**", "**/Rendered/**"]
        ),
        tpl!(
            K::VegasPro,
            C::Video,
            ["veg"],
            NONE,
            false,
            ["**/*.veg", "**/Archive/**"],
            ["**/Media/**", "**/Proxy/**", "**/Render/**"]
        ),
        tpl!(
            K::HitFilm,
            C::Video,
            ["hfcs"],
            NONE,
            false,
            NONE,
            ["**/Media/**", "**/Cache/**", "**/Proxy/**"]
        ),
        // ── MotionVfx ────────────────────────────────────────────────────
        tpl!(
            K::AfterEffects,
            C::MotionVfx,
            ["aep", "aepx"],
            NONE,
            false,
            [
                "**/*.aep",
                "**/*.aepx",
                "**/Adobe After Effects Auto-Save/**"
            ],
            [
                "**/Adobe After Effects Auto-Save/Peak Files/**",
                "**/Media Cache/**"
            ]
        ),
        tpl!(
            K::Nuke,
            C::MotionVfx,
            ["nk"],
            NONE,
            false,
            ["**/*.nk", "**/autosave/**"],
            ["**/Cache/**", "**/Render/**", "**/Previews/**"]
        ),
        tpl!(
            K::Fusion,
            C::MotionVfx,
            ["comp"],
            NONE,
            false,
            ["**/*.comp"],
            ["**/Cache/**", "**/Render/**", "**/Proxy/**"]
        ),
        // ── ThreeD ───────────────────────────────────────────────────────
        tpl!(
            K::Blender,
            C::ThreeD,
            ["blend"],
            NONE,
            false,
            ["**/*.blend", "**/*.blend1"],
            ["**/cache/**", "**/tmp/**"]
        ),
        tpl!(
            K::Cinema4D,
            C::ThreeD,
            ["c4d"],
            NONE,
            false,
            ["**/*.c4d", "**/auto saves/**"],
            ["**/tex/**", "**/cache/**", "**/render/**"]
        ),
        tpl!(
            K::Houdini,
            C::ThreeD,
            ["hip", "hiplc", "hipnc", "hipsc"],
            NONE,
            false,
            [
                "**/*.hip",
                "**/*.hiplc",
                "**/*.hipnc",
                "**/*.hipsc",
                "**/backup/**"
            ],
            ["**/render/**", "**/geo/**/cache/**", "**/sim/**/cache/**"]
        ),
        tpl!(
            K::Maya,
            C::ThreeD,
            ["ma", "mb"],
            NONE,
            false,
            [
                "**/*.ma",
                "**/*.mb",
                "**/incrementalSave/**",
                "**/scenes/**"
            ],
            [
                "**/renderData/**",
                "**/cache/**",
                "**/sourceimages/**/proxy/**"
            ]
        ),
        tpl!(
            K::Lightwave,
            C::ThreeD,
            ["lws"],
            NONE,
            false,
            ["**/*.lws", "**/Scenes/**"],
            ["**/Render/**", "**/Cache/**"]
        ),
        // ── Photo ────────────────────────────────────────────────────────
        tpl!(
            K::Photoshop,
            C::Photo,
            ["psd", "psb"],
            NONE,
            false,
            NONE,
            NONE
        ),
        tpl!(K::Gimp, C::Photo, ["xcf"], NONE, false, NONE, NONE),
        tpl!(
            K::Krita,
            C::Photo,
            ["kra", "krz"],
            NONE,
            false,
            NONE,
            ["**/Thumbnails/**"]
        ),
        tpl!(
            K::AffinityPhoto,
            C::Photo,
            ["afphoto", "afphoto~"],
            NONE,
            true,
            ["**.afphoto/**", "**.afphoto~/**"],
            ["**/Cache/**", "**/Previews/**"]
        ),
        tpl!(
            K::ClipStudioPaint,
            C::Photo,
            ["clip"],
            NONE,
            false,
            ["**/*.clip", "**/DocumentBackup/**"],
            ["**/Cache/**", "**/Export/**"]
        ),
        tpl!(
            K::CaptureOne,
            C::Photo,
            ["cocatalog", "cosession"],
            NONE,
            true,
            ["**.cocatalog/**", "**.cosession/**"],
            [
                "**/Cache/**",
                "**/Previews/**",
                "**/Proxies/**",
                "**/Trash/**"
            ]
        ),
        // ── VectorIllustration ───────────────────────────────────────────
        tpl!(
            K::Illustrator,
            C::VectorIllustration,
            ["ai"],
            NONE,
            false,
            NONE,
            NONE
        ),
        tpl!(
            K::AffinityDesigner,
            C::VectorIllustration,
            ["afdesign", "afdesign~"],
            NONE,
            true,
            ["**.afdesign/**", "**.afdesign~/**"],
            ["**/Cache/**", "**/Previews/**"]
        ),
        tpl!(
            K::Inkscape,
            C::VectorIllustration,
            ["svg"],
            NONE,
            false,
            NONE,
            NONE
        ),
        tpl!(
            K::CorelDraw,
            C::VectorIllustration,
            ["cdr"],
            NONE,
            false,
            NONE,
            ["**/Bitmaps/**", "**/Previews/**"]
        ),
        // ── Publishing ───────────────────────────────────────────────────
        tpl!(
            K::InDesign,
            C::Publishing,
            ["indd", "indt", "idml"],
            NONE,
            false,
            [
                "**/*.indd",
                "**/*.indt",
                "**/*.idml",
                "**/Document fonts/**"
            ],
            ["**/Links/**", "**/Media/**"]
        ),
        tpl!(
            K::Scrivener,
            C::Publishing,
            ["scriv", "scrivx"],
            NONE,
            true,
            ["**.scriv/**", "**.scrivx/**"],
            ["**/Snapshots/**/Attachments/**", "**/QuickLook/**"]
        ),
        tpl!(
            K::AffinityPublisher,
            C::Publishing,
            ["afpub", "afpub~"],
            NONE,
            true,
            ["**.afpub/**", "**.afpub~/**"],
            ["**/Cache/**", "**/Previews/**"]
        ),
        tpl!(
            K::LaTeX,
            C::Publishing,
            ["tex"],
            NONE,
            false,
            ["**/*.tex", "**/*.bib", "**/*.sty", "**/*.cls", "**/*.bst"],
            [
                "**/*.aux",
                "**/*.log",
                "**/*.out",
                "**/*.toc",
                "**/*.synctex.gz",
                "**/*.fls",
                "**/*.fdb_latexmk",
                "**/*.bbl",
                "**/*.blg"
            ]
        ),
        // ── GameDev ──────────────────────────────────────────────────────
        tpl!(
            K::Unreal,
            C::GameDev,
            ["uproject"],
            NONE,
            false,
            [
                "**/*.uproject",
                "**/Config/**",
                "**/Content/**",
                "**/Source/**",
                "**/Plugins/**/Config/**",
                "**/Plugins/**/Source/**"
            ],
            [
                "**/Binaries/**",
                "**/Intermediate/**",
                "**/DerivedDataCache/**",
                "**/Saved/**/Autosaves/**",
                "**/Saved/Logs/**",
                "**/Saved/Crashes/**"
            ]
        ),
        tpl!(
            K::Godot,
            C::GameDev,
            NONE,
            ["project.godot"],
            false,
            [
                "**/project.godot",
                "**/*.tscn",
                "**/*.tres",
                "**/*.gd",
                "**/*.gdshader",
                "**/export_presets.cfg"
            ],
            [
                "**/.import/**",
                "**/.godot/**/cache/**",
                "**/.godot/**/shader_cache/**"
            ]
        ),
        tpl!(
            K::Unity,
            C::GameDev,
            NONE,
            ["ProjectSettings/ProjectVersion.txt"],
            false,
            [
                "**/ProjectSettings/**",
                "**/Packages/manifest.json",
                "**/Packages/packages-lock.json",
                "**/Assets/**",
                "**/Packages/**/package.json"
            ],
            [
                "**/Library/**",
                "**/Temp/**",
                "**/Logs/**",
                "**/Obj/**",
                "**/Build/**",
                "**/UserSettings/Layouts/**"
            ]
        ),
    ]
});

pub fn all() -> &'static [BackupTemplate] {
    &TEMPLATES
}

static GLOBALLY_EXCLUDED_EXTENSIONS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        // Audio
        "wav", "aif", "aiff", "aifc", "mp3", "flac", "ogg", "m4a", "wma", "aac",
        "asd", // Ableton analysis cache
        // Video
        "mp4", "mov", "m4v", "avi", "mkv", "webm", "mxf", "r3d", "braw", "mts", "m2ts", "mpg",
        "mpeg", "wmv", "prores", // Sidecar / cache
        "cfa", "pek", "xmp", "tmp", "lock", "swp",
    ]
    .into_iter()
    .collect()
});

fn extension_of(lower_path: &str) -> &str {
    let slash_index = lower_path.rfind('/').map(|i| i as isize).unwrap_or(-1);
    match lower_path.rfind('.') {
        Some(dot_index) if (dot_index as isize) > slash_index => &lower_path[dot_index + 1..],
        _ => "",
    }
}

fn base_name_of(lower_path: &str) -> &str {
    match lower_path.rfind('/') {
        Some(idx) => &lower_path[idx + 1..],
        None => lower_path,
    }
}

fn compile_pattern(pattern: &str) -> Regex {
    let mut regex = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '*' {
            if i + 1 < chars.len() && chars[i + 1] == '*' {
                regex.push_str(".*");
                i += 1;
            } else {
                regex.push_str("[^/]*");
            }
        } else if c == '?' {
            regex.push_str("[^/]");
        } else {
            regex.push_str(&regex::escape(&c.to_string()));
        }
        i += 1;
    }
    regex.push('$');
    Regex::new(&format!("(?i){regex}")).expect("glob-derived regex is always valid")
}

static PATTERN_CACHE: Lazy<Mutex<HashMap<&'static str, Regex>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn matches_any_pattern(normalized_lower_path: &str, patterns: &[&'static str]) -> bool {
    let mut cache = PATTERN_CACHE.lock().expect("pattern cache poisoned");
    for pattern in patterns {
        let regex = cache
            .entry(pattern)
            .or_insert_with(|| compile_pattern(pattern));
        if regex.is_match(normalized_lower_path) {
            return true;
        }
    }
    false
}

fn normalize_lower(relative_path: &str) -> String {
    relative_path.replace('\\', "/").to_lowercase()
}

pub fn category_for_kind(kind: ProjectKind) -> BackupCategory {
    for tpl in all() {
        if tpl.kind == kind {
            return tpl.category;
        }
    }
    BackupCategory::DawTraditional
}

pub fn kind_for_file_name(file_name: &str) -> ProjectKind {
    let lower_name = file_name.to_lowercase();
    let base_name = base_name_of(&lower_name);

    for tpl in all() {
        for marker in tpl.project_root_markers {
            if base_name == base_name_of(&normalize_lower(marker)) {
                return tpl.kind;
            }
        }
        for extension in tpl.project_file_extensions {
            if lower_name.ends_with(&format!(".{extension}")) {
                return tpl.kind;
            }
        }
    }
    ProjectKind::Unknown
}

pub fn kind_for_root_marker_path(relative_path: &str) -> ProjectKind {
    let normalized = normalize_lower(relative_path);
    for tpl in all() {
        for marker in tpl.project_root_markers {
            if normalized == normalize_lower(marker) {
                return tpl.kind;
            }
        }
    }
    ProjectKind::Unknown
}

/// Given a marker pattern (e.g. "ProjectSettings/ProjectVersion.txt") and the
/// absolute path of the found marker file, returns the project root.
pub fn project_root_for_marker(marker: &str, marker_file_path: &str) -> String {
    let mut root = parent_path(marker_file_path);
    let marker_dir = {
        let cleaned = marker.replace('\\', "/");
        match cleaned.rfind('/') {
            Some(idx) => cleaned[..idx].to_string(),
            None => String::new(),
        }
    };
    if marker_dir.is_empty() || marker_dir == "." {
        return root;
    }

    let segments = marker_dir.split('/').filter(|s| !s.is_empty()).count();
    for _ in 0..segments {
        root = parent_path(&root);
    }
    root
}

pub fn kind_is_bundle(kind: ProjectKind) -> bool {
    for tpl in all() {
        if tpl.kind == kind {
            return tpl.project_file_is_bundle;
        }
    }
    false
}

pub fn should_track_path(relative_path: &str) -> bool {
    if relative_path.is_empty() {
        return false;
    }

    let normalized = normalize_lower(relative_path);

    if GLOBALLY_EXCLUDED_EXTENSIONS.contains(extension_of(&normalized)) {
        return false;
    }

    for tpl in all() {
        if matches_any_pattern(&normalized, tpl.exclude_patterns) {
            return false;
        }
    }

    let base_name = base_name_of(&normalized);
    for tpl in all() {
        for marker in tpl.project_root_markers {
            if base_name == base_name_of(&normalize_lower(marker)) {
                return true;
            }
        }
    }

    let extension = extension_of(&normalized);
    for tpl in all() {
        if !tpl.project_file_is_bundle && tpl.project_file_extensions.contains(&extension) {
            return true;
        }
    }

    for tpl in all() {
        if matches_any_pattern(&normalized, tpl.include_patterns) {
            return true;
        }
    }

    false
}

/// Maps a tracked path to its artifact id: the bundle root when the path is
/// inside a directory bundle (e.g. "Song.logicx/…" -> "Song.logicx"),
/// otherwise the path itself.
pub fn artifact_for_path(relative_path: &str) -> String {
    let mut bundle_suffixes: Vec<String> = Vec::new();
    for tpl in all() {
        if tpl.project_file_is_bundle {
            for extension in tpl.project_file_extensions {
                bundle_suffixes.push(format!(".{extension}"));
            }
        }
    }

    // Walk segments of the original string and lowercase per segment:
    // lowercasing can change byte length (e.g. 'İ' 2->3 bytes), so indices
    // computed on a lowercased copy must never be applied to the original.
    let bytes = relative_path.as_bytes();
    let mut segment_start = 0usize;
    for i in 0..=bytes.len() {
        if i != bytes.len() && bytes[i] != b'/' && bytes[i] != b'\\' {
            continue;
        }
        let segment = relative_path[segment_start..i].to_lowercase();
        for suffix in &bundle_suffixes {
            if segment.ends_with(suffix.as_str()) {
                return relative_path[..i].to_string();
            }
        }
        segment_start = i + 1;
    }

    relative_path.to_string()
}

/// Preference-ordered list of known kinds (template declaration order).
pub fn known_kinds() -> Vec<ProjectKind> {
    all().iter().map(|tpl| tpl.kind).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_detection() {
        assert_eq!(kind_for_file_name("song.als"), ProjectKind::Ableton);
        assert_eq!(kind_for_file_name("Track.logicx"), ProjectKind::Logic);
        assert_eq!(kind_for_file_name("project.godot"), ProjectKind::Godot);
        assert_eq!(kind_for_file_name("readme.txt"), ProjectKind::Unknown);
    }

    #[test]
    fn tracking_rules() {
        assert!(should_track_path("Song.logicx/projectdata"));
        assert!(!should_track_path("Song.logicx/Media/audio.wav"));
        assert!(!should_track_path("Song.logicx/Undo Data/x.plist"));
        assert!(should_track_path("track.als"));
        assert!(!should_track_path("bounce.wav"));
        assert!(should_track_path("ProjectSettings/ProjectVersion.txt"));
    }

    #[test]
    fn artifacts() {
        assert_eq!(artifact_for_path("Song.logicx/projectdata"), "Song.logicx");
        assert_eq!(artifact_for_path("dir/Song.logicx/a/b"), "dir/Song.logicx");
        assert_eq!(artifact_for_path("track.als"), "track.als");
    }

    #[test]
    fn artifacts_with_length_changing_lowercase() {
        // 'İ' (U+0130) grows from 2 to 3 bytes when lowercased; indices from
        // a lowercased copy must never be applied to the original.
        assert_eq!(artifact_for_path("İİ.logicx/Şarkı.wav"), "İİ.logicx");
        assert_eq!(
            artifact_for_path("İstanbul/Song.logicx/a"),
            "İstanbul/Song.logicx"
        );
        assert_eq!(artifact_for_path("İ.als"), "İ.als");
    }

    #[test]
    fn marker_root() {
        assert_eq!(
            project_root_for_marker(
                "ProjectSettings/ProjectVersion.txt",
                "/x/Game/ProjectSettings/ProjectVersion.txt"
            ),
            "/x/Game"
        );
        assert_eq!(
            project_root_for_marker("project.godot", "/x/Game/project.godot"),
            "/x/Game"
        );
    }
}
