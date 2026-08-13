use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const STORE_SCHEMA: &str = "story-store-version/v1";
const BACKUP_SCHEMA: &str = "story-store-backup/v1";
const CURRENT_VERSION: u16 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StoreOperationError {
    #[error("storage path is invalid")]
    InvalidPath,
    #[error("storage input/output failed")]
    Io,
    #[error("storage metadata is invalid")]
    InvalidMetadata,
    #[error("backup content hash mismatch")]
    Integrity,
    #[error("storage schema version is unsupported")]
    UnsupportedVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema: String,
    pub store_version: u16,
    pub files: Vec<BackupFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairReport {
    pub removed_partial_files: Vec<String>,
    pub removed_partial_dirs: Vec<String>,
    pub verified_files: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoreVersion {
    schema: String,
    version: u16,
}

pub fn migrate_store(root: &Path) -> Result<u16, StoreOperationError> {
    require_absolute(root)?;
    std::fs::create_dir_all(root).map_err(|_| StoreOperationError::Io)?;
    let version_path = root.join(".story-store-version.json");
    if version_path.exists() {
        let version: StoreVersion = read_json(&version_path)?;
        if version.schema != STORE_SCHEMA || version.version > CURRENT_VERSION {
            return Err(StoreOperationError::UnsupportedVersion);
        }
        if version.version == CURRENT_VERSION {
            return Ok(CURRENT_VERSION);
        }
    }
    let temporary = root.join(".story-store-version.partial.json");
    write_json(
        &temporary,
        &StoreVersion {
            schema: STORE_SCHEMA.into(),
            version: CURRENT_VERSION,
        },
    )?;
    replace_file(&temporary, &version_path)?;
    Ok(CURRENT_VERSION)
}

pub fn repair_store(root: &Path) -> Result<RepairReport, StoreOperationError> {
    require_absolute(root)?;
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    let mut removed = Vec::new();
    let mut verified = 0;
    for (relative, path) in files {
        if is_partial(&relative) {
            std::fs::remove_file(&path).map_err(|_| StoreOperationError::Io)?;
            removed.push(relative);
        } else {
            let _ = hash_file(&path)?;
            verified += 1;
        }
    }
    let mut partial_dirs = Vec::new();
    collect_partial_dirs(root, root, &mut partial_dirs)?;
    let mut removed_dirs = Vec::new();
    for (relative, path) in partial_dirs {
        std::fs::remove_dir_all(&path).map_err(|_| StoreOperationError::Io)?;
        removed_dirs.push(relative);
    }
    removed.sort();
    removed_dirs.sort();
    Ok(RepairReport {
        removed_partial_files: removed,
        removed_partial_dirs: removed_dirs,
        verified_files: verified,
    })
}

pub fn create_backup(
    root: &Path,
    destination: &Path,
) -> Result<BackupManifest, StoreOperationError> {
    require_absolute(root)?;
    require_absolute(destination)?;
    if !root.is_dir() || destination.exists() {
        return Err(StoreOperationError::InvalidPath);
    }
    let parent = destination
        .parent()
        .ok_or(StoreOperationError::InvalidPath)?;
    std::fs::create_dir_all(parent).map_err(|_| StoreOperationError::Io)?;
    let temporary = parent.join(format!(
        ".{}.partial",
        destination
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(StoreOperationError::InvalidPath)?
    ));
    if temporary.exists() {
        return Err(StoreOperationError::InvalidPath);
    }
    std::fs::create_dir(&temporary).map_err(|_| StoreOperationError::Io)?;

    let mut source_files = Vec::new();
    collect_files(root, root, &mut source_files)?;
    let mut files = Vec::new();
    for (relative, source) in source_files {
        if is_partial(&relative) {
            continue;
        }
        let target = join_portable(&temporary, &relative)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|_| StoreOperationError::Io)?;
        }
        std::fs::copy(&source, &target).map_err(|_| StoreOperationError::Io)?;
        let (sha256, bytes) = hash_file(&target)?;
        files.push(BackupFile {
            path: relative,
            sha256,
            bytes,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = BackupManifest {
        schema: BACKUP_SCHEMA.into(),
        store_version: CURRENT_VERSION,
        files,
    };
    write_json(&temporary.join("backup-manifest.json"), &manifest)?;
    std::fs::rename(&temporary, destination).map_err(|_| StoreOperationError::Io)?;
    Ok(manifest)
}

pub fn restore_backup(
    backup: &Path,
    destination: &Path,
) -> Result<BackupManifest, StoreOperationError> {
    require_absolute(backup)?;
    require_absolute(destination)?;
    if !backup.is_dir() || destination.exists() {
        return Err(StoreOperationError::InvalidPath);
    }
    let manifest: BackupManifest = read_json(&backup.join("backup-manifest.json"))?;
    if manifest.schema != BACKUP_SCHEMA || manifest.store_version != CURRENT_VERSION {
        return Err(StoreOperationError::InvalidMetadata);
    }
    for file in &manifest.files {
        let source = join_portable(backup, &file.path)?;
        let (sha256, bytes) = hash_file(&source)?;
        if sha256 != file.sha256 || bytes != file.bytes {
            return Err(StoreOperationError::Integrity);
        }
    }

    let parent = destination
        .parent()
        .ok_or(StoreOperationError::InvalidPath)?;
    std::fs::create_dir_all(parent).map_err(|_| StoreOperationError::Io)?;
    let temporary = parent.join(format!(
        ".{}.partial",
        destination
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(StoreOperationError::InvalidPath)?
    ));
    if temporary.exists() {
        return Err(StoreOperationError::InvalidPath);
    }
    std::fs::create_dir(&temporary).map_err(|_| StoreOperationError::Io)?;
    for file in &manifest.files {
        let source = join_portable(backup, &file.path)?;
        let target = join_portable(&temporary, &file.path)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|_| StoreOperationError::Io)?;
        }
        std::fs::copy(source, target).map_err(|_| StoreOperationError::Io)?;
    }
    std::fs::rename(&temporary, destination).map_err(|_| StoreOperationError::Io)?;
    Ok(manifest)
}

fn require_absolute(path: &Path) -> Result<(), StoreOperationError> {
    if !path.is_absolute() {
        return Err(StoreOperationError::InvalidPath);
    }
    Ok(())
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), StoreOperationError> {
    for entry in std::fs::read_dir(directory).map_err(|_| StoreOperationError::Io)? {
        let entry = entry.map_err(|_| StoreOperationError::Io)?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|_| StoreOperationError::Io)?;
        if kind.is_symlink() {
            return Err(StoreOperationError::InvalidPath);
        }
        if kind.is_dir() {
            collect_files(root, &path, files)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| StoreOperationError::InvalidPath)?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            join_portable(root, &relative)?;
            files.push((relative, path));
        }
    }
    Ok(())
}

