use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use story_core::StoryJob;
use story_provider::{CapabilityHost, CapabilityHostConfig, CapabilityToken, PricingCatalog};
use story_runtime::{
    CommandAcceptance, EventEnvelope, GenrePackRegistry, IdempotencyKey, SidecarAuthToken,
    SidecarLaunchConfig, SidecarProcess,
};
use tokio::sync::Mutex;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::artifacts::ArtifactRepository;
use crate::credentials::CredentialService;
use crate::provider_settings::ProviderSettingsService;
use crate::CommandError;

const TASKS_TOTAL: u16 = 17;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BudgetSnapshot {
    pub max_tokens: u64,
    pub consumed_tokens: u64,
    pub max_cny_fen: u64,
    pub consumed_cny_fen: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunSnapshot {
    pub schema: &'static str,
    pub run_id: String,
    pub job_id: String,
    pub status: String,
    pub last_event_id: u64,
    pub tasks_total: u16,
    pub tasks_queued: usize,
    pub tasks_started: usize,
    pub tasks_completed: usize,
    pub reviews_completed: usize,
    pub approvals_pending: usize,
    pub error: Option<String>,
    pub budget: BudgetSnapshot,
    pub events: Vec<EventEnvelope>,
}

struct RuntimeSession {
    _capability_host: Option<CapabilityHost>,
    sidecar: SidecarProcess,
    acceptance: CommandAcceptance,
    projection: ProjectionState,
    artifact_stored: bool,
}

struct ProjectionState {
    snapshot: RunSnapshot,
    queued: HashSet<String>,
    started: HashSet<String>,
    completed: HashSet<String>,
    reviews: HashSet<String>,
    pending_approvals: HashSet<String>,
}

pub struct DesktopRunController {
    repository_root: PathBuf,
    start_gate: Mutex<()>,
    session: Mutex<Option<RuntimeSession>>,
}

impl DesktopRunController {
    pub fn new(repository_root: PathBuf) -> Self {
        Self {
            repository_root,
            start_gate: Mutex::new(()),
            session: Mutex::new(None),
        }
    }

    pub async fn start(
        &self,
        credentials: &CredentialService,
        provider_settings: &ProviderSettingsService,
        artifacts: &ArtifactRepository,
        job: StoryJob,
    ) -> Result<RunSnapshot, CommandError> {
        let _start_guard = self.start_gate.lock().await;
        job.validate()
            .map_err(|_| CommandError::invalid_story_job())?;
        let genre_context = GenrePackRegistry::load(&self.repository_root)
            .and_then(|registry| registry.resolve_job(&job))
            .map_err(|_| CommandError::invalid_genre_pack())?;
        if let Some(snapshot) = self.refresh_and_find_active(artifacts).await? {
            return Ok(snapshot);
        }
        let generation = provider_settings.route(credentials, "deepseek")?;
        let review = provider_settings.route(credentials, "aliyun_bailian")?;
        let pricing = load_pricing_catalog(&self.repository_root)?;

        let mut capability_material = format!(
            "desktop-capability-{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let capability_host = CapabilityHost::start(CapabilityHostConfig {
            generation,
            review,
            pricing,
            package_schema_path: self.repository_root.join("schemas/story-package-v1.json"),
            retained_store_root: artifacts.retained_store_root(),
            media_project_store_root: artifacts.media_project_store_root(),
            token: CapabilityToken::new(capability_material.clone())
                .map_err(|_| CommandError::runtime_unavailable())?,
            request_timeout: Duration::from_secs(job.budget.deadline_seconds),
        })
        .await
        .map_err(|_| CommandError::runtime_unavailable())?;
        let capability_token = CapabilityToken::new(capability_material.clone())
            .map_err(|_| CommandError::runtime_unavailable())?;
        capability_material.zeroize();

        self.start_with_capability(
            job,
            genre_context,
            capability_host.endpoint(),
            capability_token,
            Some(capability_host),
        )
        .await
    }

    async fn refresh_and_find_active(
        &self,
        artifacts: &ArtifactRepository,
    ) -> Result<Option<RunSnapshot>, CommandError> {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return Ok(None);
        };
        if is_active(&session.projection.snapshot) {
            refresh_session(session, artifacts).await?;
        }
        Ok(is_active(&session.projection.snapshot).then(|| session.projection.snapshot.clone()))
    }

    async fn start_with_capability(
        &self,
        job: StoryJob,
        genre_context: Option<story_runtime::GenreContext>,
        capability_base_url: String,
        capability_token: CapabilityToken,
        capability_host: Option<CapabilityHost>,
    ) -> Result<RunSnapshot, CommandError> {
        let mut guard = self.session.lock().await;
        if guard.as_ref().is_some_and(|session| {
            matches!(
                session.projection.snapshot.status.as_str(),
                "accepted" | "running"
            )
        }) {
            return Err(CommandError::run_active());
        }
        let mut sidecar_material = format!(
            "desktop-sidecar-{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let sidecar_token = SidecarAuthToken::new(sidecar_material.clone())
            .map_err(|_| CommandError::runtime_unavailable())?;
        sidecar_material.zeroize();
        let sidecar_config = sidecar_launch_config(&self.repository_root)
            .map_err(|_| CommandError::runtime_unavailable())?;
        let sidecar = SidecarProcess::launch_with_capability(
            sidecar_config,
            sidecar_token,
            &capability_base_url,
            &capability_token,
        )
        .await
        .map_err(|_| CommandError::runtime_unavailable())?;

        let key = IdempotencyKey::new(format!("desktop-start-{}", Uuid::new_v4().simple()))
            .map_err(|_| CommandError::runtime_unavailable())?;
        let acceptance = match &genre_context {
            Some(context) => {
                sidecar
                    .start_run_with_genre_context(&job, &key, context)
                    .await
            }
            None => sidecar.start_run(&job, &key).await,
        }
        .map_err(|_| CommandError::run_start_failed())?;
        let snapshot = RunSnapshot {
            schema: "desktop-run-snapshot/v1",
            run_id: acceptance.run_id.clone(),
            job_id: acceptance.job_id.clone(),
            status: "accepted".into(),
            last_event_id: acceptance.accepted_event_seq,
            tasks_total: TASKS_TOTAL,
            tasks_queued: 0,
            tasks_started: 0,
            tasks_completed: 0,
            reviews_completed: 0,
            approvals_pending: 0,
            error: None,
            budget: BudgetSnapshot {
                max_tokens: job.budget.max_tokens,
                consumed_tokens: 0,
                max_cny_fen: job.budget.max_cny_fen,
                consumed_cny_fen: None,
            },
            events: Vec::new(),
        };
        *guard = Some(RuntimeSession {
            _capability_host: capability_host,
            sidecar,
            acceptance,
            projection: ProjectionState::new(snapshot.clone()),
            artifact_stored: false,
        });
        Ok(snapshot)
    }

    #[cfg(test)]
    async fn start_with_test_capability(
        &self,
        artifacts: &ArtifactRepository,
        job: StoryJob,
        capability_base_url: String,
        capability_token: CapabilityToken,
    ) -> Result<RunSnapshot, CommandError> {
        let _start_guard = self.start_gate.lock().await;
        job.validate()
            .map_err(|_| CommandError::invalid_story_job())?;
        let genre_context = GenrePackRegistry::load(&self.repository_root)
            .and_then(|registry| registry.resolve_job(&job))
            .map_err(|_| CommandError::invalid_genre_pack())?;
        if let Some(snapshot) = self.refresh_and_find_active(artifacts).await? {
            return Ok(snapshot);
        }
        self.start_with_capability(
            job,
            genre_context,
            capability_base_url,
            capability_token,
            None,
        )
        .await
    }

    pub async fn sync(&self, artifacts: &ArtifactRepository) -> Result<RunSnapshot, CommandError> {
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(CommandError::run_missing)?;
        refresh_session(session, artifacts).await?;
        Ok(session.projection.snapshot.clone())
    }

    pub async fn cancel(&self) -> Result<RunSnapshot, CommandError> {
        let mut guard = self.session.lock().await;
        let session = guard.as_mut().ok_or_else(CommandError::run_missing)?;
        let event = session
            .sidecar
            .cancel_run(&session.acceptance)
            .await
            .map_err(|_| CommandError::run_cancel_failed())?;
        session.projection.apply(&event);
        session.projection.snapshot.events = vec![event];
        Ok(session.projection.snapshot.clone())
    }
}

