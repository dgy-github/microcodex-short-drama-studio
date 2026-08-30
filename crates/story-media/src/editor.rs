use crate::{
    compile_concat_plan, run_tool, MediaToolError, MediaToolManifest, MediaToolSpec, TimelineClip,
    ToolManifestError,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum TimelineExecutionError {
    #[error("media tool manifest failed")]
    Manifest(#[from] ToolManifestError),
    #[error("media timeline plan or process failed")]
    Tool(#[from] MediaToolError),
    #[error("media timeline output is invalid")]
    InvalidOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineExecutionReceipt {
    pub output: PathBuf,
    pub byte_len: u64,
}

pub async fn execute_timeline(
    manifest: &MediaToolManifest,
    tool_root: &Path,
    clips: &[TimelineClip],
    output: &Path,
    timeout: Duration,
) -> Result<TimelineExecutionReceipt, TimelineExecutionError> {
    validate_output_target(output)?;
    let ffmpeg = manifest.resolve_verified(tool_root, "ffmpeg")?;
    let mut plan = compile_concat_plan(&ffmpeg, clips, output)?;
    plan.args.splice(
        0..0,
        ["-hide_banner".into(), "-nostdin".into(), "-n".into()],
    );
    let result = run_tool(&MediaToolSpec {
        executable: ffmpeg,
        args: plan.args,
        timeout,
    })
    .await;
    if let Err(error) = result {
        remove_partial(output);
        return Err(error.into());
    }
    let metadata = fs::metadata(output).map_err(|_| TimelineExecutionError::InvalidOutput)?;
    if !metadata.is_file() || metadata.len() == 0 {
        remove_partial(output);
        return Err(TimelineExecutionError::InvalidOutput);
    }
    Ok(TimelineExecutionReceipt {
        output: output.to_path_buf(),
        byte_len: metadata.len(),
    })
}

fn validate_output_target(output: &Path) -> Result<(), TimelineExecutionError> {
    if !output.is_absolute()
        || output.exists()
        || output.extension().and_then(|v| v.to_str()) != Some("mp4")
    {
        return Err(TimelineExecutionError::InvalidOutput);
    }
    let parent = output
        .parent()
        .ok_or(TimelineExecutionError::InvalidOutput)?;
    if !parent.is_dir() {
        return Err(TimelineExecutionError::InvalidOutput);
    }
    Ok(())
}

fn remove_partial(output: &Path) {
    if output.is_file() {
        let _ = fs::remove_file(output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn manifest_for(path: &Path) -> MediaToolManifest {
        let bytes = fs::read(path).unwrap();
        let digest = format!("{:x}", Sha256::digest(bytes));
        MediaToolManifest::parse(&format!(r#"{{"schema":"media-tool-manifest/v1","tools":[{{"id":"ffmpeg","version":"fixture","relative_path":"{}","sha256":"{digest}"}}]}}"#,
            path.file_name().unwrap().to_string_lossy())).unwrap()
    }

    #[tokio::test]
    async fn failed_tool_leaves_no_partial_output() {
        let directory = tempfile::tempdir().unwrap();
        let executable = std::env::current_exe().unwrap();
        let copied = directory.path().join(executable.file_name().unwrap());
        fs::copy(executable, &copied).unwrap();
        let output = directory.path().join("result.mp4");
        let clip = TimelineClip {
            input: copied.to_string_lossy().into_owned(),
            start_seconds: 0.0,
            end_seconds: 1.0,
        };
        assert!(execute_timeline(
            &manifest_for(&copied),
            directory.path(),
            &[clip],
            &output,
            Duration::from_secs(2)
        )
        .await
        .is_err());
        assert!(!output.exists());
    }

    #[tokio::test]
    async fn existing_or_non_mp4_output_is_rejected_before_execution() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("existing.mp4");
        fs::write(&existing, b"keep").unwrap();
        let manifest = manifest_for(&std::env::current_exe().unwrap());
        assert!(execute_timeline(
            &manifest,
            directory.path(),
            &[],
            &existing,
            Duration::from_secs(1)
        )
        .await
        .is_err());
        assert_eq!(fs::read(existing).unwrap(), b"keep");
    }
}
