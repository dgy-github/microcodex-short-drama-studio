mod artifacts;
mod commands;
mod credentials;
mod evaluations;
mod provider_settings;
mod provider_soak;
mod revisions;
mod run_controller;

use artifacts::{default_artifact_root, ArtifactRepository};
use credentials::CredentialService;
use evaluations::{default_evaluation_root, EvaluationService};
use provider_settings::{default_provider_settings_root, ProviderSettingsService};
use provider_soak::{default_provider_soak_root, ProviderSoakService};
use revisions::RevisionService;
use run_controller::{default_repository_root, DesktopRunController};
use serde::Serialize;
use story_runtime::GenrePackRegistry;

pub struct DesktopState {
    credentials: CredentialService,
    provider_settings: ProviderSettingsService,
    provider_soak: ProviderSoakService,
    artifacts: ArtifactRepository,
    genre_packs: GenrePackRegistry,
    controller: DesktopRunController,
    evaluations: EvaluationService,
    revisions: RevisionService,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    code: &'static str,
    message: &'static str,
}

impl CommandError {
    fn invalid_story_job() -> Self {
        Self::new("invalid_story_job", "故事任务不完整或格式无效。")
    }

    fn invalid_genre_pack() -> Self {
        Self::new("invalid_genre_pack", "类型包、集数约束或题材标签不匹配。")
    }

    fn invalid_provider() -> Self {
        Self::new("invalid_provider", "模型供应商或配置名称无效。")
    }

    fn invalid_secret() -> Self {
        Self::new("invalid_secret", "凭据不能为空或超过长度限制。")
    }

    fn credential_unavailable() -> Self {
        Self::new("credential_unavailable", "Windows 凭据存储当前不可用。")
    }

    fn credential_missing() -> Self {
        Self::new("credential_missing", "请先配置 DeepSeek 和阿里云百炼凭据。")
    }

    fn credential_audit_unavailable() -> Self {
        Self::new("credential_audit_unavailable", "凭据审计记录当前不可用。")
    }

    fn provider_health_failed() -> Self {
        Self::new(
            "provider_health_failed",
            "模型健康检查失败；请检查凭据、余额、网络和模型权限。",
        )
    }

    fn invalid_provider_route() -> Self {
        Self::new(
            "invalid_provider_route",
            "模型地址必须是 HTTPS chat/completions 接口，且模型 ID 不能为空。",
        )
    }

    fn provider_settings_unavailable() -> Self {
        Self::new(
            "provider_settings_unavailable",
            "模型地址配置当前不可读取或保存。",
        )
    }

    fn invalid_provider_soak() -> Self {
        Self::new(
            "invalid_provider_soak",
            "稳定性检查必须为每个供应商执行 3 至 20 次。",
        )
    }

    fn provider_soak_active() -> Self {
        Self::new("provider_soak_active", "已有模型稳定性检查正在运行。")
    }

    fn provider_soak_failed() -> Self {
        Self::new(
            "provider_soak_failed",
            "无法启动模型稳定性检查，请检查本地网络环境。",
        )
    }

    fn provider_soak_unavailable() -> Self {
        Self::new(
            "provider_soak_unavailable",
            "模型稳定性检查证据当前不可保存。",
        )
    }

    fn invalid_run_id() -> Self {
        Self::new("invalid_run_id", "故事运行标识无效。")
    }

    fn artifact_missing() -> Self {
        Self::new("artifact_missing", "未找到该故事包。")
    }

    fn artifact_invalid() -> Self {
        Self::new("artifact_invalid", "故事包不符合当前契约。")
    }

    fn artifact_unavailable() -> Self {
        Self::new("artifact_unavailable", "本地作品库当前不可用。")
    }

    fn run_active() -> Self {
        Self::new("run_active", "已有故事任务正在运行。")
    }

    fn run_missing() -> Self {
        Self::new("run_missing", "当前没有可控制的故事任务。")
    }

    fn runtime_unavailable() -> Self {
        Self::new("runtime_unavailable", "本地故事运行环境当前不可用。")
    }

    fn run_start_failed() -> Self {
        Self::new("run_start_failed", "故事任务启动失败。")
    }