fn load_pricing_catalog(repository_root: &Path) -> Result<PricingCatalog, CommandError> {
    let path = repository_root.join("config/provider-pricing-v1.json");
    let input = std::fs::read_to_string(path).map_err(|_| CommandError::runtime_unavailable())?;
    PricingCatalog::from_json(&input).map_err(|_| CommandError::runtime_unavailable())
}

fn is_active(snapshot: &RunSnapshot) -> bool {
    matches!(snapshot.status.as_str(), "accepted" | "running")
}

async fn refresh_session(
    session: &mut RuntimeSession,
    artifacts: &ArtifactRepository,
) -> Result<(), CommandError> {
    let events = session
        .sidecar
        .replay_events(
            &session.acceptance,
            Some(session.projection.snapshot.last_event_id),
        )
        .await
        .map_err(|_| CommandError::event_sync_failed())?;
    for event in &events {
        session.projection.apply(event);
    }
    session.projection.snapshot.events = events;
    if session.projection.snapshot.status == "completed" && !session.artifact_stored {
        let result = session
            .sidecar
            .workflow_result(&session.acceptance)
            .await
            .map_err(|_| CommandError::artifact_unavailable())?;
        artifacts.write(&session.acceptance.run_id, &result)?;
        session.artifact_stored = true;
    }
    // A failed run used to persist nothing, discarding every task artifact and
    // review it had already paid for and leaving the failure undiagnosable.
    // The sidecar cannot supply a workflow result here — it raised before
    // building one — so the durable event log and the projection are the record.
    if session.projection.snapshot.status == "failed" && !session.artifact_stored {
        let snapshot = &session.projection.snapshot;
        let record = serde_json::json!({
            "schema": "story-run-failure/v1",
            "run_id": snapshot.run_id,
            "job_id": snapshot.job_id,
            "status": "failed",
            "error": snapshot.error,
            "tasks_total": snapshot.tasks_total,
            "tasks_completed": snapshot.tasks_completed,
            "reviews_completed": snapshot.reviews_completed,
            "approvals_pending": snapshot.approvals_pending,
            "last_event_id": snapshot.last_event_id,
            "budget": snapshot.budget,
            "events": snapshot.events,
        });
        artifacts.write_failure(&session.acceptance.run_id, &record)?;
        session.artifact_stored = true;
    }
    Ok(())
}

