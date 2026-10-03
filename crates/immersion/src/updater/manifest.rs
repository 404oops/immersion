//! Verify the exact manifest bytes before trusting any metadata or URL.

use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub const MAX_MANIFEST: u64 = 16 * 1024;
pub const MAX_INSTALLER: u64 = 256 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    pub target: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn bounded(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("update response too large".into());
    }
    Ok(bytes)
}

pub fn staged(dir: &Path, key: &str, target: &str, current: &str) -> Result<Manifest> {
    let bytes = bounded(File::open(dir.join("manifest.json"))?, MAX_MANIFEST)?;
    let signature = bounded(File::open(dir.join("manifest.sig"))?, 1024)?;
    let manifest = verify(&bytes, std::str::from_utf8(&signature)?, key)?;
    manifest.validate(target, current)?;
    manifest.verify_installer(File::open(dir.join("installer.exe"))?)?;
    Ok(manifest)
}

pub fn verify(bytes: &[u8], signature: &str, key: &str) -> Result<Manifest> {
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("update manifest too large".into());
    }
    let key: [u8; 32] = STANDARD
        .decode(key.trim())?
        .try_into()
        .map_err(|_| "invalid update key")?;
    let signature = Signature::from_slice(&STANDARD.decode(signature.trim())?)?;
    VerifyingKey::from_bytes(&key)?.verify_strict(bytes, &signature)?;
    Ok(serde_json::from_slice(bytes)?)
}

impl Manifest {
    pub fn validate(&self, target: &str, current: &str) -> Result<()> {
        let version = Version::parse(&self.version)?;
        if !version.pre.is_empty()
            || !version.build.is_empty()
            || version <= Version::parse(current)?
        {
            return Err("update must be a newer stable version".into());
        }
        if self.target != target || !matches!(target, "windows-x64" | "windows-arm64") {
            return Err("update target mismatch".into());
        }
        // Only this repository's versioned release assets are executable updates.
        let expected = format!(
            "https://github.com/404oops/immersion/releases/download/v{}/ImmersionSetup-{}-{}.exe",
            self.version, self.version, target
        );
        if self.url != expected || self.size == 0 || self.size > MAX_INSTALLER {
            return Err("invalid update artifact".into());
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err("invalid update checksum".into());
        }
        Ok(())
    }

    pub fn verify_installer(&self, reader: impl Read) -> Result<()> {
        let mut reader = reader.take(self.size + 1);
        let mut hash = Sha256::new();
        let size = io::copy(&mut reader, &mut hash)?;
        if size != self.size || format!("{:x}", hash.finalize()) != self.sha256 {
            return Err("update installer size or checksum mismatch".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn manifest() -> Manifest {
        Manifest {
            version: "0.2.6".into(), target: "windows-x64".into(),
            url: "https://github.com/404oops/immersion/releases/download/v0.2.6/ImmersionSetup-0.2.6-windows-x64.exe".into(),
            size: 3, sha256: format!("{:x}", Sha256::digest(b"exe")),
        }
    }

    #[test]
    fn signed_metadata_rejects_tampering_and_wrong_key() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let mut bytes = serde_json::to_vec(&manifest()).unwrap();
        let signature = STANDARD.encode(key.sign(&bytes).to_bytes());
        let public = STANDARD.encode(key.verifying_key().to_bytes());
        assert!(verify(&bytes, &signature, &public).is_ok());
        bytes[0] ^= 1;
        assert!(verify(&bytes, &signature, &public).is_err());
        bytes[0] ^= 1;
        let wrong = SigningKey::from_bytes(&[8; 32]);
        assert!(
            verify(
                &bytes,
                &signature,
                &STANDARD.encode(wrong.verifying_key().to_bytes())
            )
            .is_err()
        );
        assert!(verify(&bytes, "bad signature", &public).is_err());
        assert!(verify(&vec![0; MAX_MANIFEST as usize + 1], &signature, &public).is_err());
    }

    #[test]
    fn selects_only_newer_stable_matching_artifacts() {
        let mut m = manifest();
        assert!(m.validate("windows-x64", "0.2.5").is_ok());
        assert!(m.validate("windows-arm64", "0.2.5").is_err());
        assert!(m.validate("windows-x64", "0.2.6").is_err());
        assert!(m.validate("windows-x64", "0.3.0").is_err());
        m.version = "0.3.0-beta.1".into();
        assert!(m.validate("windows-x64", "0.2.5").is_err());
        m = manifest();
        m.url.push_str("?other=1");
        assert!(m.validate("windows-x64", "0.2.5").is_err());
        m = manifest();
        m.size = MAX_INSTALLER + 1;
        assert!(m.validate("windows-x64", "0.2.5").is_err());
    }

    #[test]
    fn truncated_corrupted_and_oversized_installers_are_rejected() {
        let m = manifest();
        assert!(m.verify_installer(&b"exe"[..]).is_ok());
        for bytes in [&b"ex"[..], &b"bad"[..], &b"exe extra"[..]] {
            assert!(m.verify_installer(bytes).is_err());
        }
    }

    #[test]
    fn incomplete_or_modified_staged_update_never_installs() {
        let directory = tempfile::tempdir().unwrap();
        let dir = directory.path();
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = STANDARD.encode(key.verifying_key().to_bytes());
        let bytes = serde_json::to_vec(&manifest()).unwrap();
        let signature = STANDARD.encode(key.sign(&bytes).to_bytes());
        let check = || staged(dir, &public, "windows-x64", "0.2.5");
        assert!(check().is_err());
        std::fs::write(dir.join("manifest.json"), &bytes).unwrap();
        std::fs::write(dir.join("manifest.sig"), &signature).unwrap();
        assert!(check().is_err());
        std::fs::write(dir.join("installer.exe"), b"ex").unwrap();
        assert!(check().is_err());
        std::fs::write(dir.join("installer.exe"), b"exe").unwrap();
        assert!(check().is_ok());
        assert!(staged(dir, &public, "windows-arm64", "0.2.5").is_err());
        std::fs::write(dir.join("manifest.json"), b"{}").unwrap();
        assert!(check().is_err());
        assert!(bounded(&b"12345"[..], 4).is_err());
        assert_eq!(bounded(&b"1234"[..], 4).unwrap(), b"1234");
    }
}
