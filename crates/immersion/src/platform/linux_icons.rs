//! Linux: the icon theme's document icon for a file type, found the way file
//! managers find it. The extension maps to a MIME type through the
//! shared-mime-info globs, the MIME type to icon names, and those names to a
//! file through the freedesktop icon theme spec. SVG icons are rendered with
//! resvg, which GPUI already uses.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;

/// `$XDG_DATA_HOME` then `$XDG_DATA_DIRS`, most specific first.
fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        Some(home) => dirs.push(PathBuf::from(home)),
        None => {
            if let Some(home) = dirs::home_dir() {
                dirs.push(home.join(".local/share"));
            }
        }
    }
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(
        system
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from),
    );
    dirs
}

// ---- MIME -------------------------------------------------------------------

/// The MIME type whose `*.extension` glob has the highest weight, from
/// `mime/globs2` (`weight:type:glob[:flags]`). Earlier data directories win
/// ties, so a user's own types override the system's.
fn mime_type(extension: &str) -> Option<String> {
    let lower = extension.to_lowercase();
    let mut best: Option<(u32, String)> = None;
    for dir in data_dirs() {
        let Ok(text) = std::fs::read_to_string(dir.join("mime/globs2")) else {
            continue;
        };
        for line in text.lines().filter(|line| !line.starts_with('#')) {
            let mut fields = line.splitn(4, ':');
            let (Some(weight), Some(mime), Some(glob)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let Some(glob_extension) = glob.strip_prefix("*.") else {
                continue;
            };
            let case_sensitive = fields.next().is_some_and(|flags| flags.contains("cs"));
            let matches = if case_sensitive {
                glob_extension == extension
            } else {
                glob_extension.to_lowercase() == lower
            };
            let weight = weight.parse().unwrap_or(50);
            if matches && best.as_ref().is_none_or(|(top, _)| weight > *top) {
                best = Some((weight, mime.to_string()));
            }
        }
    }
    best.map(|(_, mime)| mime)
}

/// Looks `mime` up in a `type:value` file such as `mime/generic-icons`.
fn mime_table_lookup(file: &str, mime: &str) -> Option<String> {
    data_dirs().into_iter().find_map(|dir| {
        let text = std::fs::read_to_string(dir.join(file)).ok()?;
        text.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key == mime).then(|| value.to_string())
        })
    })
}

/// Icon names for a MIME type, best first: a declared icon, the type's own
/// name, then its generic icon. The catch-all blank page is left out, like
/// the macOS lookup, so unclaimed types keep the app's tile.
fn icon_names(mime: &str) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(icon) = mime_table_lookup("mime/icons", mime) {
        names.push(icon);
    }
    names.push(mime.replace('/', "-"));
    let generic = mime_table_lookup("mime/generic-icons", mime).or_else(|| {
        let media = mime.split('/').next()?;
        (media != "application").then(|| format!("{media}-x-generic"))
    });
    names.extend(generic);
    names.dedup();
    names
}

// ---- Icon themes ------------------------------------------------------------

enum DirKind {
    Fixed,
    Scalable { min: u32, max: u32 },
    Threshold(u32),
}

struct ThemeDir {
    path: String,
    size: u32,
    scale: u32,
    kind: DirKind,
}

impl ThemeDir {
    fn matches(&self, size: u32) -> bool {
        match self.kind {
            DirKind::Fixed => self.size == size,
            DirKind::Scalable { min, max } => (min..=max).contains(&size),
            DirKind::Threshold(threshold) => self.size.abs_diff(size) <= threshold,
        }
    }

    fn distance(&self, size: u32) -> u32 {
        match self.kind {
            DirKind::Fixed => self.size.abs_diff(size),
            DirKind::Scalable { min, max } => min.saturating_sub(size) + size.saturating_sub(max),
            DirKind::Threshold(threshold) => self.size.abs_diff(size).saturating_sub(threshold),
        }
    }
}