fn collect_partial_dirs(
    root: &Path,
    directory: &Path,
    dirs: &mut Vec<(String, PathBuf)>,
) -> Result<(), StoreOperationError> {
    for entry in std::fs::read_dir(directory).map_err(|_| StoreOperationError::Io)? {
        let entry = entry.map_err(|_| StoreOperationError::Io)?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|_| StoreOperationError::Io)?;
        if kind.is_symlink() {
            return Err(StoreOperationError::InvalidPath);
        }
        if kind.is_dir() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| StoreOperationError::InvalidPath)?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            if is_partial(&relative) {
                // Remove the whole partial directory; do not descend into it.
                dirs.push((relative, path));
            } else {
                collect_partial_dirs(root, &path, dirs)?;
            }
        }
    }
    Ok(())
}

fn join_portable(root: &Path, relative: &str) -> Result<PathBuf, StoreOperationError> {
    if relative.is_empty()
        || relative.contains('\\')
        || relative.contains(':')
        || relative
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(StoreOperationError::InvalidPath);
    }
    Ok(relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part)))
}

fn is_partial(relative: &str) -> bool {
    relative
        .split('/')
        .next_back()
        .is_some_and(|name| name.contains(".partial"))
}

fn hash_file(path: &Path) -> Result<(String, u64), StoreOperationError> {
    let bytes = std::fs::read(path).map_err(|_| StoreOperationError::Io)?;
    let digest = Sha256::digest(&bytes);
    Ok((format!("{digest:x}"), bytes.len() as u64))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, StoreOperationError> {
    let bytes = std::fs::read(path).map_err(|_| StoreOperationError::Io)?;
    serde_json::from_slice(&bytes).map_err(|_| StoreOperationError::InvalidMetadata)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), StoreOperationError> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|_| StoreOperationError::InvalidMetadata)?;
    std::fs::write(path, bytes).map_err(|_| StoreOperationError::Io)
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), StoreOperationError> {
    if destination.exists() {
        std::fs::remove_file(destination).map_err(|_| StoreOperationError::Io)?;
    }
    std::fs::rename(source, destination).map_err(|_| StoreOperationError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_repair_backup_and_restore_preserve_durable_state() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("store");
        std::fs::create_dir_all(root.join("runs/run_1")).unwrap();
        std::fs::create_dir_all(root.join("revisions/rev_1")).unwrap();
        std::fs::write(root.join("runs/run_1/story.json"), b"story").unwrap();
        std::fs::write(root.join("revisions/rev_1/approval.json"), b"approved").unwrap();
        std::fs::write(root.join("runs/run_1/story.partial.json"), b"incomplete").unwrap();
        // A leftover partial directory from an interrupted backup/restore.
        std::fs::create_dir_all(root.join("runs/run_1/.backup.partial")).unwrap();
        std::fs::write(
            root.join("runs/run_1/.backup.partial/story.json"),
            b"partial",
        )
        .unwrap();

        assert_eq!(migrate_store(&root).unwrap(), 1);
        assert_eq!(migrate_store(&root).unwrap(), 1);
        let repair = repair_store(&root).unwrap();
        assert_eq!(
            repair.removed_partial_files,
            vec!["runs/run_1/story.partial.json"]
        );
        assert_eq!(
            repair.removed_partial_dirs,
            vec!["runs/run_1/.backup.partial"]
        );
        assert!(!root.join("runs/run_1/.backup.partial").exists());

        let backup = directory.path().join("backup");
        let manifest = create_backup(&root, &backup).unwrap();
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == "revisions/rev_1/approval.json"));
        let restored = directory.path().join("restored");
        restore_backup(&backup, &restored).unwrap();
        assert_eq!(
            std::fs::read(restored.join("runs/run_1/story.json")).unwrap(),
            b"story"
        );
        assert_eq!(
            std::fs::read(restored.join("revisions/rev_1/approval.json")).unwrap(),
            b"approved"
        );
    }

    #[test]
    fn corrupted_backup_fails_before_creating_restore_target() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("store");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("story.json"), b"story").unwrap();
        migrate_store(&root).unwrap();
        let backup = directory.path().join("backup");
        create_backup(&root, &backup).unwrap();
        std::fs::write(backup.join("story.json"), b"corrupt").unwrap();
        let restored = directory.path().join("restored");

        assert!(matches!(
            restore_backup(&backup, &restored),
            Err(StoreOperationError::Integrity)
        ));
        assert!(!restored.exists());
    }
}
