use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use story_media::{
    execute_timeline, retain_timeline_output, GatewayMediaProvider, ImageGenerationRequest,
    MediaExecutor, MediaRequest, MediaRunOutcome, MediaRunService, MediaToolManifest, TimelineClip,
    VideoGenerationRequest,
};
use story_provider::{MediaGatewayClient, MediaGatewayRoute};
use story_storage::media::{MediaArtifactRef, MediaArtifactStore, MediaKind};
use story_storage::media_events::MediaEventStore;
use tokio::sync::{watch, Mutex};

use crate::credentials::CredentialService;
use crate::media_gateway_settings::MediaGatewaySettingsService;
use crate::media_projects::DesktopMediaProjects;
use crate::CommandError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesktopMediaRunResult {
    pub schema: &'static str,
    pub run_id: String,
    pub status: &'static str,
    pub result: Option<story_media::MediaGenerationResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopTimelineRequest {
    pub schema: String,
    pub project_id: String,
    pub request_id: String,
    pub clips: Vec<DesktopTimelineClip>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopTimelineClip {
    pub content_ref: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

impl DesktopTimelineRequest {
    pub(crate) fn validate(&self) -> Result<(), CommandError> {
        if self.schema != "desktop-media-timeline-request/v1"
            || !valid_safe_id(&self.project_id, 96)
            || !self.request_id.strip_prefix("edit_").is_some_and(|suffix| suffix.len() == 32 && suffix.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            || self.clips.is_empty() || self.clips.len() > 32
            || self.clips.iter().any(|clip| !valid_artifact_ref(&clip.content_ref)
                || !clip.start_seconds.is_finite() || !clip.end_seconds.is_finite()
                || clip.start_seconds < 0.0 || clip.end_seconds <= clip.start_seconds
                || clip.end_seconds > 300.0)
        { return Err(CommandError::invalid_media_project()); }
        Ok(())
    }
}

fn valid_safe_id(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_artifact_ref(value: &str) -> bool {
    value.strip_prefix("artifact://sha256/").is_some_and(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
}

pub struct DesktopMediaRuntime {
    root: PathBuf,
    active: Mutex<Option<(String, watch::Sender<bool>)>>,
}

impl DesktopMediaRuntime {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            active: Mutex::new(None),
        }
    }

    pub async fn start(
        &self,
        credentials: &CredentialService,
        settings: &MediaGatewaySettingsService,
        projects: &DesktopMediaProjects,
        run_id: String,
        request: Value,
        resume: bool,
    ) -> Result<DesktopMediaRunResult, CommandError> {
        let request = parse_request(request)?;
        let (project_id, request_id) = request_identity(&request);
        if !projects.history(project_id)?.iter().any(|record| {
            record.record_type == "generation_request" && record.record_id == request_id
        }) {
            return Err(CommandError::invalid_media_project());
        }
        let settings = settings
            .load()?
            .ok_or_else(CommandError::media_runtime_unavailable)?;
        let (endpoint, profile) = route_for_request(settings, &request);
        let secret = credentials
            .load("media_gateway", profile)
            .or_else(|_| credentials.load("media_gateway", "default"))?;
        let route = MediaGatewayRoute::new(endpoint, secret)
            .map_err(|_| CommandError::media_runtime_unavailable())?;
        self.execute_with_route(run_id, request, resume, route)
            .await
    }

    async fn execute_with_route(
        &self,
        run_id: String,
        request: MediaRequest,
        resume: bool,
        route: MediaGatewayRoute,
    ) -> Result<DesktopMediaRunResult, CommandError> {
        let client = MediaGatewayClient::new(Duration::from_secs(300))
            .map_err(|_| CommandError::media_runtime_unavailable())?;
        let artifacts = MediaArtifactStore::open(self.root.join("artifacts"))
            .map_err(|_| CommandError::media_runtime_unavailable())?;
        let events = MediaEventStore::open(self.root.join("events.jsonl"))
            .map_err(|_| CommandError::media_runtime_unavailable())?;
        let service = MediaRunService::new(
            MediaExecutor::new(GatewayMediaProvider::new(client, route), artifacts),
            events,
        );
        let (cancel, receiver) = watch::channel(false);
        {
            let mut active = self.active.lock().await;
            if active.is_some() {
                return Err(CommandError::media_run_active());
            }
            *active = Some((run_id.clone(), cancel));
        }
        let outcome = if resume {
            service.resume(&run_id, request, receiver).await
        } else {
            service.start(&run_id, request, receiver).await
        };
        self.active.lock().await.take();
        match outcome.map_err(|_| CommandError::media_run_failed())? {
            MediaRunOutcome::Completed(result) => Ok(DesktopMediaRunResult {
                schema: "desktop-media-run-result/v1",
                run_id,
                status: "completed",
                result: Some(*result),
            }),
            MediaRunOutcome::Cancelled => Ok(DesktopMediaRunResult {
                schema: "desktop-media-run-result/v1",
                run_id,
                status: "cancelled",
                result: None,
            }),
        }
    }

    pub async fn cancel(&self, run_id: &str) -> Result<(), CommandError> {
        let active = self.active.lock().await;
        let Some((active_id, sender)) = active.as_ref() else {
            return Err(CommandError::media_run_missing());
        };
        if active_id != run_id {
            return Err(CommandError::media_run_missing());
        }
        sender
            .send(true)
            .map_err(|_| CommandError::media_run_failed())
    }

    pub async fn edit_timeline(
        &self,
        request: DesktopTimelineRequest,
    ) -> Result<MediaArtifactRef, CommandError> {
        request.validate()?;
        let store = MediaArtifactStore::open(self.root.join("artifacts"))
            .map_err(|_| CommandError::media_runtime_unavailable())?;
        let temporary = self.root.join("edit-tmp").join(&request.request_id);
        std::fs::create_dir(&temporary).map_err(|_| CommandError::media_run_failed())?;
        let result = self.edit_timeline_inner(&request, &store, &temporary).await;
        cleanup_edit_directory(&temporary, request.clips.len());
        result
    }

    async fn edit_timeline_inner(
        &self,
        request: &DesktopTimelineRequest,
        store: &MediaArtifactStore,
        temporary: &std::path::Path,
    ) -> Result<MediaArtifactRef, CommandError> {
        let mut clips = Vec::with_capacity(request.clips.len());
        for (index, clip) in request.clips.iter().enumerate() {
            let (reference, bytes) = store.load_project_artifact(&request.project_id, &clip.content_ref)
                .map_err(|_| CommandError::invalid_media_project())?;
            if reference.kind != MediaKind::Video || reference.mime_type != "video/mp4" {
                return Err(CommandError::invalid_media_project());
            }
            let path = temporary.join(format!("input-{index}.mp4"));
            std::fs::write(&path, bytes).map_err(|_| CommandError::media_run_failed())?;
            clips.push(TimelineClip { input: path.to_string_lossy().into_owned(),
                start_seconds: clip.start_seconds, end_seconds: clip.end_seconds });
        }
        let tools = self.root.join("tools");
        let manifest_text = std::fs::read_to_string(tools.join("media-tool-manifest.json"))
            .map_err(|_| CommandError::media_runtime_unavailable())?;
        let manifest = MediaToolManifest::parse(&manifest_text)
            .map_err(|_| CommandError::media_runtime_unavailable())?;
        let receipt = execute_timeline(&manifest, &tools, &clips, &temporary.join("output.mp4"),
            Duration::from_secs(300)).await.map_err(|_| CommandError::media_run_failed())?;
        retain_timeline_output(store, &request.project_id, &request.request_id, receipt)
            .map_err(|_| CommandError::media_run_failed())
    }
}

fn cleanup_edit_directory(directory: &std::path::Path, clip_count: usize) {
    for index in 0..clip_count { let _ = std::fs::remove_file(directory.join(format!("input-{index}.mp4"))); }
    let _ = std::fs::remove_file(directory.join("output.mp4"));
    let _ = std::fs::remove_dir(directory);
}

fn route_for_request(
    settings: crate::media_gateway_settings::MediaGatewaySettings,
    request: &MediaRequest,
) -> (String, &'static str) {
    match request {
        MediaRequest::Video(value) if value.generation_tier.as_deref() == Some("fine") => {
            (settings.fine_endpoint.unwrap_or(settings.endpoint), "fine")
        }
        MediaRequest::Video(_) => (
            settings.coarse_endpoint.unwrap_or(settings.endpoint),
            "coarse",
        ),
        MediaRequest::Image(_) => (settings.endpoint, "default"),
    }
}

fn parse_request(value: Value) -> Result<MediaRequest, CommandError> {
    match value["schema"].as_str() {
        Some("image-generation-request/v1") => {
            serde_json::from_value::<ImageGenerationRequest>(value)
                .map(MediaRequest::Image)
                .map_err(|_| CommandError::invalid_media_project())
        }
        Some("video-generation-request/v1") => {
            serde_json::from_value::<VideoGenerationRequest>(value)
                .map(MediaRequest::Video)
                .map_err(|_| CommandError::invalid_media_project())
        }
        _ => Err(CommandError::invalid_media_project()),
    }
}

fn request_identity(request: &MediaRequest) -> (&str, &str) {
    match request {
        MediaRequest::Image(value) => (&value.project_id, &value.request_id),
        MediaRequest::Video(value) => (&value.project_id, &value.request_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_projects::DesktopMediaProjects;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};
    use serde_json::json;
    use story_provider::ProviderSecret;

    fn image_request() -> Value {
        json!({
            "schema":"image-generation-request/v1",
            "request_id":format!("img_{}", "a".repeat(32)),
            "project_id":"project_1",
            "prompt_revision_id":"prompt_1",
            "prompt":"雨夜站台",
            "source_spans":["story-package/scene-1"]
        })
    }

    #[test]
    fn generation_tier_selects_the_expected_trusted_route() {
        let settings = crate::media_gateway_settings::MediaGatewaySettings {
            schema: "desktop-media-gateway-settings/v1".into(),
            endpoint: "https://media.example/default".into(),
            coarse_endpoint: Some("https://media.example/wan".into()),
            fine_endpoint: Some("https://media.example/kling".into()),
        };
        let mut value = json!({
            "schema":"video-generation-request/v1",
            "request_id":format!("vid_{}", "f".repeat(32)), "project_id":"project_1",
            "image_artifact_ref":format!("artifact://sha256/{}", "a".repeat(64)),
            "story_spans":["story-package/scene-1"], "prompt":"精生成",
            "generation_tier":"fine"
        });
        let fine = parse_request(value.clone()).unwrap();
        assert_eq!(
            route_for_request(settings.clone(), &fine),
            ("https://media.example/kling".into(), "fine")
        );
        value["generation_tier"] = json!("coarse");
        let coarse = parse_request(value).unwrap();
        assert_eq!(
            route_for_request(settings, &coarse),
            ("https://media.example/wan".into(), "coarse")
        );
    }

    #[test]
    fn timeline_request_accepts_only_artifact_refs_and_bounded_ranges() {
        let valid: DesktopTimelineRequest = serde_json::from_value(json!({
            "schema":"desktop-media-timeline-request/v1", "project_id":"project_1",
            "request_id":format!("edit_{}", "a".repeat(32)),
            "clips":[{"content_ref":format!("artifact://sha256/{}", "b".repeat(64)),
                "start_seconds":0.0, "end_seconds":3.0}]
        })).unwrap();
        assert!(valid.validate().is_ok());
        let mut unsafe_request = valid.clone();
        unsafe_request.clips[0].content_ref = "C:/video.mp4".into();
        assert!(unsafe_request.validate().is_err());
        unsafe_request = valid;
        unsafe_request.clips[0].end_seconds = 301.0;
        assert!(unsafe_request.validate().is_err());
    }

    #[tokio::test]
    async fn unpersisted_request_fails_before_gateway_configuration() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = DesktopMediaRuntime::new(directory.path().join("runtime"));
        let settings = MediaGatewaySettingsService::new(directory.path().join("settings")).unwrap();
        let projects = DesktopMediaProjects::open(directory.path().join("projects")).unwrap();
        let credentials = CredentialService::new();
        assert!(runtime
            .start(
                &credentials,
                &settings,
                &projects,
                "media_run_1".into(),
                image_request(),
                false,
            )
            .await
            .is_err());
        assert!(!directory.path().join("runtime/events.jsonl").exists());
    }

    #[tokio::test]
    async fn cancellation_requires_matching_active_run() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = DesktopMediaRuntime::new(directory.path().into());
        assert!(runtime.cancel("missing").await.is_err());
        let (sender, _receiver) = watch::channel(false);
        *runtime.active.lock().await = Some(("media_run_1".into(), sender));
        assert!(runtime.cancel("media_run_other").await.is_err());
        assert!(runtime.cancel("media_run_1").await.is_ok());
    }

    async fn fake_gateway(
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Result<Json<Value>, StatusCode> {
        if headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            != Some("Bearer integration-secret")
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let (mime_type, content_base64, model) = match body["request"]["schema"].as_str() {
            Some("image-generation-request/v1") => {
                ("image/png", "Zml4dHVyZS1pbWFnZQ==", "fixture-image-v1")
            }
            Some("video-generation-request/v1") => {
                ("video/mp4", "Zml4dHVyZS12aWRlbw==", "fixture-video-v1")
            }
            _ => return Err(StatusCode::BAD_REQUEST),
        };
        Ok(Json(json!({
            "schema":"media-gateway-response/v1",
            "mime_type":mime_type,
            "content_base64":content_base64,
            "provider":"integration-gateway",
            "model":model,
            "cost_cny_fen":4,
            "pricing_catalog_id":"integration-pricing"
        })))
    }

    #[tokio::test]
    async fn desktop_runtime_reaches_http_gateway_and_retains_durable_artifact() {
        let directory = tempfile::tempdir().unwrap();
        let projects = DesktopMediaProjects::open(directory.path().join("projects")).unwrap();
        projects
            .append_prompt_revision(json!({
                "schema":"image-prompt-revision/v1", "project_id":"project_1",
                "revision_id":"prompt_1", "parent_revision_id":null, "prompt":"雨夜站台",
                "source_spans":["story-package/scene-1"]
            }))
            .unwrap();
        projects.append_generation_request(image_request()).unwrap();

        let app = Router::new().route("/v1/media/generate", post(fake_gateway));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let route = MediaGatewayRoute::new(
            format!("http://{address}/v1/media/generate"),
            ProviderSecret::new(b"integration-secret".to_vec()).unwrap(),
        )
        .unwrap();
        let runtime_root = directory.path().join("runtime");
        let runtime = DesktopMediaRuntime::new(runtime_root.clone());
        let result = runtime
            .execute_with_route(
                "media_run_integration".into(),
                parse_request(image_request()).unwrap(),
                false,
                route,
            )
            .await
            .unwrap();
        assert_eq!(result.status, "completed");
        let retained = result.result.unwrap();
        assert_eq!(retained.provider, "integration-gateway");
        MediaArtifactStore::open(runtime_root.join("artifacts"))
            .unwrap()
            .verify_project_image("project_1", &retained.content_ref)
            .unwrap();
        let events = MediaEventStore::open(runtime_root.join("events.jsonl"))
            .unwrap()
            .replay(0)
            .unwrap();
        assert_eq!(events.last().unwrap().event_type, "run.completed");

        let video_request = json!({
            "schema":"video-generation-request/v1",
            "request_id":format!("vid_{}", "b".repeat(32)),
            "project_id":"project_1",
            "image_artifact_ref":retained.content_ref,
            "story_spans":["story-package/scene-1"],
            "prompt":"镜头缓慢推近"
        });
        projects
            .append_generation_request(video_request.clone())
            .unwrap();
        let video_route = MediaGatewayRoute::new(
            format!("http://{address}/v1/media/generate"),
            ProviderSecret::new(b"integration-secret".to_vec()).unwrap(),
        )
        .unwrap();
        let video = runtime
            .execute_with_route(
                "media_run_video_integration".into(),
                parse_request(video_request).unwrap(),
                false,
                video_route,
            )
            .await
            .unwrap()
            .result
            .unwrap();
        assert_eq!(video.mime_type, "video/mp4");
        assert_eq!(video.model, "fixture-video-v1");
        let events = MediaEventStore::open(runtime_root.join("events.jsonl"))
            .unwrap()
            .replay(0)
            .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "run.completed")
                .count(),
            2
        );
    }
}
