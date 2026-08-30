//! Trusted execution seam for story-grounded image and video generation.

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use story_storage::media::{MediaArtifactStore, MediaKind, MediaStoreError};

mod editor;
mod gateway;
mod run;
mod timeline;
mod tool;
mod tool_manifest;
pub use editor::{
    execute_timeline, retain_timeline_output, TimelineExecutionError, TimelineExecutionReceipt,
};
pub use gateway::GatewayMediaProvider;
pub use run::{MediaRunOutcome, MediaRunService};
pub use timeline::{compile_concat_plan, FfmpegPlan, TimelineClip};
pub use tool::{run_tool, validate_tool_path, MediaToolError, MediaToolOutput, MediaToolSpec};
pub use tool_manifest::{MediaToolEntry, MediaToolManifest, ToolManifestError};

pub type ProviderFuture<'a> =
    Pin<Box<dyn Future<Output = Result<GeneratedMedia, MediaProviderError>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageGenerationRequest {
    pub schema: String,
    pub request_id: String,
    pub project_id: String,
    pub prompt_revision_id: String,
    pub prompt: String,
    pub source_spans: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoGenerationRequest {
    pub schema: String,
    pub request_id: String,
    pub project_id: String,
    pub image_artifact_ref: String,
    pub story_spans: Vec<String>,
    pub prompt: String,
    #[serde(default)]
    pub generation_tier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaRequest {
    Image(ImageGenerationRequest),
    Video(VideoGenerationRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedMedia {
    pub mime_type: String,
    pub bytes: Vec<u8>,
    pub provider: String,
    pub model: String,
    pub cost_cny_fen: u64,
    pub pricing_catalog_id: String,
}

pub trait MediaProvider: Send + Sync {
    fn generate<'a>(&'a self, request: &'a MediaRequest) -> ProviderFuture<'a>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MediaGenerationResult {
    pub schema: &'static str,
    pub project_id: String,
    pub request_id: String,
    pub kind: MediaKind,
    pub mime_type: String,
    pub content_ref: String,
    pub content_sha256: String,
    pub byte_len: usize,
    pub provider: String,
    pub model: String,
    pub cost_cny_fen: u64,
    pub pricing_catalog_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaProviderError {
    #[error("media provider rejected or failed the request")]
    Failed,
}

#[derive(Debug, thiserror::Error)]
pub enum MediaExecutionError {
    #[error("media generation request is invalid")]
    InvalidRequest,
    #[error("media provider failed")]
    Provider(#[from] MediaProviderError),
    #[error("generated media metadata is invalid")]
    InvalidOutput,
    #[error("generated media could not be retained")]
    Storage(#[from] MediaStoreError),
    #[error("media run event persistence failed")]
    Events(#[from] story_storage::media_events::MediaEventStoreError),
}

pub struct MediaExecutor<P> {
    provider: P,
    store: MediaArtifactStore,
}

impl<P: MediaProvider> MediaExecutor<P> {
    pub fn new(provider: P, store: MediaArtifactStore) -> Self {
        Self { provider, store }
    }

    pub async fn execute(
        &self,
        request: MediaRequest,
    ) -> Result<MediaGenerationResult, MediaExecutionError> {
        validate_request(&request)?;
        if let MediaRequest::Video(value) = &request {
            self.store
                .verify_project_image(&value.project_id, &value.image_artifact_ref)?;
        }
        let generated = self.provider.generate(&request).await?;
        validate_output(&generated)?;
        let (project_id, request_id, kind) = identity(&request);
        let retained = self.store.put(
            project_id,
            request_id,
            kind,
            &generated.mime_type,
            &generated.bytes,
        )?;
        Ok(MediaGenerationResult {
            schema: "media-generation-result/v1",
            project_id: project_id.into(),
            request_id: request_id.into(),
            kind,
            mime_type: retained.mime_type,
            content_ref: retained.content_ref,
            content_sha256: retained.content_sha256,
            byte_len: retained.byte_len,
            provider: generated.provider,
            model: generated.model,
            cost_cny_fen: generated.cost_cny_fen,
            pricing_catalog_id: generated.pricing_catalog_id,
        })
    }
}

pub(crate) fn validate_request(request: &MediaRequest) -> Result<(), MediaExecutionError> {
    let (schema, request_id, project_id, prompt, spans) = match request {
        MediaRequest::Image(value) => (
            value.schema.as_str(),
            value.request_id.as_str(),
            value.project_id.as_str(),
            value.prompt.as_str(),
            value.source_spans.as_slice(),
        ),
        MediaRequest::Video(value) => {
            if !valid_artifact_ref(&value.image_artifact_ref) {
                return Err(MediaExecutionError::InvalidRequest);
            }
            if value
                .generation_tier
                .as_deref()
                .is_some_and(|tier| !matches!(tier, "coarse" | "fine"))
            {
                return Err(MediaExecutionError::InvalidRequest);
            }
            (
                value.schema.as_str(),
                value.request_id.as_str(),
                value.project_id.as_str(),
                value.prompt.as_str(),
                value.story_spans.as_slice(),
            )
        }
    };
    let expected = match request {
        MediaRequest::Image(_) => "image-generation-request/v1",
        MediaRequest::Video(_) => "video-generation-request/v1",
    };
    let valid_request_id = match request {
        MediaRequest::Image(_) => valid_prefixed_id(request_id, "img_"),
        MediaRequest::Video(_) => valid_prefixed_id(request_id, "vid_"),
    };
    if schema != expected
        || !valid_request_id
        || !valid_id(project_id)
        || prompt.trim().is_empty()
        || prompt.len() > 20_000
        || spans.is_empty()
        || spans
            .iter()
            .any(|span| span.trim().is_empty() || span.len() > 512)
    {
        return Err(MediaExecutionError::InvalidRequest);
    }
    Ok(())
}

fn validate_output(output: &GeneratedMedia) -> Result<(), MediaExecutionError> {
    if output.bytes.is_empty()
        || output.provider.trim().is_empty()
        || output.model.trim().is_empty()
        || output.pricing_catalog_id.trim().is_empty()
    {
        return Err(MediaExecutionError::InvalidOutput);
    }
    Ok(())
}

pub(crate) fn identity(request: &MediaRequest) -> (&str, &str, MediaKind) {
    match request {
        MediaRequest::Image(value) => (&value.project_id, &value.request_id, MediaKind::Image),
        MediaRequest::Video(value) => (&value.project_id, &value.request_id, MediaKind::Video),
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_prefixed_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn valid_artifact_ref(value: &str) -> bool {
    let Some(digest) = value.strip_prefix("artifact://sha256/") else {
        return false;
    };
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct FixtureProvider;

    impl MediaProvider for FixtureProvider {
        fn generate<'a>(&'a self, request: &'a MediaRequest) -> ProviderFuture<'a> {
            Box::pin(async move {
                let (mime_type, bytes) = match request {
                    MediaRequest::Image(_) => ("image/png", b"fixture-image".to_vec()),
                    MediaRequest::Video(_) => ("video/mp4", b"fixture-video".to_vec()),
                };
                Ok(GeneratedMedia {
                    mime_type: mime_type.into(),
                    bytes,
                    provider: "fixture-provider".into(),
                    model: "fixture-model".into(),
                    cost_cny_fen: 3,
                    pricing_catalog_id: "fixture-pricing".into(),
                })
            })
        }
    }

    fn executor() -> (MediaExecutor<FixtureProvider>, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(directory.path()).unwrap();
        (MediaExecutor::new(FixtureProvider, store), directory)
    }

    #[tokio::test]
    async fn image_and_video_execute_through_provider_and_immutable_store() {
        let (executor, _directory) = executor();
        let image = MediaRequest::Image(ImageGenerationRequest {
            schema: "image-generation-request/v1".into(),
            request_id: format!("img_{}", "a".repeat(32)),
            project_id: "project_1".into(),
            prompt_revision_id: "prompt_1".into(),
            prompt: "一张有明确动作的竖屏画面".into(),
            source_spans: vec!["story-package/scene-1".into()],
        });
        let image_result = executor.execute(image).await.unwrap();
        assert_eq!(image_result.kind, MediaKind::Image);
        assert!(image_result.content_ref.starts_with("artifact://sha256/"));

        let video = MediaRequest::Video(VideoGenerationRequest {
            schema: "video-generation-request/v1".into(),
            request_id: format!("vid_{}", "b".repeat(32)),
            project_id: "project_1".into(),
            image_artifact_ref: image_result.content_ref,
            story_spans: vec!["story-package/scene-1".into()],
            prompt: "镜头缓慢推近".into(),
            generation_tier: Some("coarse".into()),
        });
        let video_result = executor.execute(video).await.unwrap();
        assert_eq!(video_result.kind, MediaKind::Video);
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/media-agent/media-generation-result-v1.json"
        ))
        .unwrap();
        assert!(jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&json!(video_result)));
    }

    #[tokio::test]
    async fn unsafe_or_unproven_inputs_fail_before_provider_execution() {
        let (executor, _directory) = executor();
        let request = MediaRequest::Video(VideoGenerationRequest {
            schema: "video-generation-request/v1".into(),
            request_id: "vid_bad".into(),
            project_id: "../escape".into(),
            image_artifact_ref: "C:/image.png".into(),
            story_spans: vec![],
            prompt: "".into(),
            generation_tier: None,
        });
        assert!(matches!(
            executor.execute(request).await,
            Err(MediaExecutionError::InvalidRequest)
        ));
    }

    #[tokio::test]
    async fn video_rejects_a_well_formed_but_unretained_image_reference() {
        let (executor, _directory) = executor();
        let request = MediaRequest::Video(VideoGenerationRequest {
            schema: "video-generation-request/v1".into(),
            request_id: format!("vid_{}", "c".repeat(32)),
            project_id: "project_1".into(),
            image_artifact_ref: format!("artifact://sha256/{}", "d".repeat(64)),
            story_spans: vec!["story-package/scene-1".into()],
            prompt: "镜头缓慢推近".into(),
            generation_tier: None,
        });
        assert!(matches!(
            executor.execute(request).await,
            Err(MediaExecutionError::Storage(_))
        ));
    }
}
