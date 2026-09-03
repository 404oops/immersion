//! Content-addressed object store under `.musit/` (SHA-256 + qCompress zlib
//! objects plus an uncompressed staging area).
//! Port of `qt-legacy/src/persistence/ObjectStore.{h,cpp}`.

use crate::path_cleanup::{clean_path, join_path, parent_path};
use crate::qcompress::{QCompressWriter, QUncompressReader};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Chunk size for streaming reads; objects and staged copies can be tens of
/// megabytes, and none of the paths below may buffer a whole file.
const STREAM_BUFFER_BYTES: usize = 256 * 1024;

/// Exclusive-creates a unique temp file next to `target_path` so we never
/// truncate or follow a pre-existing path.
fn create_exclusive_temp(target_path: &str) -> Option<(String, fs::File)> {
    for attempt in 0..16u32 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let candidate = format!("{target_path}.tmp{}-{nanos}-{attempt}", std::process::id());
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Some((candidate, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

/// Atomic write via temp file + rename, the QSaveFile equivalent. `write`
/// streams the content into the temp file and reports success.
pub fn write_atomically_with(file_path: &str, write: impl FnOnce(&mut fs::File) -> bool) -> bool {
    let parent = parent_path(file_path);
    if parent.is_empty() {
        return false;
    }
    let Some((temp_path, mut file)) = create_exclusive_temp(file_path) else {
        return false;
    };
    // Data must reach disk before the rename makes the file visible, or a
    // crash can leave a visible-but-truncated object (QSaveFile::commit gave
    // the same guarantee).
    let written = write(&mut file) && file.sync_all().is_ok();
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

/// Atomic write of an in-memory buffer; see [`write_atomically_with`].
pub fn write_atomically(file_path: &str, bytes: &[u8]) -> bool {
    write_atomically_with(file_path, |file| file.write_all(bytes).is_ok())
}

/// Streams the object file through decompression, comparing the content's
/// SHA-256 — constant memory regardless of object size.
fn object_file_matches_hash(object_path: &str, hash_hex: &str) -> bool {
    if hash_hex.len() != 64 {
        return false;
    }
    let Ok(file) = fs::File::open(object_path) else {
        return false;
    };
    let Some(mut decoder) = QUncompressReader::new(BufReader::new(file)) else {
        return false;
    };
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; STREAM_BUFFER_BYTES];
    loop {
        match decoder.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(_) => return false,
        }
    }
    hex_lower(&hasher.finalize()) == hash_hex
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

        // Open before creating any directories: an unreadable or vanished
        // source must fail with zero filesystem side effects.
        let mut source = fs::File::open(absolute_path).ok()?;
        if !source.metadata().ok()?.is_file() {
            return None;
        }

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis()
            .to_string();

        let target_dir_path = join_path(&self.musit_root, &format!("staging/{stamp}"));
        fs::create_dir_all(&target_dir_path).ok()?;

        let target_file_path = join_path(&target_dir_path, &normalized_relative);
        fs::create_dir_all(parent_path(&target_file_path)).ok()?;

        // Stream instead of round-tripping the whole file through memory.
        // File::create (not fs::copy) so the staged copy gets default
        // writable permissions: compaction and restore-temp cleanup delete
        // staged copies, which a copied read-only attribute would break.
        let Ok(mut target) = fs::File::create(&target_file_path) else {
            return None;
        };
        if std::io::copy(&mut source, &mut target).is_err() {
            drop(target);
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

    /// Decompresses the object with the given hash to `dest_path`, verifying
    /// the content hash as it streams (temp file + rename, so a corrupt
    /// object never leaves partial output at `dest_path`). Used to restore
    /// versions whose uncompressed staged copy has been compacted.
    pub fn extract_object(&self, object_hash: &str, dest_path: &str) -> bool {
        let Some(object_path) = self.object_path_for_hash(object_hash) else {
            return false;
        };
        if object_hash.len() != 64 {
            return false;
        }

        let Ok(file) = fs::File::open(&object_path) else {
            return false;
        };
        let Some(mut decoder) = QUncompressReader::new(BufReader::new(file)) else {
            return false;
        };

        write_atomically_with(dest_path, |dest| {
            let mut hasher = Sha256::new();
            let mut buffer = vec![0u8; STREAM_BUFFER_BYTES];
            loop {
                match decoder.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        hasher.update(&buffer[..n]);
                        if dest.write_all(&buffer[..n]).is_err() {
                            return false;
                        }
                    }
                    Err(_) => return false,
                }
            }
            hex_lower(&hasher.finalize()) == object_hash
        })
    }

    fn object_shard_and_path(&self, hash_hex: &str) -> (String, String) {
        let shard_dir = join_path(&self.musit_root, &format!("objects/{}", &hash_hex[..2]));
        let object_path = join_path(&shard_dir, &format!("{}.z", &hash_hex[2..]));
        (shard_dir, object_path)
    }

    /// Stores the file content-addressed (SHA-256, qCompress level 6) and
    /// returns the hex hash. Idempotent for existing intact objects; repairs
    /// corrupt ones in place. Both passes stream in fixed-size chunks.
    pub fn store_file(&self, absolute_path: &str) -> Option<String> {
        // Pass 1: hash only, so re-storing already-present content (every
        // unchanged file of a bundle baseline) stays a read-only operation —
        // no compression, no temp write, no fsync.
        let mut source = fs::File::open(absolute_path).ok()?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; STREAM_BUFFER_BYTES];
        loop {
            match source.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => hasher.update(&buffer[..n]),
                Err(_) => return None,
            }
        }
        let hash_hex = hex_lower(&hasher.finalize());
        let (_, object_path) = self.object_shard_and_path(&hash_hex);
        if Path::new(&object_path).exists() && object_file_matches_hash(&object_path, &hash_hex) {
            return Some(hash_hex);
        }

        // Pass 2: compress into a temp file while re-hashing. The object's
        // content-addressed name isn't known until the bytes are read, so
        // the temp is renamed into place (or discarded) afterwards. The
        // re-hash keeps content addressing honest if the file changed
        // between the passes: the object is stored under the hash of the
        // bytes actually compressed.
        let mut source = fs::File::open(absolute_path).ok()?;
        let source_len = source.metadata().ok()?.len();

        let objects_dir = join_path(&self.musit_root, "objects");
        fs::create_dir_all(&objects_dir).ok()?;
        let (temp_path, mut temp_file) =
            create_exclusive_temp(&join_path(&objects_dir, "incoming"))?;

        let discard_temp = |temp_path: &str| {
            let _ = fs::remove_file(temp_path);
        };

        let mut hasher = Sha256::new();
        let streamed = (|| {
            let mut writer = QCompressWriter::new(&mut temp_file, source_len, 6).ok()?;
            loop {
                match source.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        hasher.update(&buffer[..n]);
                        writer.write_all(&buffer[..n]).ok()?;
                    }
                    Err(_) => return None,
                }
            }
            writer.finish().ok()?;
            Some(())
        })();
        // Same durability contract as write_atomically: data reaches disk
        // before the rename makes the object visible.
        let flushed = streamed.is_some() && temp_file.sync_all().is_ok();
        drop(temp_file);
        if !flushed {
            discard_temp(&temp_path);
            return None;
        }

        let hash_hex = hex_lower(&hasher.finalize());
        let (shard_dir, object_path) = self.object_shard_and_path(&hash_hex);
        if Path::new(&object_path).exists() && object_file_matches_hash(&object_path, &hash_hex) {
            discard_temp(&temp_path);
            return Some(hash_hex);
        }

        if fs::create_dir_all(&shard_dir).is_err() || fs::rename(&temp_path, &object_path).is_err()
        {
            discard_temp(&temp_path);
            return None;
        }
        // Best-effort: persist the rename itself.
        #[cfg(unix)]
        if let Ok(dir) = fs::File::open(&shard_dir) {
            let _ = dir.sync_all();
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
    fn extract_rejects_corrupt_object_without_touching_dest() {
        let dir = tempfile::tempdir().unwrap();
        let musit_root = dir.path().join(".musit");
        let store = ObjectStore::new(musit_root.to_string_lossy().to_string());
        assert!(store.init());

        let source = dir.path().join("file.als");
        fs::write(&source, b"project bytes").unwrap();
        let hash = store.store_file(source.to_str().unwrap()).unwrap();

        // Corrupt the stored object in place.
        let object_path = store.object_path_for_hash(&hash).unwrap();
        fs::write(&object_path, b"\x00\x00\x00\x0dgarbage").unwrap();

        let dest = dir.path().join("restored.als");
        fs::write(&dest, b"pre-existing").unwrap();
        assert!(!store.extract_object(&hash, dest.to_str().unwrap()));
        // A failed restore must leave any existing destination intact.
        assert_eq!(fs::read(&dest).unwrap(), b"pre-existing");

        // store_file repairs the corrupt object in place.
        let repaired = store.store_file(source.to_str().unwrap()).unwrap();
        assert_eq!(repaired, hash);
        assert!(store.extract_object(&hash, dest.to_str().unwrap()));
        assert_eq!(fs::read(&dest).unwrap(), b"project bytes");
    }

    #[test]
    fn store_and_extract_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let musit_root = dir.path().join(".musit");
        let store = ObjectStore::new(musit_root.to_string_lossy().to_string());
        assert!(store.init());

        let source = dir.path().join("empty.als");
        fs::write(&source, b"").unwrap();
        let hash = store.store_file(source.to_str().unwrap()).unwrap();
        // Storing the same content again hits the intact-object fast path.
        assert_eq!(store.store_file(source.to_str().unwrap()).unwrap(), hash);

        let dest = dir.path().join("restored.als");
        assert!(store.extract_object(&hash, dest.to_str().unwrap()));
        assert_eq!(fs::read(&dest).unwrap(), b"");
    }

    #[test]
    #[cfg(unix)]
    fn staged_copy_of_read_only_source_is_writable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let musit_root = dir.path().join(".musit");
        let store = ObjectStore::new(musit_root.to_string_lossy().to_string());
        assert!(store.init());

        let source = dir.path().join("locked.als");
        fs::write(&source, b"project bytes").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o444)).unwrap();

        let staged = store
            .stage_file(source.to_str().unwrap(), "locked.als")
            .unwrap();
        // Compaction deletes staged copies later; a copied read-only
        // attribute would break that (and restores) on Windows.
        let mode = fs::metadata(&staged).unwrap().permissions().mode();
        assert_ne!(mode & 0o200, 0, "staged copy must stay owner-writable");
        assert_eq!(fs::read(&staged).unwrap(), b"project bytes");
    }

    #[test]
    fn re_store_of_existing_object_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let musit_root = dir.path().join(".musit");
        let store = ObjectStore::new(musit_root.to_string_lossy().to_string());
        assert!(store.init());

        let source = dir.path().join("file.als");
        fs::write(&source, b"project bytes").unwrap();
        let hash = store.store_file(source.to_str().unwrap()).unwrap();
        let object_path = store.object_path_for_hash(&hash).unwrap();
        let modified_before = fs::metadata(&object_path).unwrap().modified().unwrap();

        // The dedupe fast path must not rewrite the object or leak temps.
        assert_eq!(store.store_file(source.to_str().unwrap()).unwrap(), hash);
        let modified_after = fs::metadata(&object_path).unwrap().modified().unwrap();
        assert_eq!(modified_before, modified_after);
        let objects_dir = musit_root.join("objects");
        let stray: Vec<String> = std::fs::read_dir(&objects_dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".tmp"))
            .collect();
        assert!(stray.is_empty(), "leaked temp files: {stray:?}");
    }

    #[test]
    fn failed_stage_leaves_no_staging_directories() {
        let dir = tempfile::tempdir().unwrap();
        let musit_root = dir.path().join(".musit");
        let store = ObjectStore::new(musit_root.to_string_lossy().to_string());
        assert!(store.init());

        // Missing source: must fail before creating any staging dirs.
        let missing = dir.path().join("gone.als");
        assert!(
            store
                .stage_file(missing.to_str().unwrap(), "sub/dir/gone.als")
                .is_none()
        );
        let staging = musit_root.join("staging");
        assert_eq!(
            fs::read_dir(&staging).unwrap().count(),
            0,
            "failed stage must not leave timestamped staging dirs"
        );
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

