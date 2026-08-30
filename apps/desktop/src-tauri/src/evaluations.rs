use jsonschema::validator_for;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use story_provider::OpenAiCompatibleProvider;
use uuid::Uuid;

use crate::artifacts::ArtifactRepository;
use crate::credentials::CredentialService;
use crate::provider_settings::ProviderSettingsService;
use crate::CommandError;

const OFFLINE_DATASET: &str = "offline-v0.1.0";
const ONLINE_DATASET: &str = "online-local";
const MAX_SELECTION: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvaluationCatalog {
    pub schema: &'static str,
    pub datasets: Vec<EvaluationDataset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvaluationDataset {
    pub dataset_id: String,
    pub kind: &'static str,
    pub label: &'static str,
    pub case_count: usize,
    pub eligible_count: usize,
    pub cases: Vec<EvaluationCaseSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvaluationCaseSummary {
    pub case_id: String,
    pub label: String,
    pub genre: String,
    pub difficulty: Option<String>,
    pub split: Option<String>,
    pub eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvaluationBatchResult {
    pub schema: &'static str,
    pub batch_id: String,
    pub dataset_id: String,
    pub mode: &'static str,
    pub evidence_status: &'static str,
    pub selected_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub results: Vec<EvaluationCaseResult>,
    pub occurred_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvaluationCaseResult {
    pub case_id: String,
    pub status: &'static str,
    pub failed_gates: Vec<String>,
    pub score_record: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlindDimension {
    #[serde(alias = "id")]
    pub dimension_id: String,
    pub name: String,
    pub ask: String,
    pub anchors: BTreeMap<u8, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlindAssignment {
    pub schema: String,
    pub assignment_id: String,
    pub alias: String,
    pub prompt: String,
    pub constraints: Value,
    pub artifact: Value,
    pub dimensions: Vec<BlindDimension>,
    pub allowed_spans: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HumanDimensionInput {
    pub dimension_id: String,
    pub score: f32,
    pub reason: String,
    pub span_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrivateBlindAssignment {
    schema: String,
    dataset_id: String,
    case_id: String,
    artifact_id: String,
    artifact_hash: String,
    rater_id: String,
    public: BlindAssignment,
}

#[derive(Debug, Clone)]
struct EvaluationSubject {
    case_id: String,
    label: String,
    genre: String,
    difficulty: Option<String>,
    split: Option<String>,
    prompt: String,
    constraints: Value,
    case: Option<Value>,
    artifact_id: Option<String>,
    artifact: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RubricDocument {
    version: String,
    dimensions: Vec<BlindDimension>,
}

#[derive(Debug, Deserialize)]
struct JudgeResponse {
    dimensions: Vec<JudgeDimension>,
}

#[derive(Debug, Clone, Deserialize)]
struct JudgeDimension {
    dimension_id: String,
    score: f32,
    reason: String,
    span_refs: Vec<String>,
}

#[path = "evaluation_scoring.rs"]
mod evaluation_scoring;
use evaluation_scoring::{human_score_record, score_record_from_judge};

pub struct EvaluationService {
    repository_root: PathBuf,
    artifact_root: PathBuf,
    data_root: PathBuf,
    automatic_running: AtomicBool,
}

impl EvaluationService {
    pub fn new(
        repository_root: PathBuf,
        artifact_root: PathBuf,
        data_root: PathBuf,
    ) -> Result<Self, CommandError> {
        if !repository_root.is_absolute()
            || !artifact_root.is_absolute()
            || !data_root.is_absolute()
            || !repository_root.join("eval").is_dir()
        {
            return Err(CommandError::evaluation_unavailable());
        }
        Ok(Self {
            repository_root,
            artifact_root,
            data_root,
            automatic_running: AtomicBool::new(false),
        })
    }

    pub fn catalog(&self) -> Result<EvaluationCatalog, CommandError> {
        let offline = self.subjects(OFFLINE_DATASET)?;
        let online = self.subjects(ONLINE_DATASET)?;
        Ok(EvaluationCatalog {
            schema: "desktop-evaluation-catalog/v1",
            datasets: vec![
                dataset_projection(
                    OFFLINE_DATASET,
                    "offline",
                    "离线评测集 · eval-v0.1.0",
                    &offline,
                ),
                dataset_projection(
                    ONLINE_DATASET,
                    "online",
                    "在线评测集 · 本机真实运行",
                    &online,
                ),
            ],
        })
    }

    pub async fn run_automatic(
        &self,
        credentials: &CredentialService,
        provider_settings: &ProviderSettingsService,
        dataset_id: &str,
        case_ids: &[String],
    ) -> Result<EvaluationBatchResult, CommandError> {
        let _running = RunningGuard::acquire(&self.automatic_running)?;
        let selected = self.select_subjects(dataset_id, case_ids)?;
        let rubric = self.rubric()?;
        let manifest = self.manifest()?;
        let package_schema = self.package_schema()?;
        let route = provider_settings
            .route(credentials, "aliyun_bailian")
            .map_err(|_| CommandError::evaluation_failed())?;
        let provider = OpenAiCompatibleProvider::new(Duration::from_secs(180))
            .map_err(|_| CommandError::evaluation_failed())?;
        let system = automatic_system_prompt(&rubric);
        let mut results = Vec::with_capacity(selected.len());

        for subject in selected {
            let Some(artifact) = subject.artifact.as_ref() else {
                results.push(failed_result(&subject.case_id, "artifact_missing"));
                continue;
            };
            let failed_gates = admission_failures(&package_schema, &subject, artifact);
            if !failed_gates.is_empty() {
                results.push(EvaluationCaseResult {
                    case_id: subject.case_id,
                    status: "failed",
                    failed_gates,
                    score_record: None,
                });
                continue;
            }
            let prompt = json!({
                "case": {
                    "input": subject.prompt,
                    "constraints": subject.constraints
                },
                "artifact": artifact,
                "required_output": {
                    "dimensions": rubric.dimensions.iter().map(|item| json!({
                        "dimension_id": item.dimension_id,
                        "score": "integer 1..5",
                        "reason": "1..600 characters",
                        "span_refs": ["one or more real story-package spans"]
                    })).collect::<Vec<_>>()
                }
            });
            let output = provider
                .generate_json(&route, &system, &prompt.to_string())
                .await;
            let Ok(output) = output else {
                results.push(failed_result(&subject.case_id, "judge_request"));
                continue;
            };
            match score_record_from_judge(
                &manifest,
                &rubric,
                &subject,
                artifact,
                &output.artifact,
                &output.model,
            ) {
                Ok(record) => results.push(EvaluationCaseResult {
                    case_id: subject.case_id,
                    status: "completed",
                    failed_gates: Vec::new(),
                    score_record: Some(record),
                }),
                Err(_) => results.push(failed_result(&subject.case_id, "judge_output")),
            }
        }

        let completed_count = results
            .iter()
            .filter(|result| result.status == "completed")
            .count();
        let batch = EvaluationBatchResult {
            schema: "desktop-evaluation-batch-result/v1",
            batch_id: format!("eval_{}", Uuid::new_v4().simple()),
            dataset_id: dataset_id.to_owned(),
            mode: "automatic",
            evidence_status: "partial_advisory",
            selected_count: results.len(),
            completed_count,
            failed_count: results.len() - completed_count,
            results,
            occurred_at_unix_ms: unix_millis()?,
        };
        self.persist_batch(&batch)?;
        Ok(batch)
    }

    pub fn create_blind_assignments(
        &self,
        dataset_id: &str,
        case_ids: &[String],
        rater_id: &str,
    ) -> Result<Vec<BlindAssignment>, CommandError> {
        validate_rater_id(rater_id)?;
        let selected = self.select_subjects(dataset_id, case_ids)?;
        let rubric = self.rubric()?;
        let package_schema = self.package_schema()?;
        let assignment_root = self.data_root.join("assignments");
        std::fs::create_dir_all(&assignment_root)
            .map_err(|_| CommandError::evaluation_unavailable())?;
        let mut assignments = Vec::with_capacity(selected.len());
        for (index, subject) in selected.into_iter().enumerate() {
            let source_artifact = subject
                .artifact
                .clone()
                .ok_or_else(CommandError::evaluation_case_ineligible)?;
            if !admission_failures(&package_schema, &subject, &source_artifact).is_empty() {
                return Err(CommandError::evaluation_case_ineligible());
            }
            let assignment_id = format!("blind_{}", Uuid::new_v4().simple());
            let artifact = blinded_artifact(&source_artifact, &assignment_id)?;
            if !validator_for(&package_schema)
                .map(|validator| validator.is_valid(&artifact))
                .unwrap_or(false)
            {
                return Err(CommandError::evaluation_case_ineligible());
            }
            let public = BlindAssignment {
                schema: "desktop-blind-assignment/v1".into(),
                assignment_id: assignment_id.clone(),
                alias: format!("盲测样本 {:02}", index + 1),
                prompt: subject.prompt,
                constraints: subject.constraints,
                allowed_spans: collect_spans(&artifact),
                artifact: artifact.clone(),
                dimensions: rubric.dimensions.clone(),
            };
            let private = PrivateBlindAssignment {
                schema: "desktop-blind-assignment-private/v1".into(),
                dataset_id: dataset_id.to_owned(),
                case_id: subject.case_id,
                artifact_id: format!("blind-artifact-{assignment_id}"),
                artifact_hash: content_hash(&artifact)?,
                rater_id: rater_id.to_owned(),
                public: public.clone(),
            };
            write_json_atomic(
                &assignment_root.join(format!("{assignment_id}.json")),
                &private,
            )?;
            assignments.push(public);
        }
        Ok(assignments)
    }

    pub fn submit_blind_review(
        &self,
        assignment_id: &str,
        rater_id: &str,
        dimensions: Vec<HumanDimensionInput>,
    ) -> Result<Value, CommandError> {
        validate_assignment_id(assignment_id)?;
        validate_rater_id(rater_id)?;
        let assignment_path = self
            .data_root
            .join("assignments")
            .join(format!("{assignment_id}.json"));
        let private: PrivateBlindAssignment = read_json(&assignment_path)?;
        if private.rater_id != rater_id
            || private.public.assignment_id != assignment_id
            || content_hash(&private.public.artifact)? != private.artifact_hash
        {
            return Err(CommandError::invalid_evaluation());
        }
        let output_path = self
            .data_root
            .join("human-scores")
            .join(format!("{assignment_id}.json"));
        if output_path.exists() {
            return Err(CommandError::evaluation_already_submitted());
        }
        let rubric = self.rubric()?;
        let manifest = self.manifest()?;
        let record = human_score_record(&manifest, &rubric, &private, dimensions)?;
        write_value_atomic(&output_path, &record)?;
        Ok(record)
    }

    fn subjects(&self, dataset_id: &str) -> Result<Vec<EvaluationSubject>, CommandError> {
        match dataset_id {
            OFFLINE_DATASET => self.offline_subjects(),
            ONLINE_DATASET => self.online_subjects(),
            _ => Err(CommandError::invalid_evaluation()),
        }
    }

    fn select_subjects(
        &self,
        dataset_id: &str,
        case_ids: &[String],
    ) -> Result<Vec<EvaluationSubject>, CommandError> {
        if case_ids.is_empty() || case_ids.len() > MAX_SELECTION {
            return Err(CommandError::invalid_evaluation());
        }
        let requested = case_ids.iter().collect::<BTreeSet<_>>();
        if requested.len() != case_ids.len() {
            return Err(CommandError::invalid_evaluation());
        }
        let selected = self
            .subjects(dataset_id)?
            .into_iter()
            .filter(|subject| requested.contains(&subject.case_id))
            .collect::<Vec<_>>();
        if selected.len() != requested.len() {
            return Err(CommandError::invalid_evaluation());
        }
        Ok(selected)
    }

    fn offline_subjects(&self) -> Result<Vec<EvaluationSubject>, CommandError> {
        let packages = baseline_packages(&self.repository_root)?;
        let mut subjects = Vec::new();
        for split in ["dev", "train", "validation", "challenge"] {
            let path = self
                .repository_root
                .join("eval/cases")
                .join(split)
                .join("cases.jsonl");
            let text = std::fs::read_to_string(path)
                .map_err(|_| CommandError::evaluation_unavailable())?;
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                let case: Value = serde_json::from_str(line)
                    .map_err(|_| CommandError::evaluation_unavailable())?;
                let case_id = required_text(&case, "case_id")?;
                let artifact = packages
                    .get(&case_id)
                    .map(|path| read_value(path))
                    .transpose()?;
                let artifact_id = artifact
                    .as_ref()
                    .and_then(|value| value["package_id"].as_str())
                    .map(ToOwned::to_owned);
                subjects.push(EvaluationSubject {
                    case_id,
                    label: required_text(&case, "input")?,
                    genre: required_text(&case, "genre")?,
                    difficulty: case["difficulty"].as_str().map(ToOwned::to_owned),
                    split: Some(split.into()),
                    prompt: required_text(&case, "input")?,
                    constraints: case["constraints"].clone(),
                    case: Some(case),
                    artifact_id,
                    artifact,
                });
            }
        }
        subjects.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        Ok(subjects)
    }

    fn online_subjects(&self) -> Result<Vec<EvaluationSubject>, CommandError> {
        let repository = ArtifactRepository::new(self.artifact_root.clone());
        let mut subjects = Vec::new();
        for summary in repository.list()? {
            let workflow = repository.read(&summary.run_id)?;
            let artifact = workflow["package"].clone();
            let prompt = artifact["logline"]["text"]
                .as_str()
                .unwrap_or("本机真实故事运行")
                .to_owned();
            subjects.push(EvaluationSubject {
                case_id: summary.run_id,
                label: prompt.clone(),
                genre: artifact["promise"]["genre"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_owned(),
                difficulty: None,
                split: None,
                prompt,
                constraints: json!({
                    "source": "local_advisory_run",
                    "episodes": summary.episode_count
                }),
                case: None,
                artifact_id: artifact["package_id"].as_str().map(ToOwned::to_owned),
                artifact: Some(artifact),
            });
        }
        subjects.sort_by(|left, right| left.case_id.cmp(&right.case_id));
        Ok(subjects)
    }

    fn rubric(&self) -> Result<RubricDocument, CommandError> {
        let text = std::fs::read_to_string(self.repository_root.join("eval/rubrics/judge-v1.yaml"))
            .map_err(|_| CommandError::evaluation_unavailable())?;
        serde_yaml::from_str(&text).map_err(|_| CommandError::evaluation_unavailable())
    }

    fn manifest(&self) -> Result<Value, CommandError> {
        read_value(&self.repository_root.join("eval/manifests/eval-v0.1.0.json"))
    }

    fn package_schema(&self) -> Result<Value, CommandError> {
        read_value(&self.repository_root.join("schemas/story-package-v1.json"))
    }

    fn persist_batch(&self, batch: &EvaluationBatchResult) -> Result<(), CommandError> {
        write_json_atomic(
            &self
                .data_root
                .join("automatic-batches")
                .join(&batch.batch_id)
                .join("result.json"),
            batch,
        )
    }
}

struct RunningGuard<'a>(&'a AtomicBool);

impl<'a> RunningGuard<'a> {
    fn acquire(flag: &'a AtomicBool) -> Result<Self, CommandError> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| CommandError::evaluation_active())?;
        Ok(Self(flag))
    }
}

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(any())]
fn human_score_record(
    manifest: &Value,
    rubric: &RubricDocument,
    private: &PrivateBlindAssignment,
    dimensions: Vec<HumanDimensionInput>,
) -> Result<Value, CommandError> {
    let dimensions = dimensions
        .into_iter()
        .map(|item| JudgeDimension {
            dimension_id: item.dimension_id,
            score: item.score,
            reason: item.reason,
            span_refs: item.span_refs,
        })
        .collect();
    let subject = EvaluationSubject {
        case_id: private.case_id.clone(),
        label: private.public.alias.clone(),
        genre: String::new(),
        difficulty: None,
        split: None,
        prompt: private.public.prompt.clone(),
        constraints: private.public.constraints.clone(),
        case: None,
        artifact_id: Some(private.artifact_id.clone()),
        artifact: Some(private.public.artifact.clone()),
    };
    build_score_record(
        manifest,
        rubric,
        &subject,
        &private.public.artifact,
        dimensions,
        json!({
            "rater_id": private.rater_id,
            "rater_type": "internal_spot_check",
            "model_id": null,
            "seed": null,
            "sample_index": null,
            "credential": null,
            "blind_assignment_id": private.public.assignment_id,
            "rater_blinded": true
        }),
        format!("human_{}", Uuid::new_v4().simple()),
        private.public.assignment_id.clone(),
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg(any())]
fn build_score_record(
    manifest: &Value,
    rubric: &RubricDocument,
    subject: &EvaluationSubject,
    artifact: &Value,
    dimensions: Vec<JudgeDimension>,
    rater: Value,
    record_id: String,
    run_id: String,
) -> Result<Value, CommandError> {
    let expected = rubric
        .dimensions
        .iter()
        .map(|item| item.dimension_id.as_str())
        .collect::<BTreeSet<_>>();
    let actual = dimensions
        .iter()
        .map(|item| item.dimension_id.as_str())
        .collect::<BTreeSet<_>>();
    let spans = collect_spans(artifact).into_iter().collect::<BTreeSet<_>>();
    if dimensions.len() != expected.len()
        || actual != expected
        || dimensions.iter().any(|item| {
            !item.score.is_finite()
                || !(1.0..=5.0).contains(&item.score)
                || item.score.fract() != 0.0
                || item.reason.trim().is_empty()
                || item.reason.chars().count() > 600
                || item.span_refs.is_empty()
                || item.span_refs.iter().any(|span| !spans.contains(span))
        })
    {
        return Err(CommandError::invalid_evaluation_score());
    }
    let spec =
        RubricSpec::from_manifest(manifest).map_err(|_| CommandError::evaluation_failed())?;
    let scores = dimensions
        .iter()
        .map(|item| (item.dimension_id.clone(), item.score))
        .collect::<DimensionScores>();
    let assessment =
        Assessment::new(true, &spec, &scores).map_err(|_| CommandError::evaluation_failed())?;
    let thresholds = Thresholds::from_manifest(manifest);
    let outcome =
        verdict(&assessment, &thresholds).map_err(|_| CommandError::evaluation_failed())?;
    let pillars = spec
        .pillars()
        .iter()
        .zip(assessment.pillars.as_slice())
        .map(|(pillar, score)| (pillar.name.clone(), json!(score)))
        .collect::<serde_json::Map<_, _>>();
    let artifact_bytes =
        serde_json::to_vec(artifact).map_err(|_| CommandError::evaluation_failed())?;
    Ok(json!({
        "schema": "eval-score-record/v1",
        "record_id": record_id,
        "run_id": run_id,
        "case_id": subject.case_id,
        "artifact_id": subject.artifact_id.as_deref().unwrap_or("unknown"),
        "artifact_content_hash": content_hash(artifact)?,
        "artifact_char_count": String::from_utf8_lossy(&artifact_bytes).chars().count(),
        "rubric_version": rubric.version,
        "rater": rater,
        "admission": {"passed": true, "failed_gates": []},
        "dimensions": dimensions.iter().map(|item| json!({
            "dimension_id": item.dimension_id,
            "score": item.score,
            "reason": item.reason,
            "span_refs": item.span_refs,
            "valid": true,
            "invalid_reason": null
        })).collect::<Vec<_>>(),
        "located_defect_spans": [],
        "aggregate": {
            "pillars": pillars,
            "geometric_mean": assessment.pillars.geometric_mean(),
            "legacy_weighted_sum": assessment.pillars.legacy_arithmetic_mean(),
            "floors_passed": assessment.floors_pass(&thresholds).map_err(|_| CommandError::evaluation_failed())?,
            "verdict": match outcome {
                Verdict::Reject => "reject",
                Verdict::Consider => "consider",
                Verdict::Pass => "pass"
            }
        },
        "adjudication_required": false
    }))
}

#[path = "evaluation_support.rs"]
mod evaluation_support;
use evaluation_support::*;

pub fn default_evaluation_root() -> PathBuf {
    std::env::var_os("MICROCODEX_EVALUATION_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("MicrocodeX")
                .join("ShortDramaStudio")
                .join("evaluation")
        })
}

#[cfg(test)]
#[path = "evaluations_tests.rs"]
mod tests;
