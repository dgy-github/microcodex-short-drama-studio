use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use story_core::StoryJob;
use story_provider::{CapabilityHost, CapabilityHostConfig, CapabilityToken};
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

        let mut capability_material = format!(
            "desktop-capability-{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let capability_host = CapabilityHost::start(CapabilityHostConfig {
            generation,
            review,
            package_schema_path: self.repository_root.join("schemas/story-package-v1.json"),
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
        Ok(is_active(&session.projection.snapshot)
            .then(|| session.projection.snapshot.clone()))
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
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};
    use serde_json::{json, Value};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::oneshot;
    use tokio::task::JoinHandle;

    const TEST_CAPABILITY_TOKEN: &str = "desktop-test-capability-token-20260728";

    #[derive(Clone)]
    struct FakeCapabilityState {
        token: String,
        package: Value,
        generated: Arc<AtomicUsize>,
        validations: Arc<AtomicUsize>,
    }

    struct FakeCapabilityHost {
        endpoint: String,
        generated: Arc<AtomicUsize>,
        validations: Arc<AtomicUsize>,
        shutdown: Option<oneshot::Sender<()>>,
        task: JoinHandle<Result<(), std::io::Error>>,
    }

    impl FakeCapabilityHost {
        async fn start(repository: &Path) -> Self {
            let package: Value = serde_json::from_slice(
                &std::fs::read(
                    repository.join(
                        "eval/runs/baseline-deepseek-v4-pro-20260727/artifacts/family_001.story-package.json",
                    ),
                )
                .unwrap(),
            )
            .unwrap();
            let generated = Arc::new(AtomicUsize::new(0));
            let validations = Arc::new(AtomicUsize::new(0));
            let state = FakeCapabilityState {
                token: TEST_CAPABILITY_TOKEN.into(),
                package,
                generated: Arc::clone(&generated),
                validations: Arc::clone(&validations),
            };
            let app = Router::new()
                .route("/v1/capabilities", post(fake_capability))
                .with_state(state);
            let listener =
                tokio::net::TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
                    .await
                    .unwrap();
            let address = listener.local_addr().unwrap();
            let (shutdown, receiver) = oneshot::channel();
            let task = tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = receiver.await;
                    })
                    .await
            });
            Self {
                endpoint: format!("http://{address}"),
                generated,
                validations,
                shutdown: Some(shutdown),
                task,
            }
        }

        async fn stop(mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            self.task.await.unwrap().unwrap();
        }
    }

    async fn fake_capability(
        State(state): State<FakeCapabilityState>,
        headers: HeaderMap,
        Json(request): Json<Value>,
    ) -> Result<Json<Value>, StatusCode> {
        if headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            != Some(format!("Bearer {}", state.token).as_str())
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let request_id = request["request_id"]
            .as_str()
            .ok_or(StatusCode::BAD_REQUEST)?;
        match request["capability"].as_str() {
            Some("generate_structured_text") => {
                state.generated.fetch_add(1, Ordering::SeqCst);
                let prompt = request["prompt"].as_str().ok_or(StatusCode::BAD_REQUEST)?;
                let task_name = prompt
                    .lines()
                    .next()
                    .and_then(|line| line.split_once('=').map(|(_, value)| value))
                    .ok_or(StatusCode::BAD_REQUEST)?;
                let expected_episodes = prompt
                    .lines()
                    .find_map(|line| line.strip_prefix("输入="))
                    .and_then(|value| serde_json::from_str::<Value>(value).ok())
                    .and_then(|value| value["job"]["format"]["episodes"].as_u64())
                    .and_then(|value| usize::try_from(value).ok());
                let (artifact, model) =
                    fake_artifact(task_name, &state.package, expected_episodes)?;
                Ok(Json(json!({
                    "schema": "story-capability-response/v1",
                    "request_id": request_id,
                    "status": "ok",
                    "artifact": artifact,
                    "usage": {
                        "prompt_tokens": 1,
                        "completion_tokens": 1,
                        "total_tokens": 2
                    },
                    "model": model
                })))
            }
            Some("validate_artifact") => {
                state.validations.fetch_add(1, Ordering::SeqCst);
                Ok(Json(json!({
                    "schema": "story-capability-response/v1",
                    "request_id": request_id,
                    "status": "ok"
                })))
            }
            _ => Err(StatusCode::BAD_REQUEST),
        }
    }

    fn fake_artifact(
        task_name: &str,
        package: &Value,
        expected_episodes: Option<usize>,
    ) -> Result<(Value, &'static str), StatusCode> {
        let review = match task_name {
            "continuity_review" => Some(("t11", "continuity")),
            "human_taste_review" => Some(("t12", "human_taste")),
            "originality_review" => Some(("t13", "originality")),
            "production_review" => Some(("t14", "production")),
            "final_review" => Some(("t16", "final")),
            _ => None,
        };
        if let Some((task_id, review_type)) = review {
            return Ok((
                json!({
                    "schema": "story-review-record/v1",
                    "review_id": format!("review_{task_id}_desktop_test"),
                    "task_id": task_id,
                    "review_type": review_type,
                    "status": "pass",
                    "summary": "Deterministic desktop integration review passed.",
                    "findings": []
                }),
                "deterministic-review",
            ));
        }
        if task_name == "targeted_revision" {
            return Ok((package.clone(), "deterministic-generation"));
        }
        if task_name == "plan_episodes" {
            let episode_count = expected_episodes.ok_or(StatusCode::BAD_REQUEST)?;
            let episodes = package["episodes"]
                .as_array()
                .ok_or(StatusCode::BAD_REQUEST)?
                .iter()
                .take(episode_count)
                .cloned()
                .collect::<Vec<_>>();
            return Ok((
                json!({
                    "schema": "episode-plan/v1",
                    "episodes": episodes
                }),
                "deterministic-generation",
            ));
        }
        if let Some(index) = task_name.strip_prefix("write_episode_") {
            let episode_index = index
                .parse::<usize>()
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            return Ok((
                json!({
                    "schema": "sample-scenes/v1",
                    "scenes": [{
                        "location": format!("Episode {episode_index} location"),
                        "lines": [
                            {
                                "kind": "action",
                                "text": format!("Episode {episode_index} action.")
                            },
                            {
                                "kind": "dialogue",
                                "speaker": "Lead",
                                "text": format!("Episode {episode_index} dialogue."),
                                "subtext": "Unspoken deterministic intent."
                            }
                        ]
                    }]
                }),
                "deterministic-generation",
            ));
        }
        const KNOWN_TASKS: &[&str] = &[
            "classify_genre",
            "propose_architecture_a",
            "propose_architecture_b",
            "propose_architecture_c",
            "debate_and_select",
            "deepen_characters",
            "build_story_beats",
        ];
        if KNOWN_TASKS.contains(&task_name) {
            return Ok((
                json!({"schema": "test-artifact/v1", "value": task_name}),
                "deterministic-generation",
            ));
        }
        Err(StatusCode::BAD_REQUEST)
    }

    fn event(
        seq: u64,
        event_type: &str,
        task_id: Option<&str>,
        payload: serde_json::Value,
    ) -> EventEnvelope {
        EventEnvelope {
            protocol: "story-agent-event/v1".into(),
            event_id: format!("evt_{seq}"),
            seq,
            occurred_at: "2026-07-28T00:00:00Z".into(),
            causation_id: "req_1".into(),
            correlation_id: "req_1".into(),
            job_id: "job_1".into(),
            run_id: "run_0148aa190ce842c8b103d3885a68dfcb".into(),
            task_id: task_id.map(ToOwned::to_owned),
            agent_id: None,
            event_type: event_type.into(),
            schema_version: 1,
            payload,
        }
    }

    #[test]
    fn projection_deduplicates_sequences_and_usage() {
        let mut projection = ProjectionState::new(RunSnapshot {
            schema: "desktop-run-snapshot/v1",
            run_id: "run_0148aa190ce842c8b103d3885a68dfcb".into(),
            job_id: "job_1".into(),
            status: "running".into(),
            last_event_id: 1,
            tasks_total: TASKS_TOTAL,
            tasks_queued: 0,
            tasks_started: 0,
            tasks_completed: 0,
            reviews_completed: 0,
            approvals_pending: 0,
            error: None,
            budget: BudgetSnapshot {
                max_tokens: 1000,
                consumed_tokens: 0,
                max_cny_fen: 100,
                consumed_cny_fen: None,
            },
            events: Vec::new(),
        });
        let completed = event(
            2,
            "task.completed",
            Some("t01"),
            serde_json::json!({"usage": {"total_tokens": 123}}),
        );
        projection.apply(&completed);
        projection.apply(&completed);
        assert_eq!(projection.snapshot.tasks_completed, 1);
        assert_eq!(projection.snapshot.budget.consumed_tokens, 123);
    }

    #[tokio::test]
    async fn deterministic_desktop_runtime_completes_full_workflow_and_stores_artifact() {
        let repository = default_repository_root().canonicalize().unwrap();
        let fake_host = FakeCapabilityHost::start(&repository).await;
        let workspace = tempfile::tempdir().unwrap();
        let artifacts = ArtifactRepository::new(workspace.path().join("artifacts"));
        let controller = DesktopRunController::new(repository);
        let job: StoryJob = serde_json::from_value(json!({
            "schema": "story-job/v1",
            "job_id": format!("job_desktop_deterministic_{}", Uuid::new_v4().simple()),
            "content_form": "scripted_short_drama",
            "input": "A repair worker must rescue an estranged parent before a locked mall reopens.",
            "genre_mode": "fixed",
            "allowed_genres": ["family", "suspense"],
            "audience": "25-45",
            "format": {"episodes": 6, "minutes_per_episode": 2},
            "content_limits": ["No supernatural explanation"],
            "budget": {
                "max_tokens": 90000,
                "max_cny_fen": 1200,
                "deadline_seconds": 60
            }
        }))
        .unwrap();
        let mut snapshot = controller
            .start_with_test_capability(
                &artifacts,
                job.clone(),
                fake_host.endpoint.clone(),
                CapabilityToken::new(TEST_CAPABILITY_TOKEN).unwrap(),
            )
            .await
            .unwrap();
        let duplicate = controller
            .start_with_test_capability(
                &artifacts,
                job,
                fake_host.endpoint.clone(),
                CapabilityToken::new(TEST_CAPABILITY_TOKEN).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(duplicate.run_id, snapshot.run_id);

        for _ in 0..300 {
            snapshot = controller.sync(&artifacts).await.unwrap();
            if matches!(
                snapshot.status.as_str(),
                "completed" | "failed" | "cancelled"
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        assert_eq!(
            snapshot.status,
            "completed",
            "error={:?}, events={:?}, generated={}, validations={}",
            snapshot.error,
            snapshot.events,
            fake_host.generated.load(Ordering::SeqCst),
            fake_host.validations.load(Ordering::SeqCst)
        );
        assert_eq!(snapshot.tasks_queued, 17);
        assert_eq!(snapshot.tasks_started, 17);
        assert_eq!(snapshot.tasks_completed, 17);
        assert_eq!(snapshot.reviews_completed, 5);
        assert_eq!(snapshot.approvals_pending, 0);
        assert_eq!(snapshot.budget.consumed_tokens, 40);
        assert!(controller
            .refresh_and_find_active(&artifacts)
            .await
            .unwrap()
            .is_none());
        assert_eq!(fake_host.generated.load(Ordering::SeqCst), 20);
        assert_eq!(fake_host.validations.load(Ordering::SeqCst), 2);

        let result = artifacts.read(&snapshot.run_id).unwrap();
        assert_eq!(result["schema"], "story-workflow-result/v1");
        assert_eq!(result["promotion"], "non-promotable");
        assert_eq!(result["tasks"].as_array().unwrap().len(), 17);
        assert_eq!(result["reviews"].as_array().unwrap().len(), 5);
        assert_eq!(result["package"]["episodes"].as_array().unwrap().len(), 6);
        assert_eq!(result["package"]["scenes"].as_array().unwrap().len(), 6);
        assert_eq!(
            result["provider_routes"],
            json!({
                "generation": "deterministic-generation",
                "review": "deterministic-review"
            })
        );
        fake_host.stop().await;
    }

    #[tokio::test]
    #[ignore = "uses configured provider credentials and runs the paid 17-task workflow"]
    async fn configured_desktop_runtime_completes_and_stores_a_story() {
        let repository = default_repository_root();
        let artifact_root = repository
            .join("target/desktop-live-artifacts")
            .join(std::process::id().to_string());
        let artifacts = ArtifactRepository::new(artifact_root);
        let credentials = CredentialService::new();
        let controller = DesktopRunController::new(repository);
        let job: StoryJob = serde_json::from_value(serde_json::json!({
            "schema": "story-job/v1",
            "job_id": format!("job_desktop_live_{}", Uuid::new_v4().simple()),
            "content_form": "scripted_short_drama",
            "input": "停电后的老旧商场里，一名维修工发现故障电梯中被困的是二十年前离开的父亲。",
            "genre_mode": "fixed",
            "allowed_genres": ["family", "suspense"],
            "audience": "25-45",
            "format": {"episodes": 6, "minutes_per_episode": 2},
            "content_limits": ["不使用超自然解释"],
            "budget": {
                "max_tokens": 90000,
                "max_cny_fen": 1200,
                "deadline_seconds": 900
            }
        }))
        .unwrap();
        let settings = ProviderSettingsService::new(
            crate::provider_settings::default_provider_settings_root(),
        )
        .unwrap();
        let mut snapshot = controller
            .start(&credentials, &settings, &artifacts, job)
            .await
            .unwrap();
        for _ in 0..2400 {
            snapshot = controller.sync(&artifacts).await.unwrap();
            if matches!(
                snapshot.status.as_str(),
                "completed" | "failed" | "cancelled"
            ) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        assert_eq!(snapshot.status, "completed", "{:?}", snapshot.error);
        assert_eq!(snapshot.tasks_completed, 17);
        assert_eq!(snapshot.reviews_completed, 5);
        assert!(artifacts.read(&snapshot.run_id).is_ok());
    }
}
