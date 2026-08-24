//! Path normalization helpers for stable comparisons across platforms.
//! Port of `qt-legacy/src/core/PathCleanup.h` plus the handful of QDir
//! helpers the C++ code relied on (cleanPath, filePath, relativeFilePath).
//!
//! All paths in musit-core are `String`s with '/' separators, mirroring the
//! Qt convention after `QDir::cleanPath`.

use std::fs;
use std::path::Path;

/// Lexical path cleanup equivalent to `QDir::cleanPath` +
/// `QDir::fromNativeSeparators`: '/' separators, no duplicate slashes,
/// "." removed, ".." resolved where possible, no trailing slash except root.
pub fn clean_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    if path.is_empty() {
        return String::new();
    }

    let absolute = path.starts_with('/');
    // A UNC root ("//server/share") keeps its double slash, like
    // QDir::cleanPath on Windows; three or more slashes collapse.
    let unc = path.starts_with("//") && !path.starts_with("///");
    // Windows drive prefix ("C:") survives as the first component.
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                let can_pop = matches!(parts.last(), Some(&last) if last != ".." && !is_drive(last));
                if can_pop {
                    parts.pop();
                } else if !absolute && parts.last() != Some(&"..") {
                    parts.push("..");
                } else if !absolute {
                    parts.push("..");
                }
                // Leading ".." on an absolute path is dropped, like QDir.
            }
            other => parts.push(other),
        }
    }

    let joined = parts.join("/");
    if unc {
        format!("//{joined}")
    } else if absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    }
}

