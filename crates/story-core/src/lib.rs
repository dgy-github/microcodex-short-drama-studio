//! Product-owned short-drama job and artifact contracts.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenreMode {
    Auto,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryFormat {
    pub episodes: u16,
    pub minutes_per_episode: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryBudget {
    pub max_tokens: u64,
    pub max_cny_fen: u64,
    pub deadline_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ArtifactSpanRef(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArtifactSpanError {
    #[error("artifact span must contain an artifact kind and at least one node")]
    MissingNode,
    #[error("artifact span contains an invalid kind")]
    InvalidKind,
    #[error("artifact span node must end in a positive index")]
    InvalidIndex,
}

impl ArtifactSpanRef {
    pub fn parse(value: impl Into<String>) -> Result<Self, ArtifactSpanError> {
        let value = value.into();
        let segments: Vec<&str> = value.split('/').collect();
        if segments.len() < 2 {
            return Err(ArtifactSpanError::MissingNode);
        }
        if !valid_kind(segments[0]) {
            return Err(ArtifactSpanError::InvalidKind);
        }
        for segment in &segments[1..] {
            let Some((kind, index)) = segment.rsplit_once('-') else {
                return Err(ArtifactSpanError::InvalidIndex);
            };
            if !valid_kind(kind) {
                return Err(ArtifactSpanError::InvalidKind);
            }
            if index.starts_with('0') || index.parse::<u32>().is_err() {
                return Err(ArtifactSpanError::InvalidIndex);
            }
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_kind(value: &str) -> bool {
    value.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_lowercase() || (index > 0 && (byte.is_ascii_digit() || byte == b'-'))
    }) && !value.is_empty()
}

impl fmt::Display for ArtifactSpanRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ArtifactSpanRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Structural family for one project.
///
/// A different content form starts a different project. The initial release
/// deliberately exposes no mutation path and supports scripted drama only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentForm {
    ScriptedShortDrama,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryJob {
    pub schema: String,
    pub job_id: String,
    content_form: ContentForm,
    pub input: String,
    pub genre_mode: GenreMode,
    pub allowed_genres: Vec<String>,
    #[serde(default)]
    pub genre_pack_id: Option<String>,
    #[serde(default)]
    pub constraint_profile_id: Option<String>,
    pub audience: String,
    pub format: StoryFormat,
    pub content_limits: Vec<String>,
    pub budget: StoryBudget,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("job schema must be story-job/v1")]
    Schema,
    #[error("job id and input must not be blank")]
    Blank,
    #[error("episode count and duration must be positive")]
    Format,
}

impl StoryJob {
    pub fn content_form(&self) -> ContentForm {
        self.content_form
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema != "story-job/v1" {
            return Err(ValidationError::Schema);
        }
        if self.job_id.trim().is_empty() || self.input.trim().is_empty() {
            return Err(ValidationError::Blank);
        }
        if self.format.episodes == 0 || self.format.minutes_per_episode == 0 {
            return Err(ValidationError::Format);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> StoryJob {
        StoryJob {
            schema: "story-job/v1".into(),
            job_id: "job_1".into(),
            content_form: ContentForm::ScriptedShortDrama,
            input: "两名维修工必须在商场开门前修好同一部故障电梯。".into(),
            genre_mode: GenreMode::Auto,
            allowed_genres: vec!["family".into()],
            genre_pack_id: None,
            constraint_profile_id: None,
            audience: "25-45".into(),
            format: StoryFormat {
                episodes: 8,
                minutes_per_episode: 2,
            },
            content_limits: vec![],
            budget: StoryBudget {
                max_tokens: 100_000,
                max_cny_fen: 1_000,
                deadline_seconds: 600,
            },
        }
    }

    #[test]
    fn valid_job_passes() {
        assert_eq!(job().validate(), Ok(()));
    }

    #[test]
    fn blank_input_fails() {
        let mut value = job();
        value.input = " ".into();
        assert_eq!(value.validate(), Err(ValidationError::Blank));
    }

    #[test]
    fn content_form_round_trips_and_is_read_only() {
        let value = job();
        let encoded = serde_json::to_value(&value).unwrap();
        assert_eq!(encoded["content_form"], "scripted_short_drama");

        let decoded: StoryJob = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.content_form(), ContentForm::ScriptedShortDrama);
    }

    #[test]
    fn missing_or_unknown_content_form_is_rejected() {
        let mut missing = serde_json::to_value(job()).unwrap();
        missing.as_object_mut().unwrap().remove("content_form");
        assert!(serde_json::from_value::<StoryJob>(missing).is_err());

        let mut unknown = serde_json::to_value(job()).unwrap();
        unknown["content_form"] = serde_json::json!("knowledge_explainer");
        assert!(serde_json::from_value::<StoryJob>(unknown).is_err());
    }

    #[test]
    fn artifact_span_is_form_agnostic_and_validated() {
        let span = ArtifactSpanRef::parse("story-package/scene-2/dialogue-7").unwrap();
        assert_eq!(span.as_str(), "story-package/scene-2/dialogue-7");
        assert_eq!(
            ArtifactSpanRef::parse("story-package/scene-0"),
            Err(ArtifactSpanError::InvalidIndex)
        );
        assert_eq!(
            ArtifactSpanRef::parse("story-package"),
            Err(ArtifactSpanError::MissingNode)
        );
    }
}
