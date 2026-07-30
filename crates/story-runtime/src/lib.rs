//! Asynchronous command and durable event protocol shared by Rust and Campaign.

mod diagnostics;
mod execution;
mod genre_packs;
mod run_protocol;
mod sidecar;

pub use diagnostics::{DiagnosticError, StructuredDiagnostic};
pub use execution::{
    fixed_story_execution_order, validate_fixed_story_execution_order, ExecutionOrderError,
    ExecutionTask, SidecarLifecycle, SidecarSignal, SidecarState, SidecarTransitionError,
};
pub use genre_packs::{
    GenreContext, GenrePackError, GenrePackOption, GenrePackRegistry, GenreRetrievalSource,
    HumanWritingContext,
};
pub use run_protocol::{CommandAcceptance, IdempotencyKey, IdempotencyKeyError};
pub use sidecar::{
    SidecarAuthToken, SidecarAuthTokenError, SidecarLaunchConfig, SidecarProcess,
    SidecarProcessError,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use story_core::{ContentForm, StoryJob};

pub const EVENT_PROTOCOL: &str = "story-agent-event/v1";
pub const CONTENT_FORM_REGISTRY_SCHEMA: &str = "content-form-registry/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRef(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AssetRefError {
    #[error("asset reference must be a portable relative path")]
    UnsafePath,
}

