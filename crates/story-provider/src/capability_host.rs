use crate::media_capability::{
    Request as MediaProjectCapabilityRequest, Response as MediaProjectCapabilityResponse,
};
use crate::{OpenAiCompatibleProvider, PricingCatalog, ProviderRoute};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use story_storage::artifacts::ContentAddressedStore;
use story_storage::media_projects::MediaProjectRepository;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use zeroize::Zeroize;

const REQUEST_SCHEMA: &str = "story-capability-request/v1";
const RESPONSE_SCHEMA: &str = "story-capability-response/v1";

pub struct CapabilityToken(String);

impl CapabilityToken {
    pub fn new(value: impl Into<String>) -> Result<Self, CapabilityHostError> {
        let value = value.into();
        if value.len() < 32 || !value.is_ascii() {
            return Err(CapabilityHostError::InvalidConfig);
        }
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for CapabilityToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CapabilityToken([REDACTED])")
    }
}

impl Drop for CapabilityToken {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

pub struct CapabilityHostConfig {
    pub generation: ProviderRoute,
    pub review: ProviderRoute,
    pub pricing: PricingCatalog,
    pub package_schema_path: PathBuf,
    pub retained_store_root: PathBuf,
    /// Rust-owned root for append-only media project history.
    pub media_project_store_root: PathBuf,
    pub token: CapabilityToken,
    pub request_timeout: std::time::Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityHostError {
    #[error("capability host configuration is invalid")]
    InvalidConfig,
    #[error("capability host could not bind loopback")]
    Bind,
    #[error("capability host failed")]
    Serve,
}

struct HostState {
    provider: OpenAiCompatibleProvider,
    generation: ProviderRoute,
    review: ProviderRoute,
    pricing: PricingCatalog,
    package_schema: Value,
    retained_store: ContentAddressedStore,
    media_projects: MediaProjectRepository,
    token: CapabilityToken,
}

pub struct CapabilityHost {
    address: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), std::io::Error>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "capability")]
enum CapabilityRequest {
    #[serde(rename = "generate_structured_text")]
    Generate {
        schema: String,
        request_id: String,
        route: RouteName,
        system: String,
        prompt: String,
    },
    #[serde(rename = "validate_artifact")]
    Validate {
        schema: String,
        request_id: String,
        artifact_schema: String,
        artifact: Value,
        expected_episodes: u64,
    },
    #[serde(rename = "retain_artifact")]
    Retain {
        schema: String,
        request_id: String,
        run_id: String,
        task_id: String,
        artifact_schema: String,
        artifact: Value,
    },
    #[serde(rename = "load_artifact")]
    Load {
        schema: String,
        request_id: String,
        content_ref: String,
        content_sha256: String,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RouteName {
    Generation,
    Review,
}

#[derive(Serialize)]
struct CapabilityResponse {
    schema: &'static str,
    request_id: String,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_sha256: Option<String>,
}

impl CapabilityHost {
    pub async fn start(config: CapabilityHostConfig) -> Result<Self, CapabilityHostError> {
        if !config.package_schema_path.is_file() || config.request_timeout.is_zero() {
            return Err(CapabilityHostError::InvalidConfig);
        }
        let schema_bytes = std::fs::read(&config.package_schema_path)
            .map_err(|_| CapabilityHostError::InvalidConfig)?;
        let package_schema = serde_json::from_slice(&schema_bytes)
            .map_err(|_| CapabilityHostError::InvalidConfig)?;
        jsonschema::validator_for(&package_schema)
            .map_err(|_| CapabilityHostError::InvalidConfig)?;
        let retained_store = ContentAddressedStore::open(&config.retained_store_root)
            .map_err(|_| CapabilityHostError::InvalidConfig)?;
        let media_projects = MediaProjectRepository::open(&config.media_project_store_root)
            .map_err(|_| CapabilityHostError::InvalidConfig)?;
        let state = Arc::new(HostState {
            provider: OpenAiCompatibleProvider::new(config.request_timeout)
                .map_err(|_| CapabilityHostError::InvalidConfig)?,
            generation: config.generation,
            review: config.review,
            pricing: config.pricing,
            package_schema,
            retained_store,
            media_projects,
            token: config.token,
        });
        let app = Router::new()
            .route("/v1/capabilities", post(handle_capability))
            .route(
                "/v1/media-projects/records",
                post(handle_media_project_record),
            )
            .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
            .with_state(state);
        let listener =
            tokio::net::TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
                .await
                .map_err(|_| CapabilityHostError::Bind)?;
        let address = listener
            .local_addr()
            .map_err(|_| CapabilityHostError::Bind)?;
        let (shutdown, receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = receiver.await;
                })
                .await
        });
        Ok(Self {
            address,
            shutdown: Some(shutdown),
            task,
        })
    }

    pub fn endpoint(&self) -> String {
        format!("http://{}", self.address)
    }

    pub async fn stop(mut self) -> Result<(), CapabilityHostError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task
            .await
            .map_err(|_| CapabilityHostError::Serve)?
            .map_err(|_| CapabilityHostError::Serve)
    }
}

