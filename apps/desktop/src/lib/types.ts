export type CredentialStatus = {
  schema: "desktop-credential-status/v1";
  provider: "deepseek" | "aliyun_bailian" | "media_gateway";
  profile: "default";
  configured: boolean;
};

export type ChatProvider = "deepseek" | "aliyun_bailian";

export type CredentialAuditEvent = {
  schema: "credential-audit-event/v1";
  sequence: number;
  occurred_at_unix_seconds: number;
  provider: CredentialStatus["provider"];
  profile: "default";
  action: "configured" | "rotated" | "deleted";
  previous_hash: string;
  event_hash: string;
};

export type ProviderHealth = {
  schema: "provider-health/v1";
  provider: ChatProvider;
  status: "ready";
  model: string;
};

export type ProviderRouteSettings = {
  schema: "desktop-provider-route/v1";
  provider: ChatProvider;
  profile: "default";
  endpoint: string;
  model: string;
  thinking_disabled: boolean;
  source: "default" | "user";
  record_id: string | null;
  updated_at_unix_ms: number | null;
};

export type ProviderSoakResult = {
  schema: "provider-soak-result/v1";
  soak_id: string;
  iterations_per_provider: number;
  status: "ready" | "degraded";
  started_at_unix_ms: number;
  finished_at_unix_ms: number;
  providers: Array<{
    provider: ChatProvider;
    model: string;
    route_fingerprint: string;
    status: "ready" | "degraded";
    successful_requests: number;
    failed_requests: number;
    min_latency_ms: number;
    average_latency_ms: number;
    max_latency_ms: number;
  }>;
};

export type StoryJob = {
  schema: "story-job/v1";
  job_id: string;
  content_form: "scripted_short_drama";
  input: string;
  genre_mode: "auto" | "fixed";
  allowed_genres: string[];
  genre_pack_id?: string | null;
  constraint_profile_id?: string | null;
  audience: string;
  format: { episodes: number; minutes_per_episode: number };
  content_limits: string[];
  budget: {
    max_tokens: number;
    max_cny_fen: number;
    deadline_seconds: number;
  };
};

export type StoryJobPreview = {
  job_id: string;
  content_form: "scripted_short_drama";
  episodes: number;
  minutes_per_episode: number;
};

export type GenrePackOption = {
  pack_id: string;
  display_name: string;
  genre: string;
  default_audience: string;
};

export type RunSummary = {
  schema: "desktop-run-summary/v1";
  run_id: string;
  job_id: string;
  status: "advisory";
  promotion: "non-promotable";
  generation_model: string;
  review_model: string;
  task_count: number;
  review_count: number;
  episode_count: number;
  logline: string;
  completed_at_unix_ms: number;
};

export type WorkflowResult = {
  schema: "story-workflow-result/v1";
  run_id: string;
  job_id: string;
  status: "advisory";
  promotion: "non-promotable";
  package: {
    package_id: string;
    logline?: { text?: string };
    promise?: {
      genre?: string;
      audience?: string;
      tone?: string;
    };
    episodes?: StoryEpisode[];
    scenes?: StoryScene[];
    characters?: StoryCharacter[];
  };
  reviews: Array<{
    task_id: string;
    review_type: string;
    status: string;
    summary: string;
    findings: ReviewFinding[];
  }>;
};

export type StoryCharacter = {
  node_id?: string;
  name?: string;
  desire?: string;
  fear?: string;
  contradiction?: string;
  secret?: string;
  change?: string;
  voice_markers?: string[];
};

export type StoryEpisode = {
  node_id?: string;
  index?: number;
  opening_state?: string;
  conflict?: string;
  turn?: string;
  end_hook?: {
    text?: string;
    kind?: string;
    consequence_in?: string;
  };
};

export type StoryScene = {
  node_id?: string;
  episode_ref?: string;
  location?: string;
  lines?: Array<{
    node_id?: string;
    kind?: "action" | "dialogue";
    speaker?: string;
    text?: string;
    subtext?: string | null;
  }>;
};

export type ReviewFinding = {
  defect_id: string;
  severity: "critical" | "major" | "minor" | "note";
  span_ref: string;
  evidence: string;
  requested_change: string;
};

export type CommandError = {
  code?: string;
  message?: string;
};

export type StoryEvent = {
  protocol: "story-agent-event/v1";
  event_id: string;
  seq: number;
  occurred_at: string;
  job_id: string;
  run_id: string;
  task_id: string | null;
  agent_id: string | null;
  event_type: string;
  payload: unknown;
};

export type RunSnapshot = {
  schema: "desktop-run-snapshot/v1";
  run_id: string;
  job_id: string;
  status: "accepted" | "running" | "completed" | "failed" | "cancelled";
  last_event_id: number;
  tasks_total: 17;
  tasks_queued: number;
  tasks_started: number;
  tasks_completed: number;
  reviews_completed: number;
  approvals_pending: number;
  error: string | null;
  budget: {
    max_tokens: number;
    consumed_tokens: number;
    max_cny_fen: number;
    consumed_cny_fen: number | null;
  };
  events: StoryEvent[];
};

