use crate::ProviderSecret;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

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
}

impl OpenAiCompatibleProvider {
    pub fn new(timeout: std::time::Duration) -> Result<Self, ProviderRouteError> {
        if timeout.is_zero() {
            return Err(ProviderRouteError::InvalidRoute);
        }
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| ProviderRouteError::InvalidRoute)?;
        Ok(Self { client })
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

        let response = self
            .client
            .post(&route.endpoint)
            .headers(headers)
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
            .await
            .map_err(|_| ProviderRouteError::Request)?;
        if !response.status().is_success() {
            return Err(ProviderRouteError::Request);
        }
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
}
