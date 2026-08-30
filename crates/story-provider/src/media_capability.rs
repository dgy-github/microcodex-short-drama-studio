use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use story_storage::media_projects::{
    ImagePromptRevision, MediaProjectError, MediaProjectRepository,
};

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
pub(crate) enum Request {
    #[serde(rename = "append_prompt_revision")]
    AppendPromptRevision { schema: String, record: Value },
    #[serde(rename = "append_generation_request")]
    AppendGenerationRequest { schema: String, record: Value },
}

#[derive(Serialize)]
pub(crate) struct Response {
    pub(crate) schema: &'static str,
    pub(crate) status: &'static str,
}

pub(crate) fn append(
    repository: &MediaProjectRepository,
    request: Request,
) -> Result<(), StatusCode> {
    match request {
        Request::AppendPromptRevision { schema, record } => {
            require_schema(&schema)?;
            let revision: ImagePromptRevision =
                serde_json::from_value(record).map_err(|_| StatusCode::BAD_REQUEST)?;
            repository
                .append_prompt_revision(&revision)
                .map_err(map_error)?;
        }
        Request::AppendGenerationRequest { schema, record } => {
            require_schema(&schema)?;
            let project_id = record["project_id"]
                .as_str()
                .ok_or(StatusCode::BAD_REQUEST)?;
            let request_id = record["request_id"]
                .as_str()
                .ok_or(StatusCode::BAD_REQUEST)?;
            repository
                .append_generation_request(project_id, request_id, &record)
                .map_err(map_error)?;
        }
    }
    Ok(())
}

fn require_schema(schema: &str) -> Result<(), StatusCode> {
    (schema == "media-project-capability-request/v1")
        .then_some(())
        .ok_or(StatusCode::BAD_REQUEST)
}

fn map_error(error: MediaProjectError) -> StatusCode {
    match error {
        MediaProjectError::DuplicateRecord => StatusCode::CONFLICT,
        MediaProjectError::InvalidRecord | MediaProjectError::MissingParent => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        MediaProjectError::Corrupt
        | MediaProjectError::Lock
        | MediaProjectError::Io(_)
        | MediaProjectError::Encoding(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