impl AssetRef {
    pub fn parse(value: impl Into<String>) -> Result<Self, AssetRefError> {
        let value = value.into();
        let mut segments = value.split('/');
        if value.is_empty()
            || value.starts_with('/')
            || value.contains('\\')
            || value.as_bytes().get(1) == Some(&b':')
            || segments.any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(AssetRefError::UnsafePath);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFormAssets {
    content_form: ContentForm,
    artifact_schema: AssetRef,
    rubric: AssetRef,
    case_set: AssetRef,
}

impl ContentFormAssets {
    pub fn content_form(&self) -> ContentForm {
        self.content_form
    }

    pub fn artifact_schema(&self) -> &AssetRef {
        &self.artifact_schema
    }

    pub fn rubric(&self) -> &AssetRef {
        &self.rubric
    }

    pub fn case_set(&self) -> &AssetRef {
        &self.case_set
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFormRegistry {
    bindings: Vec<ContentFormAssets>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("content-form registry is not valid JSON")]
    Decode(#[from] serde_json::Error),
    #[error("content-form registry schema must be content-form-registry/v1")]
    Schema,
    #[error("content-form registry must contain at least one binding")]
    Empty,
    #[error("content-form registry contains an unsafe asset path")]
    UnsafePath,
    #[error("content-form registry contains a duplicate form")]
    DuplicateForm,
    #[error("content form has no registered asset binding")]
    UnregisteredForm,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryDocument {
    schema: String,
    bindings: Vec<RawContentFormAssets>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContentFormAssets {
    content_form: ContentForm,
    artifact_schema: String,
    rubric: String,
    case_set: String,
}

impl ContentFormRegistry {
    pub fn from_json(input: &str) -> Result<Self, RegistryError> {
        let document: RegistryDocument = serde_json::from_str(input)?;
        if document.schema != CONTENT_FORM_REGISTRY_SCHEMA {
            return Err(RegistryError::Schema);
        }
        if document.bindings.is_empty() {
            return Err(RegistryError::Empty);
        }

        let mut forms = HashSet::new();
        let mut bindings = Vec::with_capacity(document.bindings.len());
        for raw in document.bindings {
            if !forms.insert(raw.content_form) {
                return Err(RegistryError::DuplicateForm);
            }
            bindings.push(ContentFormAssets {
                content_form: raw.content_form,
                artifact_schema: AssetRef::parse(raw.artifact_schema)
                    .map_err(|_| RegistryError::UnsafePath)?,
                rubric: AssetRef::parse(raw.rubric).map_err(|_| RegistryError::UnsafePath)?,
                case_set: AssetRef::parse(raw.case_set).map_err(|_| RegistryError::UnsafePath)?,
            });
        }
        Ok(Self { bindings })
    }

    pub fn resolve(&self, form: ContentForm) -> Result<&ContentFormAssets, RegistryError> {
        self.bindings
            .iter()
            .find(|binding| binding.content_form == form)
            .ok_or(RegistryError::UnregisteredForm)
    }

    pub fn resolve_job(&self, job: &StoryJob) -> Result<&ContentFormAssets, RegistryError> {
        self.resolve(job.content_form())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub protocol: String,
    pub event_id: String,
    pub seq: u64,
    pub occurred_at: String,
    pub causation_id: String,
    pub correlation_id: String,
    pub job_id: String,
    pub run_id: String,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub event_type: String,
    pub schema_version: u16,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EventError {
    #[error("event protocol must be story-agent-event/v1")]
    Protocol,
    #[error("durable event sequence must be positive")]
    Sequence,
    #[error("event identity and type fields must not be blank")]
    Identity,
}

impl EventEnvelope {
    pub fn validate(&self) -> Result<(), EventError> {
        if self.protocol != EVENT_PROTOCOL {
            return Err(EventError::Protocol);
        }
        if self.seq == 0 {
            return Err(EventError::Sequence);
        }
        let fields = [&self.event_id, &self.job_id, &self.run_id, &self.event_type];
        if fields.iter().any(|field| field.trim().is_empty()) {
            return Err(EventError::Identity);
        }
        Ok(())
    }

    pub fn deduplication_key(&self) -> (&str, u64) {
        (&self.run_id, self.seq)
    }
}

#[derive(Debug, Default)]
pub struct SequenceCursor {
    run_id: Option<String>,
    last_seq: u64,
}

impl SequenceCursor {
    pub fn accept(&mut self, event: &EventEnvelope) -> bool {
        if self.run_id.as_deref() != Some(event.run_id.as_str()) {
            self.run_id = Some(event.run_id.clone());
            self.last_seq = 0;
        }
        if event.seq <= self.last_seq {
            return false;
        }
        self.last_seq = event.seq;
        true
    }

    pub fn last_event_id(&self) -> Option<u64> {
        (self.last_seq > 0).then_some(self.last_seq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn event(seq: u64) -> EventEnvelope {
        EventEnvelope {
            protocol: EVENT_PROTOCOL.into(),
            event_id: format!("evt_{seq}"),
            seq,
            occurred_at: "2026-07-26T12:00:00Z".into(),
            causation_id: "cmd_1".into(),
            correlation_id: "req_1".into(),
            job_id: "job_1".into(),
            run_id: "run_1".into(),
            task_id: Some("t01".into()),
            agent_id: Some("story-coordinator".into()),
            event_type: "task.started".into(),
            schema_version: 1,
            payload: Value::Null,
        }
    }

    #[test]
    fn duplicate_sequence_is_ignored() {
        let mut cursor = SequenceCursor::default();
        assert!(cursor.accept(&event(1)));
        assert!(!cursor.accept(&event(1)));
        assert_eq!(cursor.last_event_id(), Some(1));
    }

    #[test]
    fn invalid_zero_sequence_fails() {
        assert_eq!(event(0).validate(), Err(EventError::Sequence));
    }

    #[test]
    fn real_registry_resolves_the_scripted_drama_assets() {
        let registry =
            ContentFormRegistry::from_json(include_str!("../../../config/content-forms.json"))
                .unwrap();
        let job: StoryJob = serde_json::from_value(serde_json::json!({
            "schema": "story-job/v1",
            "job_id": "job_1",
            "content_form": "scripted_short_drama",
            "input": "两名维修工必须在商场开门前修好同一部故障电梯。",
            "genre_mode": "auto",
            "allowed_genres": ["family"],
            "audience": "25-45",
            "format": {"episodes": 8, "minutes_per_episode": 2},
            "content_limits": [],
            "budget": {
                "max_tokens": 100000,
                "max_cny_fen": 1000,
                "deadline_seconds": 600
            }
        }))
        .unwrap();

        let assets = registry.resolve_job(&job).unwrap();
        assert_eq!(
            assets.artifact_schema().as_str(),
            "schemas/story-package-v1.json"
        );
        assert_eq!(assets.rubric().as_str(), "eval/rubrics/judge-v1.yaml");
        assert_eq!(
            assets.case_set().as_str(),
            "eval/manifests/eval-v0.1.0.json"
        );

        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for asset in [assets.artifact_schema(), assets.rubric(), assets.case_set()] {
            assert!(repository.join(asset.as_str()).is_file());
        }
    }

    #[test]
    fn duplicate_content_form_bindings_are_rejected() {
        let input = r#"{
            "schema": "content-form-registry/v1",
            "bindings": [
                {
                    "content_form": "scripted_short_drama",
                    "artifact_schema": "schemas/story-package-v1.json",
                    "rubric": "eval/rubrics/judge-v1.yaml",
                    "case_set": "eval/manifests/eval-v0.1.0.json"
                },
                {
                    "content_form": "scripted_short_drama",
                    "artifact_schema": "schemas/story-package-v1.json",
                    "rubric": "eval/rubrics/judge-v1.yaml",
                    "case_set": "eval/manifests/eval-v0.1.0.json"
                }
            ]
        }"#;
        assert!(matches!(
            ContentFormRegistry::from_json(input),
            Err(RegistryError::DuplicateForm)
        ));
    }

    #[test]
    fn unsafe_asset_references_are_rejected() {
        for path in [
            "../judge.yaml",
            "C:/judge.yaml",
            "/judge.yaml",
            "eval\\judge.yaml",
        ] {
            let input = serde_json::json!({
                "schema": "content-form-registry/v1",
                "bindings": [{
                    "content_form": "scripted_short_drama",
                    "artifact_schema": path,
                    "rubric": "eval/rubrics/judge-v1.yaml",
                    "case_set": "eval/manifests/eval-v0.1.0.json"
                }]
            })
            .to_string();
            assert!(matches!(
                ContentFormRegistry::from_json(&input),
                Err(RegistryError::UnsafePath)
            ));
        }
    }

    #[test]
    fn missing_form_binding_fails_closed() {
        let registry = ContentFormRegistry { bindings: vec![] };
        assert!(matches!(
            registry.resolve(ContentForm::ScriptedShortDrama),
            Err(RegistryError::UnregisteredForm)
        ));
    }
}
