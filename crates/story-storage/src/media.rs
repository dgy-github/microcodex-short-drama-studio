//! Immutable binary media retention owned by the trusted Rust storage layer.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const INDEX_SCHEMA: &str = "media-artifact-index/v1";
const MAX_MEDIA_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MediaArtifactRef {
    pub schema: &'static str,
    pub project_id: String,
    pub request_id: String,
    pub kind: MediaKind,
    pub mime_type: String,
    pub content_ref: String,
    pub content_sha256: String,
    pub byte_len: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum MediaStoreError {
    #[error("media artifact identity is invalid")]
    InvalidIdentity,
    #[error("media artifact bytes or mime type are invalid")]
    InvalidMedia,
    #[error("media artifact reference is invalid")]
    InvalidReference,
    #[error("media artifact digest mismatch")]
    HashMismatch,
    #[error("media artifact index is corrupt")]
    CorruptIndex,
    #[error("media store io failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("media metadata encoding failed: {0}")]
    Encoding(#[from] serde_json::Error),
}

pub struct MediaArtifactStore {
    root: PathBuf,
}

impl MediaArtifactStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, MediaStoreError> {
        let root = root.into();
        fs::create_dir_all(root.join("media-blobs"))?;
        fs::create_dir_all(root.join("media-projects"))?;
        Ok(Self { root })
    }

    pub fn put(
        &self,
        project_id: &str,
        request_id: &str,
        kind: MediaKind,
        mime_type: &str,
        bytes: &[u8],
    ) -> Result<MediaArtifactRef, MediaStoreError> {
        if !valid_id(project_id) || !valid_id(request_id) {
            return Err(MediaStoreError::InvalidIdentity);
        }
        if bytes.is_empty() || bytes.len() > MAX_MEDIA_BYTES || !valid_mime(kind, mime_type) {
            return Err(MediaStoreError::InvalidMedia);
        }
        let digest = sha256_hex(bytes);
        let blob = self.blob_path(&digest);
        if blob.exists() {
            if fs::read(&blob)? != bytes {
                return Err(MediaStoreError::HashMismatch);
            }
        } else {
            atomic_create(&blob, bytes)?;
        }
        let reference = MediaArtifactRef {
            schema: "media-artifact-ref/v1",
            project_id: project_id.into(),
            request_id: request_id.into(),
            kind,
            mime_type: mime_type.into(),
            content_ref: format!("artifact://sha256/{digest}"),
            content_sha256: digest,
            byte_len: bytes.len(),
        };
        self.append_index(&reference)?;
        Ok(reference)
    }

    pub fn load(&self, reference: &MediaArtifactRef) -> Result<Vec<u8>, MediaStoreError> {
        let expected = format!("artifact://sha256/{}", reference.content_sha256);
        if reference.content_ref != expected || !valid_digest(&reference.content_sha256) {
            return Err(MediaStoreError::InvalidReference);
        }
        let bytes = fs::read(self.blob_path(&reference.content_sha256))?;
        if sha256_hex(&bytes) != reference.content_sha256 {
            return Err(MediaStoreError::HashMismatch);
        }
        Ok(bytes)
    }

    /// Load an artifact only after proving it belongs to the requested project.
    pub fn load_project_artifact(
        &self,
        project_id: &str,
        content_ref: &str,
    ) -> Result<(MediaArtifactRef, Vec<u8>), MediaStoreError> {
        if !valid_id(project_id) {
            return Err(MediaStoreError::InvalidIdentity);
        }
        let digest = content_ref
            .strip_prefix("artifact://sha256/")
            .filter(|value| valid_digest(value))
            .ok_or(MediaStoreError::InvalidReference)?;
        let path = self
            .root
            .join("media-projects")
            .join(project_id)
            .join("index.json");
        let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
        let entries = value["entries"]
            .as_array()
            .ok_or(MediaStoreError::CorruptIndex)?;
        let entry = entries
            .iter()
            .find(|entry| entry["content_ref"] == content_ref)
            .ok_or(MediaStoreError::InvalidReference)?;
        let reference = parse_reference(entry)?;
        if reference.project_id != project_id || reference.content_sha256 != digest {
            return Err(MediaStoreError::InvalidReference);
        }
        let bytes = self.load(&reference)?;
        Ok((reference, bytes))
    }

    pub fn verify_project_image(
        &self,
        project_id: &str,
        content_ref: &str,
    ) -> Result<(), MediaStoreError> {
        if !valid_id(project_id) {
            return Err(MediaStoreError::InvalidIdentity);
        }
        let digest = content_ref
            .strip_prefix("artifact://sha256/")
            .filter(|value| valid_digest(value))
            .ok_or(MediaStoreError::InvalidReference)?;
        let path = self
            .root
            .join("media-projects")
            .join(project_id)
            .join("index.json");
        let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
        let entries = value["entries"]
            .as_array()
            .ok_or(MediaStoreError::CorruptIndex)?;
        if !entries.iter().any(|entry| {
            entry["kind"] == "image"
                && entry["content_ref"] == content_ref
                && entry["content_sha256"] == digest
        }) {
            return Err(MediaStoreError::InvalidReference);
        }
        let bytes = fs::read(self.blob_path(digest))?;
        if sha256_hex(&bytes) != digest {
            return Err(MediaStoreError::HashMismatch);
        }
        Ok(())
    }

    fn append_index(&self, reference: &MediaArtifactRef) -> Result<(), MediaStoreError> {
        let path = self
            .root
            .join("media-projects")
            .join(&reference.project_id)
            .join("index.json");
        let mut entries = if path.exists() {
            let value: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
            if value["schema"] != INDEX_SCHEMA {
                return Err(MediaStoreError::CorruptIndex);
            }
            value["entries"]
                .as_array()
                .cloned()
                .ok_or(MediaStoreError::CorruptIndex)?
        } else {
            Vec::new()
        };
        if entries.iter().any(|entry| {
            entry["request_id"] == reference.request_id
                && entry["content_sha256"] == reference.content_sha256
        }) {
            return Ok(());
        }
        entries.push(serde_json::to_value(reference)?);
        let index = serde_json::json!({
            "schema": INDEX_SCHEMA,
            "project_id": reference.project_id,
            "entries": entries,
        });
        atomic_replace(&path, &serde_json::to_vec(&index)?)
    }

    fn blob_path(&self, digest: &str) -> PathBuf {
        self.root
            .join("media-blobs")
            .join(&digest[..2])
            .join(digest)
    }
}

fn parse_reference(value: &serde_json::Value) -> Result<MediaArtifactRef, MediaStoreError> {
    if value["schema"] != "media-artifact-ref/v1" {
        return Err(MediaStoreError::CorruptIndex);
    }
    let text = |name: &str| {
        value[name]
            .as_str()
            .map(str::to_owned)
            .ok_or(MediaStoreError::CorruptIndex)
    };
    let project_id = text("project_id")?;
    let request_id = text("request_id")?;
    let mime_type = text("mime_type")?;
    let content_ref = text("content_ref")?;
    let content_sha256 = text("content_sha256")?;
    let byte_len = value["byte_len"]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or(MediaStoreError::CorruptIndex)?;
    let kind = match value["kind"].as_str() {
        Some("image") => MediaKind::Image,
        Some("video") => MediaKind::Video,
        _ => return Err(MediaStoreError::CorruptIndex),
    };
    if !valid_id(&project_id)
        || !valid_id(&request_id)
        || !valid_mime(kind, &mime_type)
        || !valid_digest(&content_sha256)
        || content_ref != format!("artifact://sha256/{content_sha256}")
        || byte_len == 0
    {
        return Err(MediaStoreError::CorruptIndex);
    }
    Ok(MediaArtifactRef {
        schema: "media-artifact-ref/v1",
        project_id,
        request_id,
        kind,
        mime_type,
        content_ref,
        content_sha256,
        byte_len,
    })
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_mime(kind: MediaKind, value: &str) -> bool {
    match kind {
        MediaKind::Image => matches!(value, "image/png" | "image/jpeg" | "image/webp"),
        MediaKind::Video => matches!(value, "video/mp4" | "video/webm"),
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn atomic_create(path: &Path, bytes: &[u8]) -> Result<(), MediaStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), MediaStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("{}.partial", Uuid::new_v4().simple()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_is_content_addressed_append_only_and_digest_verified() {
        let directory = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(directory.path()).unwrap();
        let first = store
            .put(
                "project_1",
                "img_a",
                MediaKind::Image,
                "image/png",
                b"png-fixture",
            )
            .unwrap();
        let again = store
            .put(
                "project_1",
                "img_a",
                MediaKind::Image,
                "image/png",
                b"png-fixture",
            )
            .unwrap();
        assert_eq!(first, again);
        assert_eq!(store.load(&first).unwrap(), b"png-fixture");
        let second = store
            .put(
                "project_1",
                "img_b",
                MediaKind::Image,
                "image/png",
                b"other",
            )
            .unwrap();
        assert_ne!(first.content_ref, second.content_ref);
        store
            .verify_project_image("project_1", &first.content_ref)
            .unwrap();
        assert!(store
            .verify_project_image("another_project", &first.content_ref)
            .is_err());
        let (loaded, bytes) = store
            .load_project_artifact("project_1", &first.content_ref)
            .unwrap();
        assert_eq!(loaded, first);
        assert_eq!(bytes, b"png-fixture");
        assert!(store
            .load_project_artifact("another_project", &first.content_ref)
            .is_err());
    }

    #[test]
    fn unsafe_identity_wrong_mime_and_empty_media_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(directory.path()).unwrap();
        assert!(store
            .put("../x", "img_a", MediaKind::Image, "image/png", b"x")
            .is_err());
        assert!(store
            .put("p", "img_a", MediaKind::Image, "video/mp4", b"x")
            .is_err());
        assert!(store
            .put("p", "img_a", MediaKind::Image, "image/png", b"")
            .is_err());
    }
}