struct Theme {
    /// Every base directory holding this theme, searched in order.
    bases: Vec<PathBuf>,
    dirs: Vec<ThemeDir>,
    inherits: Vec<String>,
}

/// `~/.icons`, then `icons` under each data directory.
fn icon_bases() -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if let Some(home) = dirs::home_dir() {
        bases.push(home.join(".icons"));
    }
    bases.extend(data_dirs().into_iter().map(|dir| dir.join("icons")));
    bases
}

/// Parses `index.theme` keys by section.
fn parse_ini(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current = String::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            current = name.to_string();
        } else if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(current.clone())
                .or_default()
                .insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    sections
}

fn load_theme(name: &str) -> Option<Theme> {
    let bases: Vec<PathBuf> = icon_bases()
        .into_iter()
        .map(|base| base.join(name))
        .filter(|dir| dir.is_dir())
        .collect();
    let text = bases
        .iter()
        .find_map(|dir| std::fs::read_to_string(dir.join("index.theme")).ok())?;
    let ini = parse_ini(&text);
    let header = ini.get("Icon Theme")?;
    let list = |key: &str| -> Vec<String> {
        header
            .get(key)
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut dir_names = list("Directories");
    dir_names.extend(list("ScaledDirectories"));
    let dirs = dir_names
        .into_iter()
        .filter_map(|path| {
            let keys = ini.get(&path)?;
            let number = |key: &str| keys.get(key).and_then(|value| value.parse::<u32>().ok());
            let size = number("Size")?;
            let kind = match keys.get("Type").map(String::as_str) {
                Some("Fixed") => DirKind::Fixed,
                Some("Scalable") => DirKind::Scalable {
                    min: number("MinSize").unwrap_or(size),
                    max: number("MaxSize").unwrap_or(size),
                },
                _ => DirKind::Threshold(number("Threshold").unwrap_or(2)),
            };
            Some(ThemeDir {
                path,
                size,
                scale: number("Scale").unwrap_or(1).max(1),
                kind,
            })
        })
        .collect();
    Some(Theme {
        bases,
        dirs,
        inherits: list("Inherits"),
    })
}

/// The desktop's icon theme as the Settings portal reports it. It is the
/// one source that is right inside a Flatpak, whose own settings say
/// Adwaita, and it avoids a `gsettings` on PATH that is not the desktop's
/// (Anaconda ships one that reads its own defaults). Asked on a helper
/// thread, so a portal that never answers costs a second, not a hang.
fn portal_icon_theme() -> Option<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("icon-theme".into())
        .spawn(move || {
            let theme = (|| {
                let connection = zbus::blocking::Connection::session().ok()?;
                let reply = connection
                    .call_method(
                        Some("org.freedesktop.portal.Desktop"),
                        "/org/freedesktop/portal/desktop",
                        Some("org.freedesktop.portal.Settings"),
                        "ReadOne",
                        &("org.gnome.desktop.interface", "icon-theme"),
                    )
                    .ok()?;
                let value: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
                String::try_from(value).ok()
            })();
            let _ = sender.send(theme);
        })
        .ok()?;
    receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .ok()
        .flatten()
        .filter(|name| !name.is_empty())
}

/// The desktop's icon theme: the Settings portal, GNOME-family settings,
/// then GTK's and KDE's config files.
fn current_theme_name() -> Option<String> {
    if let Some(theme) = portal_icon_theme() {
        return Some(theme);
    }
    let gsettings = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .trim_matches('\'')
                .to_string()
        })
        .filter(|name| !name.is_empty());
    if gsettings.is_some() {
        return gsettings;
    }
    let config = dirs::config_dir()?;
    [
        ("gtk-3.0/settings.ini", "Settings", "gtk-icon-theme-name"),
        ("kdeglobals", "Icons", "Theme"),
    ]
    .into_iter()
    .find_map(|(file, section, key)| {
        let text = std::fs::read_to_string(config.join(file)).ok()?;
        parse_ini(&text).get(section)?.get(key).cloned()
    })
}