impl ProjectionState {
    fn new(snapshot: RunSnapshot) -> Self {
        Self {
            snapshot,
            queued: HashSet::new(),
            started: HashSet::new(),
            completed: HashSet::new(),
            reviews: HashSet::new(),
            pending_approvals: HashSet::new(),
        }
    }

    fn apply(&mut self, event: &EventEnvelope) {
        if event.seq <= self.snapshot.last_event_id {
            return;
        }
        self.snapshot.last_event_id = event.seq;
        match event.event_type.as_str() {
            "run.started" => self.snapshot.status = "running".into(),
            "run.completed" => self.snapshot.status = "completed".into(),
            "run.failed" => {
                self.snapshot.status = "failed".into();
                self.snapshot.error = event.payload["error"].as_str().map(ToOwned::to_owned);
            }
            "run.cancelled" => self.snapshot.status = "cancelled".into(),
            "task.queued" => insert_task(&mut self.queued, event),
            "task.started" => insert_task(&mut self.started, event),
            "task.completed" => {
                insert_task(&mut self.completed, event);
                self.snapshot.budget.consumed_tokens =
                    self.snapshot.budget.consumed_tokens.saturating_add(
                        event.payload["usage"]["total_tokens"].as_u64().unwrap_or(0),
                    );
                let cost = event.payload["usage"]["cost_cny_fen"].as_u64().unwrap_or(0);
                self.snapshot.budget.consumed_cny_fen = Some(
                    self.snapshot
                        .budget
                        .consumed_cny_fen
                        .unwrap_or(0)
                        .saturating_add(cost),
                );
            }
            "review.completed" => insert_task(&mut self.reviews, event),
            "approval.requested" => {
                self.pending_approvals.insert(event.event_id.clone());
            }
            "approval.granted" | "approval.rejected" => {
                if let Some(request_id) = event.payload["request_event_id"].as_str() {
                    self.pending_approvals.remove(request_id);
                }
            }
            _ => {}
        }
        self.snapshot.tasks_queued = self.queued.len();
        self.snapshot.tasks_started = self.started.len();
        self.snapshot.tasks_completed = self.completed.len();
        self.snapshot.reviews_completed = self.reviews.len();
        self.snapshot.approvals_pending = self.pending_approvals.len();
    }
}

fn insert_task(target: &mut HashSet<String>, event: &EventEnvelope) {
    if let Some(task_id) = &event.task_id {
        target.insert(task_id.clone());
    }
}

fn python_executable(repository_root: &Path) -> PathBuf {
    std::env::var_os("MICROCODEX_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| repository_root.join(".venv/Scripts/python.exe"))
}

fn sidecar_launch_config(
    repository_root: &Path,
) -> Result<SidecarLaunchConfig, story_runtime::SidecarProcessError> {
    if let Some(executable) = bundled_sidecar_executable() {
        let data_directory = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("MicrocodeX")
            .join("ShortDramaStudio")
            .join("sidecar");
        std::fs::create_dir_all(&data_directory)
            .map_err(|_| story_runtime::SidecarProcessError::InvalidConfig)?;
        SidecarLaunchConfig::bundled(executable, data_directory, Duration::from_secs(15))
    } else {
        SidecarLaunchConfig::new(
            python_executable(repository_root),
            repository_root.join("sidecar"),
            Duration::from_secs(15),
        )
    }
}

fn bundled_sidecar_executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MICROCODEX_SIDECAR_EXE").map(PathBuf::from) {
        if path.is_absolute() && path.is_file() {
            return Some(path);
        }
    }
    let executable = std::env::current_exe().ok()?;
    [
        executable.parent()?.join("story-sidecar.exe"),
        executable.parent()?.join("resources/story-sidecar.exe"),
        executable
            .parent()?
            .join("resources/story-sidecar/story-sidecar.exe"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub fn default_repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[cfg(test)]
#[path = "run_controller_tests.rs"]
mod tests;
