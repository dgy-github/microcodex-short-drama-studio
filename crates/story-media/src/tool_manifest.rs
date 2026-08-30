use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ToolManifestError {
    #[error("media tool manifest is invalid")]
    Invalid,
    #[error("media tool binary is missing")]
    Missing,
    #[error("media tool binary hash does not match")]
    HashMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MediaToolDiagnostic {
    pub id: String,
    pub version: String,
    pub status: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaToolManifest {
    pub schema: String,
    pub tools: Vec<MediaToolEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaToolEntry {
    pub id: String,
    pub version: String,
    pub relative_path: String,
    pub sha256: String,
}

impl MediaToolManifest {
    pub fn parse(input: &str) -> Result<Self, ToolManifestError> {
        let manifest: Self = serde_json::from_str(input).map_err(|_| ToolManifestError::Invalid)?;
        if manifest.schema != "media-tool-manifest/v1" || manifest.tools.is_empty() {
            return Err(ToolManifestError::Invalid);
        }
        for (index, tool) in manifest.tools.iter().enumerate() {
            if !valid_id(&tool.id)
                || tool.version.trim().is_empty()
                || !valid_digest(&tool.sha256)
                || unsafe_relative_path(&tool.relative_path)
                || manifest.tools[..index]
                    .iter()
                    .any(|prior| prior.id == tool.id)
            {
                return Err(ToolManifestError::Invalid);
            }
        }
        Ok(manifest)
    }

    pub fn resolve_verified(&self, root: &Path, id: &str) -> Result<PathBuf, ToolManifestError> {
        if !root.is_absolute() {
            return Err(ToolManifestError::Invalid);
        }
        let entry = self
            .tools
            .iter()
            .find(|tool| tool.id == id)
            .ok_or(ToolManifestError::Missing)?;
        let path = root.join(&entry.relative_path);
        let bytes = fs::read(&path).map_err(|_| ToolManifestError::Missing)?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if actual != entry.sha256 {
            return Err(ToolManifestError::HashMismatch);
        }
        Ok(path)
    }

    pub fn diagnose(&self, root: &Path) -> Vec<MediaToolDiagnostic> {
        self.tools
            .iter()
            .map(|tool| {
                let status = if !root.is_absolute() {
                    "invalid_root"
                } else {
                    match self.resolve_verified(root, &tool.id) {
                        Ok(_) => "ready",
                        Err(ToolManifestError::Missing) => "missing",
                        Err(ToolManifestError::HashMismatch) => "hash_mismatch",
                        Err(ToolManifestError::Invalid) => "invalid_root",
                    }
                };
                MediaToolDiagnostic {
                    id: tool.id.clone(),
                    version: tool.version.clone(),
                    status,
                }
            })
            .collect()
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unsafe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    value.is_empty()
        || path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_only_hash_verified_binaries_inside_the_tool_root() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("ffmpeg.exe");
        fs::write(&binary, b"fixture-ffmpeg").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"fixture-ffmpeg"));
        let manifest = MediaToolManifest::parse(&format!(r#"{{"schema":"media-tool-manifest/v1","tools":[{{"id":"ffmpeg","version":"7.1.1","relative_path":"ffmpeg.exe","sha256":"{digest}"}}]}}"#)).unwrap();
        assert_eq!(
            manifest
                .resolve_verified(directory.path(), "ffmpeg")
                .unwrap(),
            binary
        );
        fs::write(&binary, b"tampered").unwrap();
        assert_eq!(
            manifest.resolve_verified(directory.path(), "ffmpeg"),
            Err(ToolManifestError::HashMismatch)
        );
    }

    #[test]
    fn rejects_traversal_duplicates_and_invalid_hashes() {
        for body in [
            r#"{"schema":"media-tool-manifest/v1","tools":[{"id":"ffmpeg","version":"7","relative_path":"../ffmpeg.exe","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"#,
            r#"{"schema":"media-tool-manifest/v1","tools":[{"id":"ffmpeg","version":"7","relative_path":"ffmpeg.exe","sha256":"bad"}]}"#,
        ] {
            assert!(MediaToolManifest::parse(body).is_err());
        }
    }
}