/// The current theme and everything it inherits, ending with hicolor as
/// the spec requires. Loaded once: the app caches icons per extension too.
static THEMES: Lazy<Vec<Theme>> = Lazy::new(|| {
    let mut names: Vec<String> = Vec::new();
    let mut themes = Vec::new();
    // Depth-first, so each theme's parents follow it in the listed order.
    let mut queue: Vec<String> = current_theme_name().into_iter().collect();
    queue.push("hicolor".into());
    queue.reverse();
    while let Some(name) = queue.pop() {
        if names.contains(&name) {
            continue;
        }
        if let Some(theme) = load_theme(&name) {
            queue.extend(theme.inherits.iter().rev().cloned());
            themes.push(theme);
        }
        names.push(name);
    }
    themes
});

const EXTENSIONS: [&str; 2] = ["svg", "png"];

fn icon_file(theme: &Theme, dir: &ThemeDir, name: &str) -> Option<PathBuf> {
    theme.bases.iter().find_map(|base| {
        EXTENSIONS.iter().find_map(|extension| {
            let path = base.join(&dir.path).join(format!("{name}.{extension}"));
            path.is_file().then_some(path)
        })
    })
}

/// The spec's lookup across themes: in each theme, an exact size match for
/// any name (in order), else the closest size; then the next theme.
fn find_icon(names: &[String], size: u32) -> Option<PathBuf> {
    for theme in THEMES.iter() {
        for name in names {
            for dir in theme.dirs.iter().filter(|dir| dir.scale == 1) {
                if dir.matches(size)
                    && let Some(path) = icon_file(theme, dir, name)
                {
                    return Some(path);
                }
            }
        }
        let closest = names
            .iter()
            .flat_map(|name| {
                theme.dirs.iter().filter_map(move |dir| {
                    Some((dir.distance(size) * dir.scale, icon_file(theme, dir, name)?))
                })
            })
            .min_by_key(|(distance, _)| *distance);
        if let Some((_, path)) = closest {
            return Some(path);
        }
    }
    None
}

// ---- Rendering --------------------------------------------------------------

/// Straight-alpha RGBA of an icon file, at most `size` pixels on a side.
fn rasterize(path: &Path, size: u32) -> Option<(u32, u32, Vec<u8>)> {
    let data = std::fs::read(path).ok()?;
    if path.extension().is_some_and(|extension| extension == "svg") {
        use resvg::{tiny_skia, usvg};
        let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).ok()?;
        let tree_size = tree.size();
        let scale = size as f32 / tree_size.width().max(tree_size.height());
        let width = (tree_size.width() * scale).round().max(1.0) as u32;
        let height = (tree_size.height() * scale).round().max(1.0) as u32;
        let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        let mut rgba = pixmap.take();
        super::icon_pixels::unpremultiply(&mut rgba);
        return Some((width, height, rgba));
    }
    let mut image = image::load_from_memory(&data).ok()?.into_rgba8();
    if image.width().max(image.height()) > size {
        image = image::imageops::resize(&image, size, size, image::imageops::FilterType::Triangle);
    }
    Some((image.width(), image.height(), image.into_raw()))
}

/// The document icon file managers show for `extension`, as a square PNG.
/// None for types the MIME database does not know, so the UI keeps its
/// monogram tile, as it does on macOS for unclaimed types.
pub fn file_type_icon_png(extension: &str, size_px: usize) -> Option<Vec<u8>> {
    if extension.is_empty() || size_px == 0 {
        return None;
    }
    let mime = mime_type(extension)?;
    let path = find_icon(&icon_names(&mime), size_px as u32)?;
    let (width, height, rgba) = rasterize(&path, size_px as u32)?;
    super::icon_pixels::trimmed_png(width, height, &rgba)
}