async fn handle_media_project_record(
    State(state): State<Arc<HostState>>,
    headers: HeaderMap,
    Json(request): Json<MediaProjectCapabilityRequest>,
) -> Result<Json<MediaProjectCapabilityResponse>, StatusCode> {
    authorize(&state, &headers)?;
    let repository = state.media_projects.clone();
    tokio::task::spawn_blocking(move || crate::media_capability::append(&repository, request))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(Json(MediaProjectCapabilityResponse {
        schema: "media-project-capability-response/v1",
        status: "stored",
    }))
}

fn authorize(state: &HostState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let supplied = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let expected = format!("Bearer {}", state.token.expose());
    if constant_time_equal(supplied.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn handle_capability(
    State(state): State<Arc<HostState>>,
    headers: HeaderMap,
    Json(request): Json<CapabilityRequest>,
) -> Result<Json<CapabilityResponse>, StatusCode> {
    authorize(&state, &headers)?;
    match request {
        CapabilityRequest::Generate {
            schema,
            request_id,
            route,
            system,
            prompt,
        } => {
            if schema != REQUEST_SCHEMA {
                return Err(StatusCode::BAD_REQUEST);
            }
            let provider_route = match route {
                RouteName::Generation => &state.generation,
                RouteName::Review => &state.review,
            };
            let output = state
                .provider
                .generate_json(provider_route, &system, &prompt)
                .await
                .map_err(|_| StatusCode::BAD_GATEWAY)?;
            let provider = match route {
                RouteName::Generation => "deepseek",
                RouteName::Review => "aliyun_bailian",
            };
            let quote = state
                .pricing
                .quote(
                    provider,
                    &output.model,
                    output.usage.prompt_tokens,
                    output.usage.completion_tokens,
                )
                .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
            let mut usage = serde_json::to_value(output.usage)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            usage["cost_cny_fen"] = Value::from(quote.cost_cny_fen);
            usage["pricing_catalog_id"] = Value::from(quote.catalog_id);
            Ok(Json(CapabilityResponse {
                schema: RESPONSE_SCHEMA,
                request_id,
                status: "ok",
                artifact: Some(output.artifact),
                usage: Some(usage),
                model: Some(output.model),
                content_ref: None,
                content_sha256: None,
            }))
        }
        CapabilityRequest::Validate {
            schema,
            request_id,
            artifact_schema,
            artifact,
            expected_episodes,
        } => {
            if schema != REQUEST_SCHEMA
                || artifact_schema != "story-package/v1"
                || expected_episodes == 0
                || !crate::package_validation::valid_package(
                    &state.package_schema,
                    &artifact,
                    expected_episodes,
                )
            {
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
            Ok(Json(CapabilityResponse {
                schema: RESPONSE_SCHEMA,
                request_id,
                status: "ok",
                artifact: None,
                usage: None,
                model: None,
                content_ref: None,
                content_sha256: None,
            }))
        }
        CapabilityRequest::Retain {
            schema,
            request_id,
            run_id,
            task_id,
            artifact_schema,
            artifact,
        } => {
            if schema != REQUEST_SCHEMA {
                return Err(StatusCode::BAD_REQUEST);
            }
            let reference = state
                .retained_store
                .put(&run_id, &task_id, &artifact_schema, &artifact)
                .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
            Ok(Json(CapabilityResponse {
                schema: RESPONSE_SCHEMA,
                request_id,
                status: "ok",
                artifact: None,
                usage: None,
                model: None,
                content_ref: Some(reference.content_ref),
                content_sha256: Some(reference.content_sha256),
            }))
        }
        CapabilityRequest::Load {
            schema,
            request_id,
            content_ref,
            content_sha256,
        } => {
            if schema != REQUEST_SCHEMA {
                return Err(StatusCode::BAD_REQUEST);
            }
            let artifact = state
                .retained_store
                .load(&content_ref, &content_sha256)
                .map_err(|_| StatusCode::NOT_FOUND)?;
            Ok(Json(CapabilityResponse {
                schema: RESPONSE_SCHEMA,
                request_id,
                status: "ok",
                artifact: Some(artifact),
                usage: None,
                model: None,
                content_ref: None,
                content_sha256: None,
            }))
        }
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_bounded_redacted_and_compared_without_early_byte_exit() {
        assert!(CapabilityToken::new("short").is_err());
        let token = CapabilityToken::new("capability-test-token-with-at-least-32-bytes").unwrap();
        assert_eq!(format!("{token:?}"), "CapabilityToken([REDACTED])");
        assert!(constant_time_equal(b"same", b"same"));
        assert!(!constant_time_equal(b"same", b"sxme"));
    }

    #[test]
    fn schema_identity_is_not_misclassified_as_an_artifact_span() {
        assert!(!crate::package_validation::valid_span_ref(
            "story-package/v1"
        ));
        assert!(crate::package_validation::valid_span_ref(
            "story-package/scene-2/dialogue-7"
        ));
        assert!(!crate::package_validation::valid_span_ref(
            "story-package/scene-0"
        ));
    }

    fn test_config(retained_root: &std::path::Path) -> CapabilityHostConfig {
        let secret =
            || crate::ProviderSecret::new(b"test-secret-material-32-bytes-long!!").expect("secret");
        CapabilityHostConfig {
            generation: ProviderRoute::new(
                "https://api.example.com/chat/completions",
                "generator-model",
                secret(),
            )
            .expect("generation route"),
            review: ProviderRoute::new(
                "https://api.example.com/chat/completions",
                "review-model",
                secret(),
            )
            .expect("review route"),
            pricing: PricingCatalog::from_json(
                r#"{
                    "schema":"provider-pricing-catalog/v1",
                    "catalog_id":"capability-fixture",
                    "effective_at":"2026-01-01T00:00:00Z",
                    "entries":[
                        {"provider":"deepseek","model":"generator-model","prompt_cny_fen_per_million_tokens":1,"completion_cny_fen_per_million_tokens":1},
                        {"provider":"aliyun_bailian","model":"review-model","prompt_cny_fen_per_million_tokens":1,"completion_cny_fen_per_million_tokens":1}
                    ]
                }"#,
            )
            .expect("pricing fixture"),
            package_schema_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/story-package-v1.json"),
            retained_store_root: retained_root.to_path_buf(),
            media_project_store_root: retained_root.join("media-projects"),
            token: CapabilityToken::new("capability-test-token-with-at-least-32-bytes")
                .expect("token"),
            request_timeout: std::time::Duration::from_secs(5),
        }
    }

    #[tokio::test]
    async fn artifact_retain_and_load_require_the_authenticated_typed_capability() {
        let directory = tempfile::tempdir().expect("tempdir");
        let host = CapabilityHost::start(test_config(&directory.path().join("retained")))
            .await
            .expect("host starts");
        let client = reqwest::Client::new();
        let endpoint = format!("{}/v1/capabilities", host.endpoint());
        let authorization = "Bearer capability-test-token-with-at-least-32-bytes".to_string();

        let retain = serde_json::json!({
            "schema": REQUEST_SCHEMA,
            "capability": "retain_artifact",
            "request_id": "cap_retain_test",
            "run_id": "run_capability_test",
            "task_id": "t06",
            "artifact_schema": "architecture-decision/v1",
            "artifact": {"selected": "architecture-a"}
        });
        let response = client
            .post(&endpoint)
            .header("Authorization", &authorization)
            .json(&retain)
            .send()
            .await
            .expect("retain request");
        assert_eq!(response.status(), 200);
        let body: serde_json::Value = response.json().await.expect("retain body");
        let content_ref = body["content_ref"]
            .as_str()
            .expect("content ref")
            .to_string();
        let content_sha256 = body["content_sha256"].as_str().expect("hash").to_string();
        assert!(content_ref.starts_with("artifact://sha256/"));

        let load = serde_json::json!({
            "schema": REQUEST_SCHEMA,
            "capability": "load_artifact",
            "request_id": "cap_load_test",
            "content_ref": content_ref,
            "content_sha256": content_sha256
        });
        let response = client
            .post(&endpoint)
            .header("Authorization", &authorization)
            .json(&load)
            .send()
            .await
            .expect("load request");
        assert_eq!(response.status(), 200);
        let body: serde_json::Value = response.json().await.expect("load body");
        assert_eq!(body["artifact"]["selected"], "architecture-a");

        // unauthenticated retain is refused before touching the store
        let response = client
            .post(&endpoint)
            .json(&retain)
            .send()
            .await
            .expect("unauthenticated");
        assert_eq!(response.status(), 401);

        host.stop().await.expect("host stops");
    }

    #[tokio::test]
    async fn media_project_history_requires_authentication_and_valid_parentage() {
        let directory = tempfile::tempdir().expect("tempdir");
        let host = CapabilityHost::start(test_config(&directory.path().join("retained")))
            .await
            .expect("host starts");
        let client = reqwest::Client::new();
        let endpoint = format!("{}/v1/media-projects/records", host.endpoint());
        let authorization = "Bearer capability-test-token-with-at-least-32-bytes";
        let revision = serde_json::json!({
            "schema":"media-project-capability-request/v1",
            "operation":"append_prompt_revision",
            "record":{
                "schema":"image-prompt-revision/v1",
                "project_id":"project_1",
                "revision_id":"prompt_1",
                "parent_revision_id":null,
                "prompt":"厨房夜晚，钥匙位于前景",
                "source_spans":["story-package/scene-1"]
            }
        });

        let unauthenticated = client
            .post(&endpoint)
            .json(&revision)
            .send()
            .await
            .expect("unauthenticated request");
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let stored = client
            .post(&endpoint)
            .header("Authorization", authorization)
            .json(&revision)
            .send()
            .await
            .expect("revision request");
        assert_eq!(stored.status(), StatusCode::OK);
        assert_eq!(
            stored.json::<Value>().await.expect("response body"),
            serde_json::json!({
                "schema":"media-project-capability-response/v1",
                "status":"stored"
            })
        );

        let generation_request = serde_json::json!({
            "schema":"media-project-capability-request/v1",
            "operation":"append_generation_request",
            "record":{
                "schema":"image-generation-request/v1",
                "request_id":"img_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "project_id":"project_1",
                "prompt_revision_id":"prompt_1",
                "prompt":"厨房夜晚，钥匙位于前景",
                "source_spans":["story-package/scene-1"]
            }
        });
        let generation_stored = client
            .post(&endpoint)
            .header("Authorization", authorization)
            .json(&generation_request)
            .send()
            .await
            .expect("generation request");
        assert_eq!(generation_stored.status(), StatusCode::OK);

        let duplicate = client
            .post(&endpoint)
            .header("Authorization", authorization)
            .json(&revision)
            .send()
            .await
            .expect("duplicate request");
        assert_eq!(duplicate.status(), StatusCode::CONFLICT);

        let orphan = serde_json::json!({
            "schema":"media-project-capability-request/v1",
            "operation":"append_prompt_revision",
            "record":{
                "schema":"image-prompt-revision/v1",
                "project_id":"project_1",
                "revision_id":"prompt_2",
                "parent_revision_id":"prompt_missing",
                "prompt":"孤立修订",
                "source_spans":["story-package/scene-1"]
            }
        });
        let missing_parent = client
            .post(&endpoint)
            .header("Authorization", authorization)
            .json(&orphan)
            .send()
            .await
            .expect("orphan request");
        assert_eq!(missing_parent.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let wrong_schema = serde_json::json!({
            "schema":"media-project-capability-request/v2",
            "operation":"append_prompt_revision",
            "record":revision["record"]
        });
        let invalid = client
            .post(&endpoint)
            .header("Authorization", authorization)
            .json(&wrong_schema)
            .send()
            .await
            .expect("invalid request");
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

        host.stop().await.expect("host stops");
    }
}
