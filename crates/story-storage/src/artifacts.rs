//! Content-addressed retained artifact store (CAP-006 persistence).
//!
//! The online-policy contract forbids discarding rejected candidates: every
//! `t06` candidate, loser included, must survive the run with its identity so
//! offline comparison and `proxy_fidelity` remain computable. This store is
//! that persistence: artifacts are hashed, written once under their digest,
//! and never rewritten; a per-run append-only index maps task ids to refs.
//!
//! Layout under the store root:
//! ```text
//! blobs/<sha[:2]>/<sha256>       artifact bytes (JSON, canonically encoded)
//! runs/<run_id>/index.json       retained-artifact-index/v1, append-only
//! ```

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const RETAINED_INDEX_SCHEMA: &str = "retained-artifact-index/v1";
pub const ARTIFACT_REF_SCHEMA: &str = "story-artifact-ref/v1";

#[derive(Debug, thiserror::Error)]
pub enum RetainedStoreError {
    #[error("retained store identity is malformed (run id or task id)")]
    InvalidIdentity,
    #[error("retained content reference is malformed")]
    InvalidReference,
    #[error("retained blob bytes do not match the recorded hash")]
    HashMismatch,
    #[error("retained artifact is not valid JSON")]
    InvalidArtifact,
    #[error("retained index is corrupt: {0}")]
    CorruptIndex(String),
    #[error("retained store io failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("retained artifact encoding failed: {0}")]
    Encoding(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RetainedArtifactRef {
    pub schema: &'static str,
    pub artifact_schema: String,
    pub content_ref: String,
    pub content_sha256: String,
}

pub struct ContentAddressedStore {
    root: PathBuf,
}

fn valid_run_id(run_id: &str) -> bool {
    !run_id.is_empty()
        && run_id.len() <= 96
        && run_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn valid_task_id(task_id: &str) -> bool {
    let Some(digits) = task_id.strip_prefix('t') else {
        return false;
    };
    digits.len() == 2
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && digits
            .parse::<u8>()
            .map(|number| (1..=17).contains(&number))
            .unwrap_or(false)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// serde_json's map is ordered by default (no `preserve_order` feature), so
/// `to_vec` yields the same canonical bytes for the same logical value.
fn canonical_bytes(artifact: &Value) -> Result<Vec<u8>, RetainedStoreError> {
    serde_json::to_vec(artifact).map_err(|_| RetainedStoreError::InvalidArtifact)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), RetainedStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    {
        let mut handle = fs::File::create(&temporary)?;
        handle.write_all(bytes)?;
        handle.flush()?;
    }
    fs::rename(&temporary, path)?;
    Ok(())
}

impl ContentAddressedStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, RetainedStoreError> {
        let root = root.into();
        fs::create_dir_all(root.join("blobs"))?;
        fs::create_dir_all(root.join("runs"))?;
        Ok(Self { root })
    }

    fn blob_path(&self, sha256: &str) -> PathBuf {
        self.root.join("blobs").join(&sha256[..2]).join(sha256)
    }

    fn index_path(&self, run_id: &str) -> PathBuf {
        self.root.join("runs").join(run_id).join("index.json")
    }

    /// Retain one artifact. Idempotent for the same task and content; a
    /// changed artifact for the same task appends history (nothing is ever
    /// rewritten or dropped).
    pub fn put(
        &self,
        run_id: &str,
        task_id: &str,
        artifact_schema: &str,
        artifact: &Value,
    ) -> Result<RetainedArtifactRef, RetainedStoreError> {
        if !valid_run_id(run_id) || !valid_task_id(task_id) || artifact_schema.is_empty() {
            return Err(RetainedStoreError::InvalidIdentity);
        }
        let bytes = canonical_bytes(artifact)?;
        let sha256 = sha256_hex(&bytes);
        let blob = self.blob_path(&sha256);
        if blob.exists() {
            let existing = fs::read(&blob)?;
            if existing != bytes {
                return Err(RetainedStoreError::HashMismatch);
            }
        } else {
            atomic_write(&blob, &bytes)?;
        }

        let entry = serde_json::json!({
            "task_id": task_id,
            "artifact_schema": artifact_schema,
            "content_ref": format!("artifact://sha256/{sha256}"),
            "content_sha256": sha256,
            "byte_len": bytes.len(),
        });
        let index_path = self.index_path(run_id);
        let mut entries = self.read_index(run_id)?;
        if entries.iter().any(|existing| {
            existing["content_sha256"] == entry["content_sha256"]
                && existing["task_id"] == entry["task_id"]
        }) {
            return Ok(RetainedArtifactRef {
                schema: ARTIFACT_REF_SCHEMA,
                artifact_schema: artifact_schema.to_string(),
                content_ref: format!("artifact://sha256/{sha256}"),
                content_sha256: sha256,
            });
        }
        entries.push(entry);
        let index = serde_json::json!({
            "schema": RETAINED_INDEX_SCHEMA,
            "run_id": run_id,
            "entries": entries,
        });
        atomic_write(&index_path, serde_json::to_vec(&index)?.as_slice())?;

        Ok(RetainedArtifactRef {
            schema: ARTIFACT_REF_SCHEMA,
            artifact_schema: artifact_schema.to_string(),
            content_ref: format!("artifact://sha256/{sha256}"),
            content_sha256: sha256,
        })
    }

    /// Read the append-only retention index of one run.
    pub fn read_index(&self, run_id: &str) -> Result<Vec<Value>, RetainedStoreError> {
        if !valid_run_id(run_id) {
            return Err(RetainedStoreError::InvalidIdentity);
        }
        let path = self.index_path(run_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)?;
        let parsed: Value = serde_json::from_str(&raw)
            .map_err(|error| RetainedStoreError::CorruptIndex(error.to_string()))?;
        let entries = parsed["entries"]
            .as_array()
            .ok_or_else(|| RetainedStoreError::CorruptIndex("entries missing".into()))?
            .clone();
        Ok(entries)
    }

    /// Load an artifact by content reference, verifying the digest on read.
    pub fn load(
        &self,
        content_ref: &str,
        content_sha256: &str,
    ) -> Result<Value, RetainedStoreError> {
        let expected = format!("artifact://sha256/{content_sha256}");
        if content_ref != expected
            || content_sha256.len() != 64
            || !content_sha256.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(RetainedStoreError::InvalidReference);
        }
        let bytes = fs::read(self.blob_path(content_sha256))?;
        if sha256_hex(&bytes) != content_sha256 {
            return Err(RetainedStoreError::HashMismatch);
        }
        serde_json::from_slice(&bytes).map_err(|_| RetainedStoreError::InvalidArtifact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store() -> (ContentAddressedStore, tempfile::TempDir) {
        let directory = tempfile::tempdir().expect("tempdir");
        let root = directory.path().to_path_buf();
        let store = ContentAddressedStore::open(&root).expect("store opens");
        (store, directory)
    }

    #[test]
    fn content_addressed_writes_are_immutable_and_idempotent() {
        let (store, _directory) = store();
        let artifact = json!({"selected": "architecture-a"});
        let first = store
            .put("run_x", "t06", "architecture-decision/v1", &artifact)
            .expect("first put");
        let second = store
            .put("run_x", "t06", "architecture-decision/v1", &artifact)
            .expect("idempotent put");
        assert_eq!(first, second);
        assert_eq!(store.read_index("run_x").expect("index").len(), 1);
        // same content under a different task appends, never rewrites
        store
            .put("run_x", "t03", "architecture-proposal/v1", &artifact)
            .expect("second task");
        assert_eq!(store.read_index("run_x").expect("index").len(), 2);
    }

    #[test]
    fn reads_verify_hashes_and_reject_malformed_references() {
        let (store, _directory) = store();
        let artifact = json!({"beats": []});
        let reference = store
            .put("run_x", "t05", "architecture-proposal/v1", &artifact)
            .expect("put");
        let loaded = store
            .load(&reference.content_ref, &reference.content_sha256)
            .expect("load");
        assert_eq!(loaded, artifact);
        assert!(matches!(
            store.load("artifact://sha256/zz", &"0".repeat(64)),
            Err(RetainedStoreError::InvalidReference)
        ));
        assert!(matches!(
            store.load("artifact://sha256/other", &reference.content_sha256),
            Err(RetainedStoreError::InvalidReference)
        ));
    }

    #[test]
    fn tampered_blob_bytes_fail_the_digest_check() {
        let (store, directory) = store();
        let artifact = json!({"a": 1});
        let reference = store
            .put("run_x", "t06", "architecture-decision/v1", &artifact)
            .expect("put");
        let blob = directory
            .path()
            .join("blobs")
            .join(&reference.content_sha256[..2])
            .join(&reference.content_sha256);
        fs::write(&blob, b"{\"tampered\": true}").expect("tamper");
        assert!(matches!(
            store.load(&reference.content_ref, &reference.content_sha256),
            Err(RetainedStoreError::HashMismatch)
        ));
    }

    #[test]
    fn traversal_and_out_of_range_task_ids_are_rejected() {
        let (store, _directory) = store();
        let artifact = json!({});
        assert!(matches!(
            store.put("../escape", "t06", "s", &artifact),
            Err(RetainedStoreError::InvalidIdentity)
        ));
        assert!(matches!(
            store.put("run_x", "t18", "s", &artifact),
            Err(RetainedStoreError::InvalidIdentity)
        ));
        assert!(matches!(
            store.put("run_x", "t6", "s", &artifact),
            Err(RetainedStoreError::InvalidIdentity)
        ));
    }
}
