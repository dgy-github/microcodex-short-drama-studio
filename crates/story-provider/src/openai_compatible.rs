use crate::ProviderSecret;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const MAX_PROVIDER_ATTEMPTS: usize = 3;
const MAX_RETRY_DELAY: Duration = Duration::from_secs(2);

pub struct ProviderRoute {
    endpoint: String,
    model: String,
    secret: ProviderSecret,
    thinking_disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderRouteError {
    #[error("provider route is invalid")]
    InvalidRoute,
    #[error("provider request failed")]
    Request,
    #[error("provider response is invalid")]
    InvalidResponse,
}

impl ProviderRoute {
    pub fn validate(endpoint: &str, model: &str) -> Result<(), ProviderRouteError> {
        let parsed = reqwest::Url::parse(endpoint).map_err(|_| ProviderRouteError::InvalidRoute)?;
        if endpoint.len() > 2048
            || model.len() > 128
            || endpoint.trim() != endpoint
            || parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !parsed.path().ends_with("/chat/completions")
            || model.trim().is_empty()
            || model.trim() != model
        {
            return Err(ProviderRouteError::InvalidRoute);
        }
        Ok(())
    }

    pub fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        secret: ProviderSecret,
    ) -> Result<Self, ProviderRouteError> {
        let endpoint = endpoint.into();
        let model = model.into();
        Self::validate(&endpoint, &model)?;
        Ok(Self {
            endpoint,
            model,
            secret,
            thinking_disabled: false,
        })
    }

    pub fn with_thinking_disabled(mut self) -> Self {
        self.thinking_disabled = true;
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

impl std::fmt::Debug for ProviderRoute {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderRoute")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderOutput {
    pub artifact: serde_json::Value,
    pub usage: ProviderUsage,
    pub model: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderUsage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: ProviderUsage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage<'a>; 2],
    response_format: ResponseFormat,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<Thinking>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct ResponseFormat {
    r#type: &'static str,
}

#[derive(Serialize)]
struct Thinking {
    r#type: &'static str,
}

pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    timeout: Duration,
}

impl OpenAiCompatibleProvider {
    pub fn new(timeout: Duration) -> Result<Self, ProviderRouteError> {
        if timeout.is_zero() {
            return Err(ProviderRouteError::InvalidRoute);
        }
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| ProviderRouteError::InvalidRoute)?;
        Ok(Self { client, timeout })
    }

    pub async fn generate_json(
        &self,
        route: &ProviderRoute,
        system: &str,
        prompt: &str,
    ) -> Result<ProviderOutput, ProviderRouteError> {
        if system.trim().is_empty() || prompt.trim().is_empty() {
            return Err(ProviderRouteError::InvalidRoute);
        }
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let mut bearer = b"Bearer ".to_vec();
        bearer.extend_from_slice(route.secret.expose_secret());
        let authorization =
            HeaderValue::from_bytes(&bearer).map_err(|_| ProviderRouteError::InvalidRoute)?;
        bearer.fill(0);
        headers.insert(AUTHORIZATION, authorization);

        let response = tokio::time::timeout(
            self.timeout,
            self.send_with_retry(route, headers, system, prompt),
        )
        .await
        .map_err(|_| ProviderRouteError::Request)??;
        let response = response
            .json::<ChatResponse>()
            .await
            .map_err(|_| ProviderRouteError::InvalidResponse)?;
        let content = response
            .choices
            .first()
            .map(|choice| choice.message.content.trim())
            .filter(|content| !content.is_empty())
            .ok_or(ProviderRouteError::InvalidResponse)?;
        let artifact: serde_json::Value = serde_json::from_str(strip_code_fence(content))
            .map_err(|_| ProviderRouteError::InvalidResponse)?;
        if !artifact.is_object() {
            return Err(ProviderRouteError::InvalidResponse);
        }
        Ok(ProviderOutput {
            artifact,
            usage: response.usage,
            model: route.model.clone(),
        })
    }

    async fn send_with_retry(
        &self,
        route: &ProviderRoute,
        headers: HeaderMap,
        system: &str,
        prompt: &str,
    ) -> Result<reqwest::Response, ProviderRouteError> {
        for attempt in 0..MAX_PROVIDER_ATTEMPTS {
            let response = self
                .client
                .post(&route.endpoint)
                .headers(headers.clone())
                .json(&ChatRequest {
                    model: &route.model,
                    messages: [
                        ChatMessage {
                            role: "system",
                            content: system,
                        },
                        ChatMessage {
                            role: "user",
                            content: prompt,
                        },
                    ],
                    response_format: ResponseFormat {
                        r#type: "json_object",
                    },
                    temperature: 0.4,
                    thinking: route
                        .thinking_disabled
                        .then_some(Thinking { r#type: "disabled" }),
                })
                .send()
                .await;
            match response {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response)
                    if retryable_status(response.status())
                        && attempt + 1 < MAX_PROVIDER_ATTEMPTS =>
                {
                    tokio::time::sleep(retry_delay(attempt, response.headers())).await;
                }
                Ok(_) => return Err(ProviderRouteError::Request),
                Err(_) if attempt + 1 < MAX_PROVIDER_ATTEMPTS => {
                    tokio::time::sleep(retry_delay(attempt, &HeaderMap::new())).await;
                }
                Err(_) => return Err(ProviderRouteError::Request),
            }
        }
        Err(ProviderRouteError::Request)
    }
}

fn retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn retry_delay(attempt: usize, headers: &HeaderMap) -> Duration {
    let provider_delay = headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs);
    provider_delay
        .unwrap_or_else(|| Duration::from_millis(250 * (attempt as u64 + 1)))
        .min(MAX_RETRY_DELAY)
}

/// Tolerate providers that wrap the JSON body in a Markdown code fence
/// (```` ```json ... ``` ````). Returns the inner JSON when a fence is present,
/// otherwise the trimmed content unchanged.
fn strip_code_fence(content: &str) -> &str {
    let trimmed = content.trim();
    if !trimmed.starts_with("```") {
        return trimmed;
    }
    let after_open = &trimmed[3..];
    let after_lang = after_open
        .split_once(['\n', '\r'])
        .map_or(after_open, |(_, rest)| rest);
    let inner = after_lang.trim_start();
    inner.find("```").map_or(trimmed, |end| inner[..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};
    use axum::routing::post;
    use axum::{Json, Router};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Clone)]
    struct RetryServerState {
        calls: Arc<AtomicUsize>,
        transient_then_success: bool,
    }

