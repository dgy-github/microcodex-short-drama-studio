import { invoke } from "@tauri-apps/api/core";
import type {
  CredentialStatus,
  CredentialAuditEvent,
  BlindAssignment,
  EvaluationBatchResult,
  EvaluationCatalog,
  EvaluationScoreRecord,
  HumanDimensionInput,
  GenrePackOption,
  ProviderHealth,
  ProviderRouteSettings,
  ProviderSoakResult,
  ExportReceipt,
  RevisionComparison,
  RevisionSummary,
  RevisionWorkspace,
  RunSummary,
  RunSnapshot,
  StoryJob,
  StoryJobPreview,
  WorkflowResult,
  MediaGatewaySettings,
  MediaProjectRecord,
  ImagePromptRevision,
  DesktopMediaRunResult,
  DesktopTimelineRequest,
} from "./types";

export const desktopApi = {
  listGenrePacks: () => invoke<GenrePackOption[]>("list_genre_packs"),
  validateStoryJob: (job: StoryJob) =>
    invoke<StoryJobPreview>("validate_story_job", { job }),
  credentialStatus: (provider: CredentialStatus["provider"]) =>
    invoke<CredentialStatus>("credential_status", {
      provider,
      profile: "default",
    }),
  storeCredential: (
    provider: CredentialStatus["provider"],
    secret: string,
  ) =>
    invoke<CredentialStatus>("store_provider_credential", {
      provider,
      profile: "default",
      secret,
    }),
  deleteCredential: (provider: CredentialStatus["provider"]) =>
    invoke<CredentialStatus>("delete_provider_credential", {
      provider,
      profile: "default",
    }),
  credentialAudit: () => invoke<CredentialAuditEvent[]>("credential_audit"),
  providerRoute: (provider: CredentialStatus["provider"]) =>
    invoke<ProviderRouteSettings>("provider_route", { provider }),
  saveProviderRoute: (
    provider: CredentialStatus["provider"],
    endpoint: string,
    model: string,
  ) =>
    invoke<ProviderRouteSettings>("save_provider_route", {
      provider,
      endpoint,
      model,
    }),
  checkProviderHealth: (provider: CredentialStatus["provider"]) =>
    invoke<ProviderHealth>("check_provider_health", { provider }),
  runProviderSoak: (iterations: number) =>
    invoke<ProviderSoakResult>("run_provider_soak", { iterations }),
  listRuns: () => invoke<RunSummary[]>("list_story_runs"),
  readRun: (runId: string) =>
    invoke<WorkflowResult>("read_story_run", { runId }),
  startRun: (job: StoryJob) =>
    invoke<RunSnapshot>("start_story_run", { job }),
  syncRun: () => invoke<RunSnapshot>("sync_story_run"),
  cancelRun: () => invoke<RunSnapshot>("cancel_story_run"),
  openRevisionWorkspace: (runId: string) =>
    invoke<RevisionWorkspace>("open_revision_workspace", { runId }),
  readRevisionSpan: (revisionId: string, span: string) =>
    invoke<unknown>("read_revision_span", { revisionId, span }),
  createRevision: (
    baseRevisionId: string,
    span: string,
    replacement: unknown,
    requestedChange: string,
  ) =>
    invoke<RevisionSummary>("create_story_revision", {
      baseRevisionId,
      span,
      replacement,
      requestedChange,
    }),
  approveRevision: (
    revisionId: string,
    decision: "approved" | "rejected",
    actor: string,
    note: string,
  ) =>
    invoke<RevisionSummary>("approve_story_revision", {
      revisionId,
      decision,
      actor,
      note,
    }),
  compareRevisions: (fromRevisionId: string, toRevisionId: string) =>
    invoke<RevisionComparison>("compare_story_revisions", {
      fromRevisionId,
      toRevisionId,
    }),
  rollbackRevision: (
    currentRevisionId: string,
    targetRevisionId: string,
    requestedChange: string,
  ) =>
    invoke<RevisionSummary>("rollback_story_revision", {
      currentRevisionId,
      targetRevisionId,
      requestedChange,
    }),
  exportRevision: (revisionId: string, targetPath: string) =>
    invoke<ExportReceipt>("export_story_revision", {
      revisionId,
      targetPath,
    }),
  evaluationCatalog: () =>
    invoke<EvaluationCatalog>("evaluation_catalog"),
  runAutomaticEvaluation: (
    datasetId: EvaluationCatalog["datasets"][number]["dataset_id"],
    caseIds: string[],
  ) =>
    invoke<EvaluationBatchResult>("run_automatic_evaluation", {
      datasetId,
      caseIds,
    }),
  createBlindAssignments: (
    datasetId: EvaluationCatalog["datasets"][number]["dataset_id"],
    caseIds: string[],
    raterId: string,
  ) =>
    invoke<BlindAssignment[]>("create_blind_assignments", {
      datasetId,
      caseIds,
      raterId,
    }),
  submitBlindReview: (
    assignmentId: string,
    raterId: string,
    dimensions: HumanDimensionInput[],
  ) =>
    invoke<EvaluationScoreRecord>("submit_blind_review", {
      assignmentId,
      raterId,
      dimensions,
    }),
  mediaGatewaySettings: () =>
    invoke<MediaGatewaySettings | null>("media_gateway_settings"),
  saveMediaGatewaySettings: (endpoint: string) =>
    invoke<MediaGatewaySettings>("save_media_gateway_settings", { endpoint }),
  saveMediaGenerationRoutes: (coarseEndpoint: string, fineEndpoint: string) =>
    invoke<MediaGatewaySettings>("save_media_generation_routes", {
      coarseEndpoint, fineEndpoint,
    }),
  storeMediaGatewayCredential: (secret: string, profile = "default") =>
    invoke<CredentialStatus>("store_provider_credential", {
      provider: "media_gateway", profile, secret,
    }),
  appendMediaPromptRevision: (revision: ImagePromptRevision) =>
    invoke<MediaProjectRecord>("append_media_prompt_revision", { revision }),
  appendMediaGenerationRequest: (request: Record<string, unknown>) =>
    invoke<MediaProjectRecord>("append_media_generation_request", { request }),
  readMediaProjectHistory: (projectId: string) =>
    invoke<MediaProjectRecord[]>("read_media_project_history", { projectId }),
  startMediaRun: (runId: string, request: Record<string, unknown>) =>
    invoke<DesktopMediaRunResult>("start_media_run", { runId, request }),
  resumeMediaRun: (runId: string, request: Record<string, unknown>) =>
    invoke<DesktopMediaRunResult>("resume_media_run", { runId, request }),
  cancelMediaRun: (runId: string) =>
    invoke<void>("cancel_media_run", { runId }),
  validateMediaTimelineRequest: (request: DesktopTimelineRequest) =>
    invoke<void>("validate_media_timeline_request", { request }),
};

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const value = error as { message?: unknown };
    if (typeof value.message === "string") return value.message;
  }
  return "操作失败，请检查本地配置后重试。";
}