export type RevisionRecord = {
  schema: "story-revision-record/v1";
  revision_id: string;
  job_id: string;
  package_id: string;
  supersedes_package_id: string | null;
  kind: "origin" | "targeted" | "rollback";
  round: number;
  source_run_id: string;
  target_span: string | null;
  requested_change: string;
  content_sha256: string;
  created_at_unix_ms: number;
  node_correspondence_count: number;
};

export type ApprovalEvent = {
  schema: "story-approval-event/v1";
  approval_id: string;
  revision_id: string;
  decision: "approved" | "rejected";
  actor: string;
  note: string;
  occurred_at_unix_ms: number;
};

export type RevisionSummary = {
  schema: "desktop-revision-summary/v1";
  record: RevisionRecord;
  approval: ApprovalEvent | null;
};

export type RevisionWorkspace = {
  run_id: string;
  job_id: string;
  revisions: RevisionSummary[];
  findings: ReviewFinding[];
};

export type RevisionComparison = {
  from_revision_id: string;
  to_revision_id: string;
  changed_spans: string[];
  removed_spans: string[];
  added_spans: string[];
};

export type ExportReceipt = {
  revision_id: string;
  target_path: string;
  status: "exported";
};

export type EvaluationCase = {
  case_id: string;
  label: string;
  genre: string;
  difficulty: string | null;
  split: string | null;
  eligible: boolean;
};

export type EvaluationDataset = {
  dataset_id: "offline-v0.1.0" | "online-local";
  kind: "offline" | "online";
  label: string;
  case_count: number;
  eligible_count: number;
  cases: EvaluationCase[];
};

export type EvaluationCatalog = {
  schema: "desktop-evaluation-catalog/v1";
  datasets: EvaluationDataset[];
};

export type EvaluationScoreRecord = {
  schema: "eval-score-record/v1";
  record_id: string;
  case_id: string;
  rater: {
    rater_id: string;
    rater_type: "llm_judge" | "internal_spot_check";
    model_id: string | null;
  };
  aggregate: {
    pillars: Record<string, number>;
    geometric_mean: number;
    legacy_weighted_sum: number | null;
    floors_passed: boolean;
    verdict: "reject" | "consider" | "pass";
  };
};

export type EvaluationBatchResult = {
  schema: "desktop-evaluation-batch-result/v1";
  batch_id: string;
  dataset_id: EvaluationDataset["dataset_id"];
  mode: "automatic";
  evidence_status: "partial_advisory";
  selected_count: number;
  completed_count: number;
  failed_count: number;
  results: Array<{
    case_id: string;
    status: "completed" | "failed";
    failed_gates: string[];
    score_record: EvaluationScoreRecord | null;
  }>;
  occurred_at_unix_ms: number;
};

export type BlindDimension = {
  dimension_id: string;
  name: string;
  ask: string;
  anchors: Record<"1" | "3" | "5", string>;
};

export type BlindAssignment = {
  schema: "desktop-blind-assignment/v1";
  assignment_id: string;
  alias: string;
  prompt: string;
  constraints: Record<string, unknown>;
  artifact: Record<string, unknown>;
  dimensions: BlindDimension[];
  allowed_spans: string[];
};

export type HumanDimensionInput = {
  dimension_id: string;
  score: number;
  reason: string;
  span_refs: string[];
};

export type MediaGatewaySettings = {
  schema: "desktop-media-gateway-settings/v1";
  endpoint: string;
  coarse_endpoint?: string | null;
  fine_endpoint?: string | null;
};

export type MediaProjectRecord = {
  schema: "media-project-record/v1";
  seq: number;
  project_id: string;
  record_id: string;
  record_type: "image_prompt_revision" | "generation_request";
  data: Record<string, unknown>;
};

export type ImagePromptRevision = {
  schema: "image-prompt-revision/v1";
  project_id: string;
  revision_id: string;
  parent_revision_id: string | null;
  prompt: string;
  source_spans: string[];
};

export type MediaGenerationResult = {
  schema: "media-generation-result/v1";
  project_id: string;
  request_id: string;
  kind: "Image" | "Video";
  mime_type: string;
  content_ref: string;
  content_sha256: string;
  byte_len: number;
  provider: string;
  model: string;
  cost_cny_fen: number;
  pricing_catalog_id: string;
};

export type DesktopTimelineRequest = {
  schema: "desktop-media-timeline-request/v1";
  project_id: string;
  request_id: string;
  clips: Array<{
    content_ref: string;
    start_seconds: number;
    end_seconds: number;
  }>;
};

export type MediaArtifactRef = {
  schema: "media-artifact-ref/v1";
  project_id: string;
  request_id: string;
  kind: "image" | "video";
  mime_type: string;
  content_ref: string;
  content_sha256: string;
  byte_len: number;
};

export type DesktopMediaRunResult = {
  schema: "desktop-media-run-result/v1";
  run_id: string;
  status: "completed" | "cancelled";
  result: MediaGenerationResult | null;
};

export type DesktopMediaToolStatus = {
  schema: "desktop-media-tool-status/v1";
  tools: Array<{ id: string; version: string; status: "ready" | "missing" | "hash_mismatch" | "invalid_root" }>;
};
