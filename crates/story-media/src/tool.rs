use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;

const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MediaToolError {
    #[error("media tool path must be absolute")]
    NonAbsolutePath,
    #[error("media tool path or timeout is invalid")]
    InvalidPath,
    #[error("media tool arguments contain an invalid value")]
    InvalidArgument,
    #[error("media tool timed out")]
    Timeout,
    #[error("media tool exited unsuccessfully")]
    Failed,
    #[error("media tool output exceeded the bound")]
    OutputTooLarge,
    #[error("media tool could not be started")]
    Start,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaToolSpec {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub timeout: Duration,
}

impl MediaToolSpec {
    pub fn validate(&self) -> Result<(), MediaToolError> {
        if !self.executable.is_absolute() {
            return Err(MediaToolError::NonAbsolutePath);
        }
        if self.executable.as_os_str().is_empty() || self.timeout.is_zero() {
            return Err(MediaToolError::InvalidPath);
        }
        if self.args.iter().any(|arg| arg.contains('\0')) {
            return Err(MediaToolError::InvalidArgument);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaToolOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Run a pinned media binary directly, without invoking a command shell.
pub async fn run_tool(spec: &MediaToolSpec) -> Result<MediaToolOutput, MediaToolError> {
    spec.validate()?;
    let child = Command::new(&spec.executable)
        .args(&spec.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| MediaToolError::Start)?;
    let output = match tokio::time::timeout(spec.timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(_)) => return Err(MediaToolError::Failed),
        Err(_) => return Err(MediaToolError::Timeout),
    };
    if output.stdout.len() > MAX_OUTPUT_BYTES || output.stderr.len() > MAX_OUTPUT_BYTES {
        return Err(MediaToolError::OutputTooLarge);
    }
    if !output.status.success() {
        return Err(MediaToolError::Failed);
    }
    Ok(MediaToolOutput {
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

pub fn validate_tool_path(path: &Path) -> Result<(), MediaToolError> {
    if !path.is_absolute() {
        return Err(MediaToolError::NonAbsolutePath);
    }
    if path.as_os_str().is_empty() {
        return Err(MediaToolError::InvalidPath);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_and_embedded_nul_arguments() {
        let spec = MediaToolSpec {
            executable: PathBuf::from("ffmpeg"),
            args: vec![],
            timeout: Duration::from_secs(1),
        };
        assert_eq!(spec.validate(), Err(MediaToolError::NonAbsolutePath));
        let valid = MediaToolSpec {
            executable: std::env::current_exe().unwrap(),
            args: vec!["bad\0arg".into()],
            timeout: Duration::from_secs(1),
        };
        assert_eq!(valid.validate(), Err(MediaToolError::InvalidArgument));
    }

    #[tokio::test]
    async fn executes_a_real_child_without_a_shell() {
        let spec = MediaToolSpec {
            executable: std::env::current_exe().unwrap(),
            args: vec!["--help".into()],
            timeout: Duration::from_secs(5),
        };
        run_tool(&spec).await.unwrap();
    }

    #[test]
    #[ignore]
    fn slow_fixture_child() {
        std::thread::sleep(Duration::from_secs(30));
    }

    #[tokio::test]
    async fn timeout_kills_and_reaps_the_child() {
        let spec = MediaToolSpec {
            executable: std::env::current_exe().unwrap(),
            args: vec![
                "--exact".into(),
                "tool::tests::slow_fixture_child".into(),
                "--ignored".into(),
            ],
            timeout: Duration::from_millis(20),
        };
        assert_eq!(run_tool(&spec).await, Err(MediaToolError::Timeout));
    }
}
