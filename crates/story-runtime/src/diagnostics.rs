use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructuredDiagnostic {
    pub schema: &'static str,
    pub code: String,
    pub component: String,
    pub message: String,
    pub context: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DiagnosticError {
    #[error("diagnostic identity is invalid")]
    InvalidIdentity,
}

impl StructuredDiagnostic {
    pub fn new(
        code: impl Into<String>,
        component: impl Into<String>,
        message: impl Into<String>,
        context: Value,
    ) -> Result<Self, DiagnosticError> {
        let code = code.into();
        let component = component.into();
        if !valid_identity(&code) || !valid_identity(&component) {
            return Err(DiagnosticError::InvalidIdentity);
        }
        Ok(Self {
            schema: "structured-diagnostic/v1",
            code,
            component,
            message: redact_text(&message.into()),
            context: redact_value(context),
        })
    }
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| {
                    if sensitive_key(&key) {
                        (key, Value::String("[REDACTED]".into()))
                    } else {
                        (key, redact_value(value))
                    }
                })
                .collect::<Map<_, _>>(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_value).collect()),
        Value::String(value) => Value::String(redact_text(&value)),
        value => value,
    }
}

fn sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "secret",
        "token",
        "api_key",
        "authorization",
        "chain_of_thought",
        "reasoning",
        "prompt",
    ]
    .iter()
    .any(|part| normalized.contains(part))
}

fn redact_text(value: &str) -> String {
    value
        .split_whitespace()
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.starts_with("sk-") || lower.starts_with("bearer") {
                "[REDACTED]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_remove_secrets_prompts_and_chain_of_thought() {
        let diagnostic = StructuredDiagnostic::new(
            "provider_failed",
            "story_provider",
            "request used sk-sensitive",
            serde_json::json!({
                "authorization": "Bearer secret",
                "nested": {
                    "chain_of_thought": "private reasoning",
                    "prompt": "licensed source text",
                    "status": 503
                }
            }),
        )
        .unwrap();
        let encoded = serde_json::to_string(&diagnostic).unwrap();
        assert!(!encoded.contains("sensitive"));
        assert!(!encoded.contains("private reasoning"));
        assert!(!encoded.contains("licensed source text"));
        assert!(encoded.contains("\"status\":503"));
    }
}
