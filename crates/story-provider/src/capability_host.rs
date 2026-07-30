use crate::{OpenAiCompatibleProvider, ProviderRoute};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
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
    pub package_schema_path: PathBuf,
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
    package_schema: Value,
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
        let state = Arc::new(HostState {
            provider: OpenAiCompatibleProvider::new(config.request_timeout)
                .map_err(|_| CapabilityHostError::InvalidConfig)?,
            generation: config.generation,
            review: config.review,
            package_schema,
            token: config.token,
        });
        let app = Router::new()
            .route("/v1/capabilities", post(handle_capability))
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

async fn handle_capability(
    State(state): State<Arc<HostState>>,
    headers: HeaderMap,
    Json(request): Json<CapabilityRequest>,
) -> Result<Json<CapabilityResponse>, StatusCode> {
    let supplied = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let expected = format!("Bearer {}", state.token.expose());
    if !constant_time_equal(supplied.as_bytes(), expected.as_bytes()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
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
            Ok(Json(CapabilityResponse {
                schema: RESPONSE_SCHEMA,
                request_id,
                status: "ok",
                artifact: Some(output.artifact),
                usage: Some(
                    serde_json::to_value(output.usage)
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                ),
                model: Some(output.model),
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
                || !valid_package(&state.package_schema, &artifact, expected_episodes)
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
            }))
        }
    }
}

fn valid_package(schema: &Value, artifact: &Value, expected_episodes: u64) -> bool {
    let Ok(validator) = jsonschema::validator_for(schema) else {
        return false;
    };
    if !validator.is_valid(artifact) {
        return false;
    }
    if artifact["episodes"]
        .as_array()
        .map(|items| items.len() as u64)
        != Some(expected_episodes)
    {
        return false;
    }
    let mut known = HashSet::new();
    if let Some(value) = artifact["logline"]["node_id"].as_str() {
        known.insert(format!("story-package/{value}"));
    }
    if let Some(value) = artifact["promise"]["node_id"].as_str() {
        known.insert(format!("story-package/{value}"));
    }
    for collection in ["characters", "beats", "episodes", "scenes"] {
        for node in artifact[collection].as_array().into_iter().flatten() {
            let Some(node_id) = node["node_id"].as_str() else {
                return false;
            };
            let parent = format!("story-package/{node_id}");
            known.insert(parent.clone());
            if collection == "episodes" {
                if let Some(child) = node["end_hook"]["node_id"].as_str() {
                    known.insert(format!("{parent}/{child}"));
                }
            }
            if collection == "scenes" {
                for line in node["lines"].as_array().into_iter().flatten() {
                    if let Some(child) = line["node_id"].as_str() {
                        known.insert(format!("{parent}/{child}"));
                    }
                }
            }
        }
    }
    for collection in ["facts", "relationships", "timeline", "setups"] {
        for node in artifact["continuity_ledger"][collection]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(node_id) = node["node_id"].as_str() {
                known.insert(format!("story-package/{node_id}"));
            }
        }
    }
    let mut referenced = Vec::new();
    collect_refs(artifact, &mut referenced);
    referenced
        .into_iter()
        .all(|reference| known.contains(reference))
}

fn collect_refs<'a>(value: &'a Value, refs: &mut Vec<&'a str>) {
    match value {
        Value::String(text) if valid_span_ref(text) => {
            refs.push(text);
        }
        Value::Array(items) => {
            for item in items {
                collect_refs(item, refs);
            }
        }
        Value::Object(fields) => {
            for value in fields.values() {
                collect_refs(value, refs);
            }
        }
        _ => {}
    }
}

fn valid_span_ref(value: &str) -> bool {
    let Some(path) = value.strip_prefix("story-package/") else {
        return false;
    };
    !path.is_empty()
        && path.split('/').all(|segment| {
            let Some((kind, index)) = segment.rsplit_once('-') else {
                return false;
            };
            !kind.is_empty()
                && kind.bytes().all(|byte| byte.is_ascii_lowercase())
                && !index.starts_with('0')
                && index.parse::<u32>().is_ok_and(|value| value > 0)
        })
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
        assert!(!valid_span_ref("story-package/v1"));
        assert!(valid_span_ref("story-package/scene-2/dialogue-7"));
        assert!(!valid_span_ref("story-package/scene-0"));
    }
}
