//! Content-addressed object store under `.musit/` (SHA-256 + qCompress zlib
//! objects plus an uncompressed staging area).
//! Port of `qt-legacy/src/persistence/ObjectStore.{h,cpp}`.

use crate::path_cleanup::{clean_path, join_path, parent_path};
use crate::qcompress::{q_compress, q_uncompress};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Atomic write via temp file + rename, the QSaveFile equivalent.
pub fn write_atomically(file_path: &str, bytes: &[u8]) -> bool {
    let parent = parent_path(file_path);
    if parent.is_empty() {
        return false;
    }
    // Exclusive-create a unique temp file next to the target so we never
    // truncate or follow a pre-existing path.
    let mut temp = None;
    for attempt in 0..16u32 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let candidate = format!("{file_path}.tmp{}-{nanos}-{attempt}", std::process::id());
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temp = Some((candidate, file));
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return false,
        }
    }
    let Some((temp_path, mut file)) = temp else {
        return false;
    };
    // Data must reach disk before the rename makes the file visible, or a
    // crash can leave a visible-but-truncated object (QSaveFile::commit gave
    // the same guarantee).
    let written = file.write_all(bytes).is_ok() && file.sync_all().is_ok();
    drop(file);
    if !written || fs::rename(&temp_path, file_path).is_err() {
        let _ = fs::remove_file(&temp_path);
        return false;
    }
    // Best-effort: persist the rename itself.
    #[cfg(unix)]
    if let Ok(dir) = fs::File::open(&parent) {
        let _ = dir.sync_all();
    }
    true
}

fn compressed_object_matches_hash(compressed: &[u8], hash_hex: &str) -> bool {
    if compressed.len() < 4 || hash_hex.len() != 64 {
        return false;
    }
    let Some(decompressed) = q_uncompress(compressed) else {
        return false;
    };
    let actual_hash = hex_lower(&Sha256::digest(&decompressed));
    actual_hash == hash_hex
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct ObjectStore {
    musit_root: String,
}

impl ObjectStore {
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
        fs::create_dir_all(join_path(&self.musit_root, "objects")).is_ok()
            && fs::create_dir_all(join_path(&self.musit_root, "staging")).is_ok()
    }

    /// Copies a file into a fresh timestamped staging directory, preserving
    /// its project-relative path. Returns the absolute staged path.
    pub fn stage_file(&self, absolute_path: &str, relative_path: &str) -> Option<String> {
        // The relative path is used to build a path inside the staging area;
        // refuse anything that could escape it.
        let normalized_relative = clean_path(relative_path);
        // `X:` is a drive prefix only on Windows; on POSIX a ':' is a legal
        // filename character (Finder renders '/' in names as ':').
        if normalized_relative.is_empty()
            || normalized_relative.starts_with("..")
            || normalized_relative.contains("/../")
            || normalized_relative.starts_with('/')
            || (cfg!(windows) && normalized_relative.chars().nth(1) == Some(':'))
        {
            return None;
        }

        let bytes = fs::read(absolute_path).ok()?;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis()
            .to_string();

        let target_dir_path = join_path(&self.musit_root, &format!("staging/{stamp}"));
        fs::create_dir_all(&target_dir_path).ok()?;

        let target_file_path = join_path(&target_dir_path, &normalized_relative);
        fs::create_dir_all(parent_path(&target_file_path)).ok()?;

        if fs::write(&target_file_path, &bytes).is_err() {
            let _ = fs::remove_file(&target_file_path);
            return None;
        }

        Some(target_file_path)
    }

    pub fn object_path_for_hash(&self, object_hash: &str) -> Option<String> {
        // The hash is read from log.jsonl and is untrusted: require ASCII hex
        // so the slices below stay on char boundaries (a multi-byte char
        // would panic) and the path can't be steered elsewhere.
        if self.musit_root.is_empty()
            || object_hash.len() <= 2
            || !object_hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return None;
        }
        Some(join_path(
            &self.musit_root,
            &format!("objects/{}/{}.z", &object_hash[..2], &object_hash[2..]),
        ))
    }

    /// Decompresses the object with the given hash to `dest_path`. Used to
    /// restore versions whose uncompressed staged copy has been compacted.
    pub fn extract_object(&self, object_hash: &str, dest_path: &str) -> bool {
        let Some(object_path) = self.object_path_for_hash(object_hash) else {
            return false;
        };

        let Ok(compressed) = fs::read(&object_path) else {
            return false;
        };
        if !compressed_object_matches_hash(&compressed, object_hash) {
            return false;
        }
        let Some(decompressed) = q_uncompress(&compressed) else {
            return false;
        };

        if fs::write(dest_path, &decompressed).is_err() {
            let _ = fs::remove_file(dest_path);
            return false;
        }
        true
    }

    /// Stores the file content-addressed (SHA-256, qCompress level 6) and
    /// returns the hex hash. Idempotent for existing intact objects; repairs
    /// corrupt ones in place.
    pub fn store_file(&self, absolute_path: &str) -> Option<String> {
        let bytes = fs::read(absolute_path).ok()?;

        let hash_hex = hex_lower(&Sha256::digest(&bytes));
        let shard = &hash_hex[..2];
        let file_part = &hash_hex[2..];

        let shard_dir = join_path(&self.musit_root, &format!("objects/{shard}"));
        fs::create_dir_all(&shard_dir).ok()?;

        let object_path = join_path(&shard_dir, &format!("{file_part}.z"));
        if Path::new(&object_path).exists() {
            if let Ok(existing) = fs::read(&object_path) {
                if compressed_object_matches_hash(&existing, &hash_hex) {
                    return Some(hash_hex);
                }
            }
        }

        let compressed = q_compress(&bytes, 6);
        if !write_atomically(&object_path, &compressed) {
            return None;
        }

        Some(hash_hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_extract_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let musit_root = dir.path().join(".musit");
        let store = ObjectStore::new(musit_root.to_string_lossy().to_string());
        assert!(store.init());

        let source = dir.path().join("file.als");
        fs::write(&source, b"project bytes").unwrap();

        let hash = store.store_file(source.to_str().unwrap()).unwrap();
        assert_eq!(hash.len(), 64);

        let dest = dir.path().join("restored.als");
        assert!(store.extract_object(&hash, dest.to_str().unwrap()));
        assert_eq!(fs::read(&dest).unwrap(), b"project bytes");
    }

    #[test]
    fn stage_rejects_escapes() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path().to_string_lossy().to_string());
        assert!(store.init());
        let source = dir.path().join("f");
        fs::write(&source, b"x").unwrap();
        assert!(
            store
                .stage_file(source.to_str().unwrap(), "../evil")
                .is_none()
        );
        assert!(store.stage_file(source.to_str().unwrap(), "/abs").is_none());
    }
}
