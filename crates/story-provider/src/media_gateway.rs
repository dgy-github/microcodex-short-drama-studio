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
}

#[derive(Serialize)]
struct GatewayRequest {
    schema: &'static str,
    request: Value,
}

#[derive(Deserialize)]
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
            .header(AUTHORIZATION, authorization)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
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