    fn event_sync_failed() -> Self {
        Self::new("event_sync_failed", "运行事件同步失败，可稍后重试。")
    }

    fn run_cancel_failed() -> Self {
        Self::new("run_cancel_failed", "故事任务取消失败。")
    }

    fn invalid_revision() -> Self {
        Self::new("invalid_revision", "修订内容、引用位置或版本标识无效。")
    }

    fn revision_unavailable() -> Self {
        Self::new("revision_unavailable", "本地修订历史当前不可用。")
    }

    fn revision_limit() -> Self {
        Self::new("revision_limit", "D3/D4 两轮修订已用完，需要明确人工输入。")
    }

    fn span_missing() -> Self {
        Self::new("span_missing", "引用位置在当前版本中不存在。")
    }

    fn approval_final() -> Self {
        Self::new("approval_final", "该版本已经完成审批，审批记录不可改写。")
    }

    fn revision_not_approved() -> Self {
        Self::new("revision_not_approved", "只有明确批准的版本可以导出。")
    }

    fn invalid_export() -> Self {
        Self::new("invalid_export", "导出路径必须是尚不存在的绝对 JSON 文件。")
    }

    fn invalid_evaluation() -> Self {
        Self::new("invalid_evaluation", "评测集、用例或评测参数无效。")
    }

    fn evaluation_unavailable() -> Self {
        Self::new(
            "evaluation_unavailable",
            "本地评测资产或结果存储当前不可用。",
        )
    }

    fn evaluation_active() -> Self {
        Self::new("evaluation_active", "已有自动评测批次正在运行。")
    }

    fn evaluation_failed() -> Self {
        Self::new(
            "evaluation_failed",
            "自动评测失败，请检查模型配置和评测产物。",
        )
    }

    fn evaluation_case_ineligible() -> Self {
        Self::new(
            "evaluation_case_ineligible",
            "所选用例缺少可评故事包或未通过准入检查。",
        )
    }

    fn invalid_evaluation_score() -> Self {
        Self::new(
            "invalid_evaluation_score",
            "评分必须覆盖全部维度并包含有效理由和故事位置。",
        )
    }

    fn evaluation_already_submitted() -> Self {
        Self::new(
            "evaluation_already_submitted",
            "该盲测任务已经提交，原记录不可覆盖。",
        )
    }

    const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
}

pub fn run() {
    let repository_root = default_repository_root();
    let artifact_root = default_artifact_root();
    let genre_packs = GenrePackRegistry::load(&repository_root)
        .expect("genre pack registry configuration failed");
    let revisions =
        RevisionService::new(&repository_root).expect("revision repository configuration failed");
    let evaluations = EvaluationService::new(
        repository_root.clone(),
        artifact_root.clone(),
        default_evaluation_root(),
    )
    .expect("evaluation service configuration failed");
    let state = DesktopState {
        credentials: CredentialService::new(),
        provider_settings: ProviderSettingsService::new(default_provider_settings_root())
            .expect("provider settings configuration failed"),
        provider_soak: ProviderSoakService::new(default_provider_soak_root())
            .expect("provider soak storage configuration failed"),
        artifacts: ArtifactRepository::new(artifact_root),
        genre_packs,
        controller: DesktopRunController::new(repository_root),
        evaluations,
        revisions,
    };
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::validate_story_job,
            commands::list_genre_packs,
            commands::credential_status,
            commands::store_provider_credential,
            commands::delete_provider_credential,
            commands::credential_audit,
            commands::provider_route,
            commands::save_provider_route,
            commands::check_provider_health,
            commands::run_provider_soak,
            commands::list_story_runs,
            commands::read_story_run,
            commands::start_story_run,
            commands::sync_story_run,
            commands::cancel_story_run,
            commands::open_revision_workspace,
            commands::read_revision_span,
            commands::create_story_revision,
            commands::approve_story_revision,
            commands::compare_story_revisions,
            commands::rollback_story_revision,
            commands::export_story_revision,
            commands::evaluation_catalog,
            commands::run_automatic_evaluation,
            commands::create_blind_assignments,
            commands::submit_blind_review
        ])
        .run(tauri::generate_context!())
        .expect("desktop runtime failed");
}
