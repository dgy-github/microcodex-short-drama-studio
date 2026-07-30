use crate::run_protocol::{
    frame_end, sse_data, valid_acceptance, CommandAcceptance, IdempotencyKey, StartRunRequest,
};
use crate::{
    EventEnvelope, GenreContext, SidecarLifecycle, SidecarSignal, SidecarState,
    SidecarTransitionError,
};
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use story_core::StoryJob;
use story_provider::CapabilityToken;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::timeout;
use zeroize::Zeroize;

const CONTROL_PROTOCOL: &str = "story-sidecar-control/v1";
const TOKEN_ENV: &str = "MICROCODEX_SIDECAR_TOKEN";
const CAPABILITY_URL_ENV: &str = "MICROCODEX_CAPABILITY_URL";
const CAPABILITY_TOKEN_ENV: &str = "MICROCODEX_CAPABILITY_TOKEN";
const MAX_SSE_BUFFER_BYTES: usize = 1024 * 1024;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct SidecarAuthToken(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SidecarAuthTokenError {
    #[error("sidecar auth token must be at least 32 ASCII characters")]
    Invalid,
}

impl SidecarAuthToken {
    pub fn new(value: impl Into<String>) -> Result<Self, SidecarAuthTokenError> {
        let value = value.into();
        if value.len() < 32 || !value.is_ascii() {
            return Err(SidecarAuthTokenError::Invalid);
        }
        Ok(Self(value))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SidecarAuthToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SidecarAuthToken([REDACTED])")
    }
}

impl Drop for SidecarAuthToken {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarLaunchConfig {
    python_executable: PathBuf,
    working_directory: PathBuf,
    startup_timeout: Duration,
    module_mode: bool,
}

impl SidecarLaunchConfig {
    pub fn new(
        python_executable: impl Into<PathBuf>,
        working_directory: impl Into<PathBuf>,
        startup_timeout: Duration,
    ) -> Result<Self, SidecarProcessError> {
        let python_executable = python_executable.into();
        let working_directory = working_directory.into();
        if !python_executable.is_absolute()
            || !working_directory.is_absolute()
            || startup_timeout.is_zero()
        {
            return Err(SidecarProcessError::InvalidConfig);
        }
        Ok(Self {
            python_executable,
            working_directory,
            startup_timeout,
            module_mode: true,
        })
    }

    pub fn bundled(
        executable: impl Into<PathBuf>,
        data_directory: impl Into<PathBuf>,
        startup_timeout: Duration,
    ) -> Result<Self, SidecarProcessError> {
        let executable = executable.into();
        let data_directory = data_directory.into();
        if !executable.is_absolute() || !data_directory.is_absolute() || startup_timeout.is_zero() {
            return Err(SidecarProcessError::InvalidConfig);
        }
        Ok(Self {
            python_executable: executable,
            working_directory: data_directory,
            startup_timeout,
            module_mode: false,
        })
    }

    pub fn python_executable(&self) -> &Path {
        &self.python_executable
    }

    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    pub fn startup_timeout(&self) -> Duration {
        self.startup_timeout
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SidecarProcessError {
    #[error("sidecar launch configuration is invalid")]
    InvalidConfig,
    #[error("sidecar process could not be started")]
    Spawn,
    #[error("sidecar process did not provide a readiness channel")]
    MissingReadiness,
    #[error("sidecar readiness timed out")]
    ReadinessTimeout,
    #[error("sidecar readiness response is invalid")]
    InvalidReadiness,
    #[error("sidecar authenticated health check failed")]
    Health,
    #[error("sidecar command was rejected")]
    Command,
    #[error("sidecar idempotency key conflicts with an earlier command")]
    IdempotencyConflict,
    #[error("sidecar event stream is invalid")]
    EventStream,
    #[error("sidecar process operation failed")]
    Process,
    #[error(transparent)]
    Transition(#[from] SidecarTransitionError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadyMessage {
    protocol: String,
    status: String,
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct HealthMessage {
    protocol: String,
    status: String,
}

pub struct SidecarProcess {
    child: Child,
    base_url: String,
    token: SidecarAuthToken,
    client: reqwest::Client,
    lifecycle: SidecarLifecycle,
}

impl SidecarProcess {
    pub async fn launch(
        config: SidecarLaunchConfig,
        token: SidecarAuthToken,
    ) -> Result<Self, SidecarProcessError> {
        Self::launch_inner(config, token, None).await
    }

    pub async fn launch_with_capability(
        config: SidecarLaunchConfig,
        token: SidecarAuthToken,
        capability_endpoint: &str,
        capability_token: &CapabilityToken,
    ) -> Result<Self, SidecarProcessError> {
        if !capability_endpoint.starts_with("http://127.0.0.1:") {
            return Err(SidecarProcessError::InvalidConfig);
        }
        Self::launch_inner(config, token, Some((capability_endpoint, capability_token))).await
    }

    async fn launch_inner(
        config: SidecarLaunchConfig,
        token: SidecarAuthToken,
        capability: Option<(&str, &CapabilityToken)>,
    ) -> Result<Self, SidecarProcessError> {
        if !config.python_executable.is_file() || !config.working_directory.is_dir() {
            return Err(SidecarProcessError::InvalidConfig);
        }

        let mut lifecycle = SidecarLifecycle::default();
        lifecycle.transition(SidecarSignal::StartRequested)?;
        let mut command = Command::new(&config.python_executable);
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        if config.module_mode {
            command.arg("-m").arg("campaign_adapter.server");
        }
        command
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg("0")
            .arg("--event-log")
            .arg(config.working_directory.join("campaign_events.db"))
            .current_dir(&config.working_directory)
            .env(TOKEN_ENV, token.expose())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some((endpoint, capability_token)) = capability {
            command
                .env(CAPABILITY_URL_ENV, endpoint)
                .env(CAPABILITY_TOKEN_ENV, capability_token.expose());
        }

        let mut child = command.spawn().map_err(|_| SidecarProcessError::Spawn)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(SidecarProcessError::MissingReadiness)?;
        let mut line = String::new();
        let read = timeout(
            config.startup_timeout,
            BufReader::new(stdout).read_line(&mut line),
        )
        .await;
        let ready = match read {
            Err(_) => {
                terminate(&mut child).await;
                return Err(SidecarProcessError::ReadinessTimeout);
            }
            Ok(Err(_)) | Ok(Ok(0)) => {
                terminate(&mut child).await;
                return Err(SidecarProcessError::InvalidReadiness);
            }
            Ok(Ok(_)) => match serde_json::from_str::<ReadyMessage>(&line) {
                Ok(ready) if valid_ready(&ready) => ready,
                _ => {
                    terminate(&mut child).await;
                    return Err(SidecarProcessError::InvalidReadiness);
                }
            },
        };

        let base_url = format!("http://127.0.0.1:{}", ready.port);
        let client = reqwest::Client::builder()
            .timeout(config.startup_timeout)
            .build()
            .map_err(|_| SidecarProcessError::Health)?;
        if !health_is_ready(&client, &base_url, &token).await {
            terminate(&mut child).await;
            return Err(SidecarProcessError::Health);
        }
        lifecycle.transition(SidecarSignal::HealthReady)?;

        Ok(Self {
            child,
            base_url,
            token,
            client,
            lifecycle,
        })
    }

    pub fn state(&self) -> SidecarState {
        self.lifecycle.state()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn process_id(&self) -> Option<u32> {
        self.child.id()
    }

    pub async fn health(&self) -> Result<(), SidecarProcessError> {
        if !self.lifecycle.can_accept_commands()
            || !health_is_ready(&self.client, &self.base_url, &self.token).await
        {
            return Err(SidecarProcessError::Health);
        }
        Ok(())
    }

    pub async fn start_run(
        &self,
        job: &StoryJob,
        idempotency_key: &IdempotencyKey,
    ) -> Result<CommandAcceptance, SidecarProcessError> {
        self.send_start_run(StartRunRequest::new(job), job, idempotency_key)
            .await
    }

    pub async fn start_run_with_genre_context(
        &self,
        job: &StoryJob,
        idempotency_key: &IdempotencyKey,
        genre_context: &GenreContext,
    ) -> Result<CommandAcceptance, SidecarProcessError> {
        self.send_start_run(
            StartRunRequest::with_genre_context(job, genre_context),
            job,
            idempotency_key,
        )
        .await
    }

    async fn send_start_run(
        &self,
        request: StartRunRequest<'_>,
        job: &StoryJob,
        idempotency_key: &IdempotencyKey,
    ) -> Result<CommandAcceptance, SidecarProcessError> {
        if !self.lifecycle.can_accept_commands() || job.validate().is_err() {
            return Err(SidecarProcessError::Command);
        }
        let response = self
            .client
            .post(format!("{}/v1/runs", self.base_url))
            .bearer_auth(self.token.expose())
            .header("Idempotency-Key", idempotency_key.as_str())
            .json(&request)
            .send()
            .await
            .map_err(|_| SidecarProcessError::Command)?;
        if response.status() == reqwest::StatusCode::CONFLICT {
            return Err(SidecarProcessError::IdempotencyConflict);
        }
        if response.status() != reqwest::StatusCode::ACCEPTED {
            return Err(SidecarProcessError::Command);
        }
        let acceptance = response
            .json::<CommandAcceptance>()
            .await
            .map_err(|_| SidecarProcessError::Command)?;
        if !valid_acceptance(&acceptance, &job.job_id) {
            return Err(SidecarProcessError::Command);
        }
        Ok(acceptance)
    }

    pub async fn replay_events(
        &self,
        acceptance: &CommandAcceptance,
        last_event_id: Option<u64>,
    ) -> Result<Vec<EventEnvelope>, SidecarProcessError> {
        if !self.lifecycle.can_accept_commands()
            || !valid_acceptance(acceptance, &acceptance.job_id)
        {
            return Err(SidecarProcessError::EventStream);
        }
        let mut request = self
            .client
            .get(format!("{}{}", self.base_url, acceptance.event_stream_url))
            .bearer_auth(self.token.expose());
        if let Some(sequence) = last_event_id {
            request = request.header("Last-Event-ID", sequence.to_string());
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| SidecarProcessError::EventStream)?;
        if !response.status().is_success() {
            return Err(SidecarProcessError::EventStream);
        }

        let mut buffer = Vec::new();
        let mut events = Vec::new();
        loop {
            let chunk = response
                .chunk()
                .await
                .map_err(|_| SidecarProcessError::EventStream)?
                .ok_or(SidecarProcessError::EventStream)?;
            buffer.extend_from_slice(&chunk);
            if buffer.len() > MAX_SSE_BUFFER_BYTES {
                return Err(SidecarProcessError::EventStream);
            }
            while let Some(end) = frame_end(&buffer) {
                let frame = buffer.drain(..end + 2).collect::<Vec<_>>();
                if frame.starts_with(b": replay-complete") {
                    return Ok(events);
                }
                if let Some(data) = sse_data(&frame) {
                    let event: EventEnvelope = serde_json::from_slice(data)
                        .map_err(|_| SidecarProcessError::EventStream)?;
                    let after_cursor = last_event_id.is_none_or(|last| event.seq > last);
                    if event.validate().is_err()
                        || event.run_id != acceptance.run_id
                        || !after_cursor
                    {
                        return Err(SidecarProcessError::EventStream);
                    }
                    events.push(event);
                }
            }
        }
    }

    pub async fn workflow_result(
        &self,
        acceptance: &CommandAcceptance,
    ) -> Result<serde_json::Value, SidecarProcessError> {
        if !self.lifecycle.can_accept_commands()
            || !valid_acceptance(acceptance, &acceptance.job_id)
        {
            return Err(SidecarProcessError::Command);
        }
        let response = self
            .client
            .get(format!(
                "{}/v1/runs/{}/result",
                self.base_url, acceptance.run_id
            ))
            .bearer_auth(self.token.expose())
            .send()
            .await
            .map_err(|_| SidecarProcessError::Command)?;
        if !response.status().is_success() {
            return Err(SidecarProcessError::Command);
        }
        response
            .json()
            .await
            .map_err(|_| SidecarProcessError::Command)
    }

    pub async fn cancel_run(
        &self,
        acceptance: &CommandAcceptance,
    ) -> Result<EventEnvelope, SidecarProcessError> {
        if !self.lifecycle.can_accept_commands()
            || !valid_acceptance(acceptance, &acceptance.job_id)
        {
            return Err(SidecarProcessError::Command);
        }
        let response = self
            .client
            .post(format!(
                "{}/v1/runs/{}/cancel",
                self.base_url, acceptance.run_id
            ))
            .bearer_auth(self.token.expose())
            .send()
            .await
            .map_err(|_| SidecarProcessError::Command)?;
        if !matches!(
            response.status(),
            reqwest::StatusCode::OK | reqwest::StatusCode::ACCEPTED
        ) {
            return Err(SidecarProcessError::Command);
        }
        let event = response
            .json::<EventEnvelope>()
            .await
            .map_err(|_| SidecarProcessError::Command)?;
        if event.validate().is_err()
            || event.run_id != acceptance.run_id
            || !matches!(
                event.event_type.as_str(),
                "run.completed" | "run.failed" | "run.cancelled"
            )
        {
            return Err(SidecarProcessError::Command);
        }
        Ok(event)
    }

    pub fn refresh_state(&mut self) -> Result<SidecarState, SidecarProcessError> {
        if self
            .child
            .try_wait()
            .map_err(|_| SidecarProcessError::Process)?
            .is_some()
            && matches!(
                self.lifecycle.state(),
                SidecarState::Starting | SidecarState::Ready
            )
        {
            self.lifecycle.transition(SidecarSignal::ProcessExited)?;
        }
        Ok(self.lifecycle.state())
    }

    pub async fn stop(mut self) -> Result<(), SidecarProcessError> {
        self.lifecycle.transition(SidecarSignal::StopRequested)?;
        if self
            .child
            .try_wait()
            .map_err(|_| SidecarProcessError::Process)?
            .is_none()
        {
            self.child
                .kill()
                .await
                .map_err(|_| SidecarProcessError::Process)?;
        }
        self.lifecycle.transition(SidecarSignal::ProcessExited)?;
        Ok(())
    }
}

impl Drop for SidecarProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.start_kill();
        }
    }
}

fn valid_ready(ready: &ReadyMessage) -> bool {
    ready.protocol == CONTROL_PROTOCOL
        && ready.status == "ready"
        && ready.host == "127.0.0.1"
        && ready.port > 0
}

async fn health_is_ready(
    client: &reqwest::Client,
    base_url: &str,
    token: &SidecarAuthToken,
) -> bool {
    let response = match client
        .get(format!("{base_url}/health"))
        .bearer_auth(token.expose())
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response,
        _ => return false,
    };
    match response.json::<HealthMessage>().await {
        Ok(health) => health.protocol == CONTROL_PROTOCOL && health.status == "ready",
        Err(_) => false,
    }
}

async fn terminate(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_redacted_and_validated() {
        assert_eq!(
            SidecarAuthToken::new("short").unwrap_err(),
            SidecarAuthTokenError::Invalid
        );
        let token = SidecarAuthToken::new("a-secure-test-token-with-32-characters").unwrap();
        assert_eq!(format!("{token:?}"), "SidecarAuthToken([REDACTED])");
    }

    #[test]
    fn launch_paths_must_be_absolute() {
        assert!(matches!(
            SidecarLaunchConfig::new("python", "sidecar", Duration::from_secs(5)),
            Err(SidecarProcessError::InvalidConfig)
        ));
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let bundled = SidecarLaunchConfig::bundled(
            root.join("story-sidecar.exe"),
            root.join("target/sidecar-data"),
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(!bundled.module_mode);
    }

    #[test]
    fn readiness_is_bound_to_the_control_protocol_and_loopback() {
        let ready = ReadyMessage {
            protocol: CONTROL_PROTOCOL.into(),
            status: "ready".into(),
            host: "127.0.0.1".into(),
            port: 1234,
        };
        assert!(valid_ready(&ready));

        let external = ReadyMessage {
            host: "0.0.0.0".into(),
            ..ready
        };
        assert!(!valid_ready(&external));
    }
}
