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
    retained: Arc<AtomicUsize>,
    validations: Arc<AtomicUsize>,
}

struct FakeCapabilityHost {
    endpoint: String,
    generated: Arc<AtomicUsize>,
    retained: Arc<AtomicUsize>,
    validations: Arc<AtomicUsize>,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl FakeCapabilityHost {
    async fn start(repository: &Path) -> Self {
        // eval/runs/ is generated output and gitignored; the tracked
        // baseline under eval/baselines/ is the fixture that survives a
        // clean checkout.
        let fixture = repository
            .join("eval/baselines/baseline-deepseek-v4-pro-20260727/family_001.story-package.json");
        let bytes = std::fs::read(&fixture).unwrap_or_else(|error| {
            panic!("tracked workflow fixture missing at {fixture:?}: {error}")
        });
        let package: Value = serde_json::from_slice(&bytes).unwrap();
        let generated = Arc::new(AtomicUsize::new(0));
        let retained = Arc::new(AtomicUsize::new(0));
        let validations = Arc::new(AtomicUsize::new(0));
        let state = FakeCapabilityState {
            token: TEST_CAPABILITY_TOKEN.into(),
            package,
            generated: Arc::clone(&generated),
            retained: Arc::clone(&retained),
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
            retained,
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
            let (artifact, model) = fake_artifact(task_name, &state.package, expected_episodes)?;
            Ok(Json(json!({
                "schema": "story-capability-response/v1",
                "request_id": request_id,
                "status": "ok",
                "artifact": artifact,
                "usage": {
                    "prompt_tokens": 1,
                    "completion_tokens": 1,
                    "total_tokens": 2,
                    "cost_cny_fen": 0,
                    "pricing_catalog_id": "deterministic-fixture"
                },
                "model": model
            })))
        }
        Some("retain_artifact") => {
            state.retained.fetch_add(1, Ordering::SeqCst);
            let bytes =
                serde_json::to_vec(&request["artifact"]).map_err(|_| StatusCode::BAD_REQUEST)?;
            let digest = sha256_hex(&bytes);
            Ok(Json(json!({
                "schema": "story-capability-response/v1",
                "request_id": request_id,
                "status": "ok",
                "content_ref": format!("artifact://sha256/{digest}"),
                "content_sha256": digest
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

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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
            consumed_cny_fen: Some(0),
        },
        events: Vec::new(),
    });
    let completed = event(
        2,
        "task.completed",
        Some("t01"),
        serde_json::json!({"usage": {"total_tokens": 123, "cost_cny_fen": 7}}),
    );
    projection.apply(&completed);
    projection.apply(&completed);
    assert_eq!(projection.snapshot.tasks_completed, 1);
    assert_eq!(projection.snapshot.budget.consumed_tokens, 123);
    assert_eq!(projection.snapshot.budget.consumed_cny_fen, Some(7));
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
        "error={:?}, events={:?}, generated={}, retained={}, validations={}",
        snapshot.error,
        snapshot.events,
        fake_host.generated.load(Ordering::SeqCst),
        fake_host.retained.load(Ordering::SeqCst),
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
    // every task artifact was retained through the capability seam (CAP-006)
    assert_eq!(fake_host.retained.load(Ordering::SeqCst), 17);
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
    let settings =
        ProviderSettingsService::new(crate::provider_settings::default_provider_settings_root())
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
