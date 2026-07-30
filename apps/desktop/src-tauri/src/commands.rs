use serde::Serialize;
use serde_json::Value;
use story_core::StoryJob;
use story_provider::OpenAiCompatibleProvider;
use tauri::State;

use crate::artifacts::RunSummary;
use crate::credentials::{CredentialAuditEvent, CredentialStatus};
use crate::evaluations::{
    BlindAssignment, EvaluationBatchResult, EvaluationCatalog, HumanDimensionInput,
};
use crate::provider_settings::ProviderRouteSettings;
use crate::provider_soak::ProviderSoakResult;
use crate::revisions::{ExportReceipt, RevisionWorkspace};
use crate::run_controller::RunSnapshot;
use crate::{CommandError, DesktopState};
use story_runtime::GenrePackOption;
use story_storage::{RevisionComparison, RevisionSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoryJobPreview {
    pub job_id: String,
    pub content_form: &'static str,
    pub episodes: u16,
    pub minutes_per_episode: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderHealth {
    pub schema: &'static str,
    pub provider: String,
    pub status: &'static str,
    pub model: String,
}

#[tauri::command]
pub fn validate_story_job(job: Value) -> Result<StoryJobPreview, CommandError> {
    let job: StoryJob =
        serde_json::from_value(job).map_err(|_| CommandError::invalid_story_job())?;
    job.validate()
        .map_err(|_| CommandError::invalid_story_job())?;
    Ok(StoryJobPreview {
        job_id: job.job_id,
        content_form: "scripted_short_drama",
        episodes: job.format.episodes,
        minutes_per_episode: job.format.minutes_per_episode,
    })
}

#[tauri::command]
pub fn list_genre_packs(
    state: State<'_, DesktopState>,
) -> Result<Vec<GenrePackOption>, CommandError> {
    Ok(state.genre_packs.options())
}

#[tauri::command]
pub fn credential_status(
    state: State<'_, DesktopState>,
    provider: String,
    profile: String,
) -> Result<CredentialStatus, CommandError> {
    state.credentials.status(&provider, &profile)
}

#[tauri::command]
pub fn store_provider_credential(
    state: State<'_, DesktopState>,
    provider: String,
    profile: String,
    secret: String,
) -> Result<CredentialStatus, CommandError> {
    state.credentials.store(&provider, &profile, secret)
}

#[tauri::command]
pub fn delete_provider_credential(
    state: State<'_, DesktopState>,
    provider: String,
    profile: String,
) -> Result<CredentialStatus, CommandError> {
    state.credentials.delete(&provider, &profile)
}

#[tauri::command]
pub fn credential_audit(
    state: State<'_, DesktopState>,
) -> Result<Vec<CredentialAuditEvent>, CommandError> {
    state.credentials.audit_events()
}

#[tauri::command]
pub fn provider_route(
    state: State<'_, DesktopState>,
    provider: String,
) -> Result<ProviderRouteSettings, CommandError> {
    state.provider_settings.load(&provider, "default")
}

#[tauri::command]
pub fn save_provider_route(
    state: State<'_, DesktopState>,
    provider: String,
    endpoint: String,
    model: String,
) -> Result<ProviderRouteSettings, CommandError> {
    state
        .provider_settings
        .save(&provider, "default", endpoint, model)
}

#[tauri::command]
pub async fn check_provider_health(
    state: State<'_, DesktopState>,
    provider: String,
) -> Result<ProviderHealth, CommandError> {
    let route = state
        .provider_settings
        .route(&state.credentials, &provider)?;
    let client = OpenAiCompatibleProvider::new(std::time::Duration::from_secs(30))
        .map_err(|_| CommandError::provider_health_failed())?;
    let output = client
        .generate_json(
            &route,
            "Return one JSON object only.",
            r#"Return exactly {"health":"ok"}."#,
        )
        .await
        .map_err(|_| CommandError::provider_health_failed())?;
    if output.artifact["health"] != "ok" {
        return Err(CommandError::provider_health_failed());
    }
    Ok(ProviderHealth {
        schema: "provider-health/v1",
        provider,
        status: "ready",
        model: output.model,
    })
}

#[tauri::command]
pub async fn run_provider_soak(
    state: State<'_, DesktopState>,
    iterations: u8,
) -> Result<ProviderSoakResult, CommandError> {
    state
        .provider_soak
        .run(&state.credentials, &state.provider_settings, iterations)
        .await
}

#[tauri::command]
pub fn list_story_runs(state: State<'_, DesktopState>) -> Result<Vec<RunSummary>, CommandError> {
    let runs = state.artifacts.list()?;
    #[cfg(debug_assertions)]
    eprintln!("desktop IPC ready: {} completed runs", runs.len());
    Ok(runs)
}

#[tauri::command]
pub fn read_story_run(
    state: State<'_, DesktopState>,
    run_id: String,
) -> Result<Value, CommandError> {
    state.artifacts.read(&run_id)
}

#[tauri::command]
pub async fn start_story_run(
    state: State<'_, DesktopState>,
    job: Value,
) -> Result<RunSnapshot, CommandError> {
    let job: StoryJob =
        serde_json::from_value(job).map_err(|_| CommandError::invalid_story_job())?;
    state
        .controller
        .start(
            &state.credentials,
            &state.provider_settings,
            &state.artifacts,
            job,
        )
        .await
}

#[tauri::command]
pub async fn sync_story_run(state: State<'_, DesktopState>) -> Result<RunSnapshot, CommandError> {
    state.controller.sync(&state.artifacts).await
}

#[tauri::command]
pub async fn cancel_story_run(state: State<'_, DesktopState>) -> Result<RunSnapshot, CommandError> {
    state.controller.cancel().await
}

#[tauri::command]
pub fn open_revision_workspace(
    state: State<'_, DesktopState>,
    run_id: String,
) -> Result<RevisionWorkspace, CommandError> {
    let workflow = state.artifacts.read(&run_id)?;
    state.revisions.open(&run_id, &workflow)
}

#[tauri::command]
pub fn read_revision_span(
    state: State<'_, DesktopState>,
    revision_id: String,
    span: String,
) -> Result<Value, CommandError> {
    state.revisions.read_span(&revision_id, &span)
}

#[tauri::command]
pub fn create_story_revision(
    state: State<'_, DesktopState>,
    base_revision_id: String,
    span: String,
    replacement: Value,
    requested_change: String,
) -> Result<RevisionSummary, CommandError> {
    state
        .revisions
        .create(&base_revision_id, &span, replacement, &requested_change)
}

#[tauri::command]
pub fn approve_story_revision(
    state: State<'_, DesktopState>,
    revision_id: String,
    decision: String,
    actor: String,
    note: String,
) -> Result<RevisionSummary, CommandError> {
    state
        .revisions
        .approve(&revision_id, &decision, &actor, &note)
}

#[tauri::command]
pub fn compare_story_revisions(
    state: State<'_, DesktopState>,
    from_revision_id: String,
    to_revision_id: String,
) -> Result<RevisionComparison, CommandError> {
    state.revisions.compare(&from_revision_id, &to_revision_id)
}

#[tauri::command]
pub fn rollback_story_revision(
    state: State<'_, DesktopState>,
    current_revision_id: String,
    target_revision_id: String,
    requested_change: String,
) -> Result<RevisionSummary, CommandError> {
    state
        .revisions
        .rollback(&current_revision_id, &target_revision_id, &requested_change)
}

#[tauri::command]
pub fn export_story_revision(
    state: State<'_, DesktopState>,
    revision_id: String,
    target_path: String,
) -> Result<ExportReceipt, CommandError> {
    state.revisions.export(&revision_id, &target_path)
}

#[tauri::command]
pub fn evaluation_catalog(
    state: State<'_, DesktopState>,
) -> Result<EvaluationCatalog, CommandError> {
    state.evaluations.catalog()
}

#[tauri::command]
pub async fn run_automatic_evaluation(
    state: State<'_, DesktopState>,
    dataset_id: String,
    case_ids: Vec<String>,
) -> Result<EvaluationBatchResult, CommandError> {
    state
        .evaluations
        .run_automatic(
            &state.credentials,
            &state.provider_settings,
            &dataset_id,
            &case_ids,
        )
        .await
}

#[tauri::command]
pub fn create_blind_assignments(
    state: State<'_, DesktopState>,
    dataset_id: String,
    case_ids: Vec<String>,
    rater_id: String,
) -> Result<Vec<BlindAssignment>, CommandError> {
    state
        .evaluations
        .create_blind_assignments(&dataset_id, &case_ids, &rater_id)
}

#[tauri::command]
pub fn submit_blind_review(
    state: State<'_, DesktopState>,
    assignment_id: String,
    rater_id: String,
    dimensions: Vec<HumanDimensionInput>,
) -> Result<Value, CommandError> {
    state
        .evaluations
        .submit_blind_review(&assignment_id, &rater_id, dimensions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_job_command_reuses_domain_validation() {
        let valid = serde_json::json!({
            "schema": "story-job/v1",
            "job_id": "job_desktop_1",
            "content_form": "scripted_short_drama",
            "input": "一名维修工必须在开门前救出被困电梯的人。",
            "genre_mode": "fixed",
            "allowed_genres": ["family"],
            "audience": "25-45",
            "format": {"episodes": 6, "minutes_per_episode": 2},
            "content_limits": [],
            "budget": {
                "max_tokens": 90000,
                "max_cny_fen": 1200,
                "deadline_seconds": 900
            }
        });
        assert_eq!(validate_story_job(valid).unwrap().episodes, 6);
        assert!(validate_story_job(serde_json::json!({})).is_err());
    }
}