fn is_drive(component: &str) -> bool {
    component.len() == 2
        && component.ends_with(':')
        && component.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// Normalizes a filesystem path for stable comparisons across platforms.
pub fn normalize_absolute_path(path: &str) -> String {
    clean_path(path)
}

/// Decodes file:// URLs and percent-encoded path segments (e.g. %20) from
/// folder pickers before hitting the filesystem.
pub fn normalize_folder_path(url_or_path: &str) -> String {
    if url_or_path.is_empty() {
        return String::new();
    }

    let mut path = url_or_path.trim().to_string();
    let lower = path.to_lowercase();
    if lower.starts_with("file://") {
        let rest = &path["file://".len()..];
        let decoded = percent_decode(rest);
        path = if let Some(after_slash) = decoded.strip_prefix('/') {
            // file:///path form. A Windows drive ("C:/...") drops the
            // leading slash, matching QUrl::toLocalFile.
            let bytes = after_slash.as_bytes();
            if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
                after_slash.to_string()
            } else {
                decoded.clone()
            }
        } else {
            // file://host/share: "localhost" means the local machine,
            // any other host forms a UNC path (as QUrl::toLocalFile).
            match decoded.split_once('/') {
                Some((host, tail)) if host.eq_ignore_ascii_case("localhost") => {
                    format!("/{tail}")
                }
                _ => format!("//{decoded}"),
            }
        };
    } else if path.contains('%') {
        path = percent_decode(&path);
    }

    normalize_absolute_path(&path)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Case-insensitive on Windows and macOS default volumes; exact on Linux.
pub const fn path_compare_case_insensitive() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

fn eq_with_sensitivity(a: &str, b: &str) -> bool {
    if path_compare_case_insensitive() {
        a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

fn starts_with_sensitivity(haystack: &str, prefix: &str) -> bool {
    if path_compare_case_insensitive() {
        haystack.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        haystack.starts_with(prefix)
    }
}

pub fn path_equals(a: &str, b: &str) -> bool {
    eq_with_sensitivity(&normalize_absolute_path(a), &normalize_absolute_path(b))
}

pub fn path_is_under_root(path: &str, root: &str) -> bool {
    let normalized_path = normalize_absolute_path(path);
    let normalized_root = normalize_absolute_path(root);
    if eq_with_sensitivity(&normalized_path, &normalized_root) {
        return true;
    }
    starts_with_sensitivity(&normalized_path, &format!("{normalized_root}/"))
}

/// Hash-map key for absolute project roots (stable across casing drift).
pub fn path_key(path: &str) -> String {
    let key = normalize_absolute_path(path);
    if path_compare_case_insensitive() {
        key.to_lowercase()
    } else {
        key
    }
}

/// Project-relative artifact ids (e.g. "Song.logicx", "track.als").
pub fn artifact_equals(a: &str, b: &str) -> bool {
    let left = a.replace('\\', "/");
    let right = b.replace('\\', "/");
    eq_with_sensitivity(left.trim(), right.trim())
}

/// `QDir(dir).filePath(name)`: joins unless `name` is already absolute.
/// A drive prefix ("X:") makes a path absolute only on Windows — on POSIX
/// a ':' is an ordinary filename character.
pub fn join_path(dir: &str, name: &str) -> String {
    if name.starts_with('/') || (cfg!(windows) && name.chars().nth(1) == Some(':')) {
        return name.replace('\\', "/");
    }
    if dir.is_empty() {
        return name.replace('\\', "/");
    }
    let dir = dir.trim_end_matches('/');
    format!("{}/{}", dir, name.replace('\\', "/"))
}

/// `QDir(dir).relativeFilePath(path)`: lexical relative path with ".."
/// where needed. Both inputs should be absolute.
pub fn relative_file_path(dir: &str, path: &str) -> String {
    let dir = clean_path(dir);
    let path = clean_path(path);
    if dir.is_empty() {
        return path;
    }

    let dir_parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let mut common = 0usize;
    while common < dir_parts.len()
        && common < path_parts.len()
        && component_eq(dir_parts[common], path_parts[common])
    {
        common += 1;
    }

    let mut out: Vec<String> = Vec::new();
    for _ in common..dir_parts.len() {
        out.push("..".to_string());
    }
    for part in &path_parts[common..] {
        out.push((*part).to_string());
    }

    if out.is_empty() {
        // QDir::relativeFilePath returns "." for identical paths... actually
        // it returns an empty string; callers in this codebase never hit it
        // with identical paths except prefix checks, where "" behaves the
        // same as Qt's result for practical purposes.
        return String::new();
    }
    out.join("/")
}

fn component_eq(a: &str, b: &str) -> bool {
    // Qt compares path components case-insensitively only on Windows in
    // relativeFilePath; keep exact comparison elsewhere. Drive letters on
    // Windows also compare insensitively, which this covers.
    if cfg!(target_os = "windows") {
        a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

/// `QFileInfo(path).fileName()`.
pub fn file_name(path: &str) -> &str {
    match path.rfind('/') {
        Some(idx) => &path[idx + 1..],
        None => path,
    }
}

/// `QFileInfo(path).completeBaseName()`: file name without the last suffix.
pub fn complete_base_name(path: &str) -> &str {
    let name = file_name(path);
    match name.rfind('.') {
        Some(idx) if idx > 0 => &name[..idx],
        _ => name,
    }
}

/// Parent directory of a path ("/a/b/c" -> "/a/b").
pub fn parent_path(path: &str) -> String {
    let cleaned = clean_path(path);
    match cleaned.rfind('/') {
        Some(0) => "/".to_string(),
        Some(idx) => cleaned[..idx].to_string(),
        None => String::new(),
    }
}

/// Removes now-empty directories from `start_dir_path` upwards, never
/// crossing above `stop_dir_path`. Used to keep .musit/staging tidy after
/// staged copies are deleted (compaction or version deletion).
pub fn remove_empty_parent_dirs(start_dir_path: &str, stop_dir_path: &str) {
    let stop = normalize_absolute_path(stop_dir_path);
    let mut current = normalize_absolute_path(start_dir_path);

    let is_same_or_below = |path: &str| -> bool {
        eq_with_sensitivity(path, &stop) || starts_with_sensitivity(path, &format!("{stop}/"))
    };

    while !current.is_empty() && is_same_or_below(&current) {
        let dir = Path::new(&current);
        if !dir.exists() {
            break;
        }

        let is_empty = match fs::read_dir(dir) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => break,
        };
        if !is_empty {
            break;
        }

        let parent = parent_path(&current);
        let _ = fs::remove_dir(dir);

        if eq_with_sensitivity(&current, &stop) {
            break;
        }
        current = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_path_basics() {
        assert_eq!(clean_path("/a/b/../c/./d//e/"), "/a/c/d/e");
        assert_eq!(clean_path("a/b/.."), "a");
        assert_eq!(clean_path("/"), "/");
        assert_eq!(clean_path("C:\\Users\\x\\..\\y"), "C:/Users/y");
    }

    #[test]
    fn clean_path_preserves_unc_roots() {
        assert_eq!(clean_path("\\\\NAS\\Music"), "//NAS/Music");
        assert_eq!(clean_path("//server/share/x/../y"), "//server/share/y");
        // Three or more slashes are not UNC; they collapse.
        assert_eq!(clean_path("///a/b"), "/a/b");
    }

    #[test]
    fn colon_is_a_filename_char_on_posix() {
        if cfg!(windows) {
            assert_eq!(join_path("/root", "C:/abs"), "C:/abs");
        } else {
            // Finder renders '/' in file names as ':' on disk.
            assert_eq!(join_path("/root", "5: Mix.als"), "/root/5: Mix.als");
        }
    }

    #[test]
    fn folder_url_normalization() {
        assert_eq!(normalize_folder_path("file:///Users/x/My%20Music"), "/Users/x/My Music");
        assert_eq!(normalize_folder_path("file://localhost/Users/x"), "/Users/x");
        assert_eq!(normalize_folder_path("file://nas/share"), "//nas/share");
        assert_eq!(normalize_folder_path("file:///C:/Users/x"), "C:/Users/x");
    }

    #[test]
    fn relative_paths() {
        assert_eq!(relative_file_path("/a/b", "/a/b/c/d"), "c/d");
        assert_eq!(relative_file_path("/a/b", "/a/x"), "../x");
        assert!(relative_file_path("/a/b", "/c").starts_with(".."));
    }

    #[test]
    fn under_root() {
        assert!(path_is_under_root("/a/b/c", "/a/b"));
        assert!(path_is_under_root("/a/b", "/a/b"));
        assert!(!path_is_under_root("/a/bc", "/a/b"));
    }
}
