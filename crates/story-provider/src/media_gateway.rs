use crate::ProviderSecret;
use base64::Engine;
use reqwest::header::{HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

const MAX_RESPONSE_BYTES: usize = 256 * 1024 * 1024;

pub struct MediaGatewayRoute {
    endpoint: String,
    secret: ProviderSecret,
}

impl MediaGatewayRoute {
    pub fn validate(endpoint: &str) -> Result<(), MediaGatewayError> {
        let parsed = reqwest::Url::parse(endpoint).map_err(|_| MediaGatewayError::InvalidRoute)?;
        let secure_transport = parsed.scheme() == "https"
            || (parsed.scheme() == "http"
                && matches!(parsed.host_str(), Some("127.0.0.1" | "localhost")));
        if endpoint.len() > 2048
            || !secure_transport
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !parsed.path().ends_with("/v1/media/generate")
        {
            return Err(MediaGatewayError::InvalidRoute);
        }
        Ok(())
    }

    pub fn new(
        endpoint: impl Into<String>,
        secret: ProviderSecret,
    ) -> Result<Self, MediaGatewayError> {
        let endpoint = endpoint.into();
        Self::validate(&endpoint)?;
        Ok(Self { endpoint, secret })
    }
}

pub struct MediaGatewayClient {
    client: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaGatewayOutput {
    pub mime_type: String,
    pub bytes: Vec<u8>,
    pub provider: String,
    pub model: String,
    pub cost_cny_fen: u64,
    pub pricing_catalog_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaGatewayError {
    #[error("media gateway route is invalid")]
    InvalidRoute,
    #[error("media gateway request failed")]
    Request,
    #[error("media gateway response is invalid")]
    InvalidResponse,
    #[error("media gateway task timed out")]
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTaskHandle {
    pub task_id: String,
    pub status_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewayTaskResponse {
    schema: String,
    task_id: String,
    status_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewayTaskStatus {
    schema: String,
    status: String,
    #[serde(default)]
    result: Option<GatewayResponse>,
    #[serde(default, rename = "error")]
    _error: Option<String>,
}

#[derive(Serialize)]
struct GatewayRequest {
    schema: &'static str,
    request: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GatewayResponse {
    schema: String,
    mime_type: String,
    content_base64: String,
    provider: String,
    model: String,
    cost_cny_fen: u64,
    pricing_catalog_id: String,
}

impl MediaGatewayClient {
    pub fn new(timeout: Duration) -> Result<Self, MediaGatewayError> {
        if timeout.is_zero() {
            return Err(MediaGatewayError::InvalidRoute);
        }
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| MediaGatewayError::InvalidRoute)?;
        Ok(Self { client })
    }

    pub async fn generate(
        &self,
        route: &MediaGatewayRoute,
        request: Value,
    ) -> Result<MediaGatewayOutput, MediaGatewayError> {
        if !request.is_object() {
            return Err(MediaGatewayError::InvalidResponse);
        }
        let mut bearer = b"Bearer ".to_vec();
        bearer.extend_from_slice(route.secret.expose_secret());
        let authorization =
            HeaderValue::from_bytes(&bearer).map_err(|_| MediaGatewayError::InvalidRoute)?;
        bearer.fill(0);
        let response = self
            .client
            .post(&route.endpoint)
            .header(AUTHORIZATION, authorization.clone())
            .json(&GatewayRequest {
                schema: "media-gateway-request/v1",
                request,
            })
            .send()
            .await
            .map_err(|_| MediaGatewayError::Request)?;
        if response.status() == reqwest::StatusCode::ACCEPTED {
            let task: GatewayTaskResponse = response
                .json()
                .await
                .map_err(|_| MediaGatewayError::InvalidResponse)?;
            return self
                .poll_task(route, authorization, task, Duration::from_secs(2), 150)
                .await;
        }
        if !response.status().is_success() {
            return Err(MediaGatewayError::Request);
        }
        let body: GatewayResponse = response
            .json()
            .await
            .map_err(|_| MediaGatewayError::InvalidResponse)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(body.content_base64)
            .map_err(|_| MediaGatewayError::InvalidResponse)?;
        if body.schema != "media-gateway-response/v1"
            || bytes.is_empty()
            || bytes.len() > MAX_RESPONSE_BYTES
            || body.provider.trim().is_empty()
            || body.model.trim().is_empty()
            || body.pricing_catalog_id.trim().is_empty()
        {
            return Err(MediaGatewayError::InvalidResponse);
        }
        Ok(MediaGatewayOutput {
            mime_type: body.mime_type,
            bytes,
            provider: body.provider,
            model: body.model,
            cost_cny_fen: body.cost_cny_fen,
            pricing_catalog_id: body.pricing_catalog_id,
        })
    }

    /// Submit an asynchronous provider task and poll it with bounded delay.
    /// The gateway owns provider-specific submission and response mapping; this
    /// layer only enforces the authenticated, provider-neutral task contract.
    pub async fn generate_async(
        &self,
        route: &MediaGatewayRoute,
        request: Value,
        poll_interval: Duration,
        max_polls: usize,
    ) -> Result<MediaGatewayOutput, MediaGatewayError> {
        if !request.is_object() || poll_interval.is_zero() || max_polls == 0 {
            return Err(MediaGatewayError::InvalidResponse);
        }
        let mut bearer = b"Bearer ".to_vec();
        bearer.extend_from_slice(route.secret.expose_secret());
        let authorization =
            HeaderValue::from_bytes(&bearer).map_err(|_| MediaGatewayError::InvalidRoute)?;
        bearer.fill(0);
        let response = self
            .client
            .post(&route.endpoint)
            .header(AUTHORIZATION, authorization.clone())
            .json(&GatewayRequest {
                schema: "media-gateway-request/v1",
                request,
            })
            .send()
            .await
            .map_err(|_| MediaGatewayError::Request)?;
        if !response.status().is_success() {
            return Err(MediaGatewayError::Request);
        }
        let task: GatewayTaskResponse = response
            .json()
            .await
            .map_err(|_| MediaGatewayError::InvalidResponse)?;
        self.poll_task(route, authorization, task, poll_interval, max_polls)
            .await
    }

    async fn poll_task(
        &self,
        route: &MediaGatewayRoute,
        authorization: HeaderValue,
        task: GatewayTaskResponse,
        poll_interval: Duration,
        max_polls: usize,
    ) -> Result<MediaGatewayOutput, MediaGatewayError> {
        let submit_url =
            reqwest::Url::parse(&route.endpoint).map_err(|_| MediaGatewayError::InvalidRoute)?;
        let status_url = reqwest::Url::parse(&task.status_url)
            .map_err(|_| MediaGatewayError::InvalidResponse)?;
        let same_origin = submit_url.scheme() == status_url.scheme()
            && submit_url.host_str() == status_url.host_str()
            && submit_url.port_or_known_default() == status_url.port_or_known_default();
        if task.schema != "media-gateway-task/v1"
            || !valid_task_id(&task.task_id)
            || !same_origin
            || !status_url.path().starts_with("/v1/media/tasks/")
            || status_url.query().is_some()
            || status_url.fragment().is_some()
            || !status_url.username().is_empty()
            || status_url.password().is_some()
        {
            return Err(MediaGatewayError::InvalidResponse);
        }
        for _ in 0..max_polls {
            let status = self
                .client
                .get(&task.status_url)
                .header(AUTHORIZATION, authorization.clone())
                .send()
                .await
                .map_err(|_| MediaGatewayError::Request)?;
            if !status.status().is_success() {
                return Err(MediaGatewayError::Request);
            }
            let body: GatewayTaskStatus = status
                .json()
                .await
                .map_err(|_| MediaGatewayError::InvalidResponse)?;
            if body.schema != "media-gateway-task-status/v1" {
                return Err(MediaGatewayError::InvalidResponse);
            }
            match body.status.as_str() {
                "succeeded" => {
                    return body
                        .result
                        .ok_or(MediaGatewayError::InvalidResponse)
                        .and_then(decode_gateway_output)
                }
                "failed" | "cancelled" => return Err(MediaGatewayError::Request),
                "queued" | "running" => tokio::time::sleep(poll_interval).await,
                _ => return Err(MediaGatewayError::InvalidResponse),
            }
        }
        Err(MediaGatewayError::Timeout)
    }
}

fn valid_task_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn decode_gateway_output(body: GatewayResponse) -> Result<MediaGatewayOutput, MediaGatewayError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(body.content_base64)
        .map_err(|_| MediaGatewayError::InvalidResponse)?;
    if bytes.is_empty()
        || bytes.len() > MAX_RESPONSE_BYTES
        || body.provider.trim().is_empty()
        || body.model.trim().is_empty()
        || body.pricing_catalog_id.trim().is_empty()
    {
        return Err(MediaGatewayError::InvalidResponse);
    }
    Ok(MediaGatewayOutput {
        mime_type: body.mime_type,
        bytes,
        provider: body.provider,
        model: body.model,
        cost_cny_fen: body.cost_cny_fen,
        pricing_catalog_id: body.pricing_catalog_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::{Json, Router};

    async fn generate(
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Result<Json<Value>, StatusCode> {
        if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some("Bearer media-secret")
            || body["schema"] != "media-gateway-request/v1"
            || body["request"]["schema"] != "image-generation-request/v1"
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        Ok(Json(serde_json::json!({
            "schema":"media-gateway-response/v1",
            "mime_type":"image/png",
            "content_base64":base64::engine::general_purpose::STANDARD.encode(b"fixture-image"),
            "provider":"fake-gateway",
            "model":"fake-image-v1",
            "cost_cny_fen":2,
            "pricing_catalog_id":"fixture-pricing"
        })))
    }

    async fn submit_task(State(origin): State<String>, Json(body): Json<Value>) -> Json<Value> {
        let mode = body["request"]["mode"].as_str().unwrap_or("success");
        Json(serde_json::json!({
            "schema":"media-gateway-task/v1",
            "task_id":format!("task_{mode}"),
            "status_url":format!("{origin}/v1/media/tasks/{mode}")
        }))
    }

    async fn task_status(axum::extract::Path(mode): axum::extract::Path<String>) -> Json<Value> {
        let body = match mode.as_str() {
            "success" => serde_json::json!({
                "schema":"media-gateway-task-status/v1", "status":"succeeded",
                "result": {"schema":"media-gateway-response/v1", "mime_type":"video/mp4",
                    "content_base64":base64::engine::general_purpose::STANDARD.encode(b"fixture-video"),
                    "provider":"wan", "model":"wan2.1", "cost_cny_fen":9,
                    "pricing_catalog_id":"fixture-pricing"}
            }),
            "failed" => {
                serde_json::json!({"schema":"media-gateway-task-status/v1", "status":"failed"})
            }
            _ => serde_json::json!({"schema":"media-gateway-task-status/v1", "status":"running"}),
        };
        Json(body)
    }

    async fn task_fixture() -> (MediaGatewayRoute, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let origin = format!("http://{address}");
        let app = Router::new()
            .route("/v1/media/generate", post(submit_task))
            .route("/v1/media/tasks/{mode}", get(task_status))
            .with_state(origin.clone());
        let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (
            MediaGatewayRoute {
                endpoint: format!("{origin}/v1/media/generate"),
                secret: ProviderSecret::new(b"media-secret".to_vec()).unwrap(),
            },
            handle,
        )
    }

    #[tokio::test]
    async fn authenticated_loopback_gateway_exercises_real_http_contract() {
        let app = Router::new().route("/v1/media/generate", post(generate));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let route = MediaGatewayRoute {
            endpoint: format!("http://{address}/v1/media/generate"),
            secret: ProviderSecret::new(b"media-secret".to_vec()).unwrap(),
        };
        let output = MediaGatewayClient::new(Duration::from_secs(5))
            .unwrap()
            .generate(
                &route,
                serde_json::json!({"schema":"image-generation-request/v1"}),
            )
            .await
            .unwrap();
        assert_eq!(output.bytes, b"fixture-image");
        assert_eq!(output.cost_cny_fen, 2);
    }

    #[tokio::test]
    async fn asynchronous_gateway_polls_success_and_fails_closed() {
        let (route, _server) = task_fixture().await;
        let client = MediaGatewayClient::new(Duration::from_secs(2)).unwrap();
        let output = client
            .generate_async(
                &route,
                serde_json::json!({"mode":"success"}),
                Duration::from_millis(1),
                2,
            )
            .await
            .unwrap();
        assert_eq!(output.bytes, b"fixture-video");
        assert_eq!(
            client
                .generate_async(
                    &route,
                    serde_json::json!({"mode":"failed"}),
                    Duration::from_millis(1),
                    2
                )
                .await,
            Err(MediaGatewayError::Request)
        );
        assert_eq!(
            client
                .generate_async(
                    &route,
                    serde_json::json!({"mode":"running"}),
                    Duration::from_millis(1),
                    2
                )
                .await,
            Err(MediaGatewayError::Timeout)
        );
    }

    #[tokio::test]
    async fn asynchronous_gateway_rejects_cross_origin_poll_url() {
        async fn malicious() -> Json<Value> {
            Json(serde_json::json!({
            "schema":"media-gateway-task/v1", "task_id":"task_1",
            "status_url":"https://attacker.example/v1/media/tasks/task_1"}))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/v1/media/generate", post(malicious)),
            )
            .await
            .unwrap()
        });
        let route = MediaGatewayRoute {
            endpoint: format!("http://{address}/v1/media/generate"),
            secret: ProviderSecret::new(b"media-secret".to_vec()).unwrap(),
        };
        assert_eq!(
            MediaGatewayClient::new(Duration::from_secs(2))
                .unwrap()
                .generate_async(&route, serde_json::json!({}), Duration::from_millis(1), 1)
                .await,
            Err(MediaGatewayError::InvalidResponse)
        );
    }

    #[test]
    fn route_requires_https_and_redacts_url_credentials() {
        assert!(MediaGatewayRoute::new(
            "http://provider.test/v1/media/generate",
            ProviderSecret::new(b"secret".to_vec()).unwrap()
        )
        .is_err());
        assert!(MediaGatewayRoute::new(
            "https://user:pass@provider.test/v1/media/generate",
            ProviderSecret::new(b"secret".to_vec()).unwrap()
        )
        .is_err());
        assert!(MediaGatewayRoute::new(
            "http://127.0.0.1:8080/v1/media/generate",
            ProviderSecret::new(b"secret".to_vec()).unwrap()
        )
        .is_ok());
    }
}
