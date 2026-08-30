use serde_json::Value;
use story_media::{ImageGenerationRequest, VideoGenerationRequest};
use story_storage::media_projects::{
    ImagePromptRevision, MediaProjectRecord, MediaProjectRepository,
};

use crate::CommandError;

pub struct DesktopMediaProjects {
    repository: MediaProjectRepository,
}

impl DesktopMediaProjects {
    pub fn open(root: impl Into<std::path::PathBuf>) -> Result<Self, CommandError> {
        Ok(Self {
            repository: MediaProjectRepository::open(root.into())
                .map_err(|_| CommandError::media_project_unavailable())?,
        })
    }

    pub fn append_prompt_revision(&self, value: Value) -> Result<MediaProjectRecord, CommandError> {
        let revision: ImagePromptRevision =
            serde_json::from_value(value).map_err(|_| CommandError::invalid_media_project())?;
        self.repository
            .append_prompt_revision(&revision)
            .map_err(map_repository_error)
    }

    pub fn append_generation_request(
        &self,
        value: Value,
    ) -> Result<MediaProjectRecord, CommandError> {
        let (project_id, request_id) = match value["schema"].as_str() {
            Some("image-generation-request/v1") => {
                let request: ImageGenerationRequest = serde_json::from_value(value.clone())
                    .map_err(|_| CommandError::invalid_media_project())?;
                (request.project_id, request.request_id)
            }
            Some("video-generation-request/v1") => {
                let request: VideoGenerationRequest = serde_json::from_value(value.clone())
                    .map_err(|_| CommandError::invalid_media_project())?;
                (request.project_id, request.request_id)
            }
            _ => return Err(CommandError::invalid_media_project()),
        };
        self.repository
            .append_generation_request(&project_id, &request_id, &value)
            .map_err(map_repository_error)
    }

    pub fn history(&self, project_id: &str) -> Result<Vec<MediaProjectRecord>, CommandError> {
        self.repository
            .history(project_id)
            .map_err(map_repository_error)
    }
}

fn map_repository_error(_: story_storage::media_projects::MediaProjectError) -> CommandError {
    CommandError::invalid_media_project()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn desktop_media_history_uses_shared_types_and_append_only_storage() {
        let directory = tempfile::tempdir().unwrap();
        let service = DesktopMediaProjects::open(directory.path()).unwrap();
        service
            .append_prompt_revision(json!({
                "schema":"image-prompt-revision/v1",
                "project_id":"project_1",
                "revision_id":"prompt_1",
                "parent_revision_id":null,
                "prompt":"雨夜车站，人物回头",
                "source_spans":["story-package/scene-1"]
            }))
            .unwrap();
        service
            .append_generation_request(json!({
                "schema":"image-generation-request/v1",
                "request_id":format!("img_{}", "a".repeat(32)),
                "project_id":"project_1",
                "prompt_revision_id":"prompt_1",
                "prompt":"雨夜车站，人物回头",
                "source_spans":["story-package/scene-1"]
            }))
            .unwrap();
        assert_eq!(service.history("project_1").unwrap().len(), 2);
    }

    #[test]
    fn desktop_rejects_untyped_media_requests() {
        let directory = tempfile::tempdir().unwrap();
        let service = DesktopMediaProjects::open(directory.path()).unwrap();
        assert!(service
            .append_generation_request(json!({"schema":"unknown/v1"}))
            .is_err());
    }
}