    async fn retry_server(State(state): State<RetryServerState>) -> Response {
        let call = state.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if !state.transient_then_success {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        if call == 1 {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        if call == 2 {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        Json(serde_json::json!({
            "choices": [{"message": {"content": "{\"ok\":true}"}}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        }))
        .into_response()
    }

    async fn local_retry_route(transient_then_success: bool) -> (ProviderRoute, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/v1/chat/completions", post(retry_server))
            .with_state(RetryServerState {
                calls: Arc::clone(&calls),
                transient_then_success,
            });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (
            ProviderRoute {
                endpoint: format!("http://{address}/v1/chat/completions"),
                model: "test-model".into(),
                secret: ProviderSecret::new(b"test-secret".to_vec()).unwrap(),
                thinking_disabled: false,
            },
            calls,
        )
    }

    #[test]
    fn route_requires_https_chat_completions_and_redacts_secret() {
        let route = ProviderRoute::new(
            "https://provider.test/v1/chat/completions",
            "model",
            ProviderSecret::new(b"sensitive-material".to_vec()).unwrap(),
        )
        .unwrap();
        let debug = format!("{route:?}");
        assert!(!debug.contains("sensitive-material"));
        assert_eq!(route.model(), "model");
        assert!(ProviderRoute::new(
            "http://provider.test/v1/chat/completions",
            "model",
            ProviderSecret::new(b"secret".to_vec()).unwrap(),
        )
        .is_err());
        assert!(
            ProviderRoute::validate("https://provider.test/v1/chat/completions", " model").is_err()
        );
        assert!(ProviderRoute::validate(
            "https://provider.test/v1/chat/completions",
            &"m".repeat(129)
        )
        .is_err());
        assert!(ProviderRoute::validate(
            "https://user:secret@provider.test/v1/chat/completions",
            "model"
        )
        .is_err());
        assert!(ProviderRoute::validate(
            "https://provider.test/v1/chat/completions?unsafe=true",
            "model"
        )
        .is_err());
    }

    #[test]
    fn strip_code_fence_extracts_inner_json_and_passes_plain_through() {
        assert_eq!(strip_code_fence("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_code_fence("```\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_code_fence("  {\"a\":1}  "), "{\"a\":1}");
        assert_eq!(strip_code_fence("plain text"), "plain text");
    }

    #[test]
    fn retries_only_transient_statuses_with_bounded_delays() {
        assert!(retryable_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(retryable_status(reqwest::StatusCode::SERVICE_UNAVAILABLE));
        assert!(!retryable_status(reqwest::StatusCode::UNAUTHORIZED));
        assert!(!retryable_status(reqwest::StatusCode::BAD_REQUEST));

        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("60"));
        assert_eq!(retry_delay(0, &headers), MAX_RETRY_DELAY);
        assert_eq!(
            retry_delay(0, &HeaderMap::new()),
            Duration::from_millis(250)
        );
        assert_eq!(
            retry_delay(1, &HeaderMap::new()),
            Duration::from_millis(500)
        );
    }

    #[tokio::test]
    async fn transient_http_failures_retry_until_success() {
        let (route, calls) = local_retry_route(true).await;
        let provider = OpenAiCompatibleProvider::new(Duration::from_secs(5)).unwrap();

        let output = provider
            .generate_json(&route, "system", "prompt")
            .await
            .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(output.artifact, serde_json::json!({"ok": true}));
        assert_eq!(output.usage.total_tokens, 2);
    }

    #[tokio::test]
    async fn authentication_failures_are_not_retried() {
        let (route, calls) = local_retry_route(false).await;
        let provider = OpenAiCompatibleProvider::new(Duration::from_secs(5)).unwrap();

        assert_eq!(
            provider.generate_json(&route, "system", "prompt").await,
            Err(ProviderRouteError::Request)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
