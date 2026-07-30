use jsonschema::validator_for;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use story_eval::{verdict, Assessment, DimensionScores, RubricSpec, Thresholds, Verdict};
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

fn dataset_projection(
    dataset_id: &str,
    kind: &'static str,
    label: &'static str,
    subjects: &[EvaluationSubject],
) -> EvaluationDataset {
    EvaluationDataset {
        dataset_id: dataset_id.into(),
        kind,
        label,
        case_count: subjects.len(),
        eligible_count: subjects
            .iter()
            .filter(|subject| subject.artifact.is_some())
            .count(),
        cases: subjects
            .iter()
            .map(|subject| EvaluationCaseSummary {
                case_id: subject.case_id.clone(),
                label: subject.label.clone(),
                genre: subject.genre.clone(),
                difficulty: subject.difficulty.clone(),
                split: subject.split.clone(),
                eligible: subject.artifact.is_some(),
            })
            .collect(),
    }
}

fn baseline_packages(repository_root: &Path) -> Result<BTreeMap<String, PathBuf>, CommandError> {
    let root = repository_root.join("eval/baselines");
    let mut archives = std::fs::read_dir(root)
        .map_err(|_| CommandError::evaluation_unavailable())?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    archives.sort_by_key(|entry| entry.file_name());
    let mut packages = BTreeMap::new();
    for archive in archives {
        let index_path = archive.path().join("index.json");
        if !index_path.is_file() {
            continue;
        }
        let index = read_value(&index_path)?;
        for item in index["cases"].as_array().into_iter().flatten() {
            let Some(case_id) = item["case_id"].as_str() else {
                return Err(CommandError::evaluation_unavailable());
            };
            let Some(relative) = item["package"].as_str() else {
                return Err(CommandError::evaluation_unavailable());
            };
            if Path::new(relative).is_absolute() || relative.contains("..") {
                return Err(CommandError::evaluation_unavailable());
            }
            let package = archive.path().join(relative);
            if packages.insert(case_id.to_owned(), package).is_some() {
                return Err(CommandError::evaluation_unavailable());
            }
        }
    }
    Ok(packages)
}

fn admission_failures(
    schema: &Value,
    subject: &EvaluationSubject,
    artifact: &Value,
) -> Vec<String> {
    let mut failures = Vec::new();
    let schema_valid = validator_for(schema)
        .map(|validator| validator.is_valid(artifact))
        .unwrap_or(false);
    if !schema_valid {
        failures.push("artifact_schema".into());
        return failures;
    }
    let Some(case) = &subject.case else {
        return failures;
    };
    let expected_episodes = case["constraints"]["episodes"].as_u64();
    let actual_episodes = artifact["episodes"]
        .as_array()
        .map(|items| items.len() as u64);
    if expected_episodes != actual_episodes {
        failures.push("format_constraints".into());
    }
    let encoded = serde_json::to_string(artifact).unwrap_or_default();
    if case["required_elements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|required| !encoded.contains(required))
    {
        failures.push("required_elements".into());
    }
    if case["forbidden_elements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|forbidden| encoded.contains(forbidden))
    {
        failures.push("forbidden_elements".into());
    }
    let license_id = case["rights"]["license_id"].as_str();
    if license_id.is_none()
        || !artifact["provenance"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|item| item["license_id"].as_str() == license_id)
    {
        failures.push("provenance_complete".into());
    }
    failures
}

fn automatic_system_prompt(rubric: &RubricDocument) -> String {
    let mut lines = vec![
        "你是短剧评测员。只输出 JSON，不要 Markdown。".to_owned(),
        "不能奖励文本长度；每项必须引用真实 story-package span。".to_owned(),
        format!("rubric_version={}", rubric.version),
    ];
    for dimension in &rubric.dimensions {
        lines.push(format!(
            "{}（{}）：{}\n1分：{}\n3分：{}\n5分：{}",
            dimension.dimension_id,
            dimension.name,
            dimension.ask,
            dimension.anchors.get(&1).map(String::as_str).unwrap_or(""),
            dimension.anchors.get(&3).map(String::as_str).unwrap_or(""),
            dimension.anchors.get(&5).map(String::as_str).unwrap_or("")
        ));
    }
    lines.push(
        "输出 {\"dimensions\":[{\"dimension_id\":\"...\",\"score\":1,\"reason\":\"...\",\"span_refs\":[\"story-package/...\"]}]}，恰好覆盖全部维度。".into(),
    );
    lines.join("\n")
}

fn score_record_from_judge(
    manifest: &Value,
    rubric: &RubricDocument,
    subject: &EvaluationSubject,
    artifact: &Value,
    output: &Value,
    model: &str,
) -> Result<Value, CommandError> {
    let response: JudgeResponse =
        serde_json::from_value(output.clone()).map_err(|_| CommandError::evaluation_failed())?;
    build_score_record(
        manifest,
        rubric,
        subject,
        artifact,
        response.dimensions,
        json!({
            "rater_id": format!("judge_{model}"),
            "rater_type": "llm_judge",
            "model_id": model,
            "seed": null,
            "sample_index": 0,
            "credential": null,
            "blind_assignment_id": null,
            "rater_blinded": true
        }),
        format!("auto_{}", Uuid::new_v4().simple()),
        format!("eval_auto_{}", Uuid::new_v4().simple()),
    )
}

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

fn collect_spans(artifact: &Value) -> Vec<String> {
    fn walk(value: &Value, parent: &str, output: &mut BTreeSet<String>) {
        match value {
            Value::Object(object) => {
                let current = object
                    .get("node_id")
                    .and_then(Value::as_str)
                    .map(|node| format!("{parent}/{node}"));
                if let Some(path) = &current {
                    output.insert(path.clone());
                }
                let next = current.as_deref().unwrap_or(parent);
                for child in object.values() {
                    walk(child, next, output);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, parent, output);
                }
            }
            _ => {}
        }
    }
    let mut spans = BTreeSet::new();
    walk(artifact, "story-package", &mut spans);
    spans.into_iter().collect()
}

fn blinded_artifact(source: &Value, assignment_id: &str) -> Result<Value, CommandError> {
    let mut artifact = source.clone();
    let object = artifact
        .as_object_mut()
        .ok_or_else(CommandError::evaluation_case_ineligible)?;
    object.insert(
        "package_id".into(),
        Value::String(format!("blind-package-{assignment_id}")),
    );
    object.insert(
        "job_id".into(),
        Value::String(format!("blind-job-{assignment_id}")),
    );
    object.insert("case_id".into(), Value::Null);
    object.remove("supersedes");
    object.remove("node_correspondence");
    object.insert(
        "provenance".into(),
        json!([{
            "source_id": "blind-source-1",
            "license_id": "redacted-evaluation-license",
            "usage": "blinded evaluation input"
        }]),
    );
    Ok(artifact)
}

fn failed_result(case_id: &str, gate: &str) -> EvaluationCaseResult {
    EvaluationCaseResult {
        case_id: case_id.to_owned(),
        status: "failed",
        failed_gates: vec![gate.into()],
        score_record: None,
    }
}

fn required_text(value: &Value, key: &str) -> Result<String, CommandError> {
    value[key]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(CommandError::evaluation_unavailable)
}

fn content_hash(value: &Value) -> Result<String, CommandError> {
    let bytes = serde_json::to_vec(value).map_err(|_| CommandError::evaluation_failed())?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn validate_rater_id(value: &str) -> Result<(), CommandError> {
    if !(2..=64).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(CommandError::invalid_evaluation());
    }
    Ok(())
}

fn validate_assignment_id(value: &str) -> Result<(), CommandError> {
    let Some(suffix) = value.strip_prefix("blind_") else {
        return Err(CommandError::invalid_evaluation());
    };
    if !(16..=64).contains(&suffix.len())
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(CommandError::invalid_evaluation());
    }
    Ok(())
}

fn unix_millis() -> Result<u64, CommandError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CommandError::evaluation_unavailable())?;
    u64::try_from(duration.as_millis()).map_err(|_| CommandError::evaluation_unavailable())
}

fn read_value(path: &Path) -> Result<Value, CommandError> {
    serde_json::from_slice(
        &std::fs::read(path).map_err(|_| CommandError::evaluation_unavailable())?,
    )
    .map_err(|_| CommandError::evaluation_unavailable())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, CommandError> {
    serde_json::from_slice(
        &std::fs::read(path).map_err(|_| CommandError::evaluation_unavailable())?,
    )
    .map_err(|_| CommandError::evaluation_unavailable())
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), CommandError> {
    let encoded =
        serde_json::to_vec_pretty(value).map_err(|_| CommandError::evaluation_unavailable())?;
    write_bytes_atomic(path, &encoded)
}

fn write_value_atomic(path: &Path, value: &Value) -> Result<(), CommandError> {
    let encoded =
        serde_json::to_vec_pretty(value).map_err(|_| CommandError::evaluation_unavailable())?;
    write_bytes_atomic(path, &encoded)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = path
        .parent()
        .ok_or_else(CommandError::evaluation_unavailable)?;
    std::fs::create_dir_all(parent).map_err(|_| CommandError::evaluation_unavailable())?;
    if path.exists() {
        return Err(CommandError::evaluation_already_submitted());
    }
    let temporary = path.with_extension(format!("{}.partial", Uuid::new_v4().simple()));
    std::fs::write(&temporary, bytes).map_err(|_| CommandError::evaluation_unavailable())?;
    if path.exists() {
        let _ = std::fs::remove_file(&temporary);
        return Err(CommandError::evaluation_already_submitted());
    }
    if std::fs::rename(&temporary, path).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return if path.exists() {
            Err(CommandError::evaluation_already_submitted())
        } else {
            Err(CommandError::evaluation_unavailable())
        };
    }
    Ok(())
}

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
mod tests {
    use super::*;
    use crate::artifacts::default_artifact_root;
    use crate::run_controller::default_repository_root;
    use jsonschema::Resource;

    fn service(data_root: PathBuf) -> EvaluationService {
        EvaluationService::new(
            default_repository_root().canonicalize().unwrap(),
            default_artifact_root(),
            data_root,
        )
        .unwrap()
    }

    fn valid_dimensions(assignment: &BlindAssignment) -> Vec<HumanDimensionInput> {
        assignment
            .dimensions
            .iter()
            .map(|dimension| HumanDimensionInput {
                dimension_id: dimension.dimension_id.clone(),
                score: 4.0,
                reason: "该维度有明确的结构化证据。".into(),
                span_refs: vec![assignment.allowed_spans[0].clone()],
            })
            .collect()
    }

    #[test]
    fn catalog_exposes_two_datasets_and_marks_archived_offline_cases() {
        let directory = tempfile::tempdir().unwrap();
        let service = service(directory.path().join("evaluation"));
        let catalog = service.catalog().unwrap();
        assert_eq!(catalog.datasets.len(), 2);
        assert_eq!(catalog.datasets[0].dataset_id, OFFLINE_DATASET);
        assert_eq!(catalog.datasets[0].case_count, 30);
        assert_eq!(catalog.datasets[0].eligible_count, 10);
        assert_eq!(catalog.datasets[1].dataset_id, ONLINE_DATASET);
    }

    #[test]
    fn judge_scores_require_every_dimension_and_real_spans() {
        let directory = tempfile::tempdir().unwrap();
        let service = service(directory.path().join("evaluation"));
        let subject = service
            .select_subjects(OFFLINE_DATASET, &["family_001".into()])
            .unwrap()
            .remove(0);
        let rubric = service.rubric().unwrap();
        let manifest = service.manifest().unwrap();
        let artifact = subject.artifact.as_ref().unwrap();
        let span = collect_spans(artifact)[0].clone();
        let output = json!({
            "dimensions": rubric.dimensions.iter().map(|dimension| json!({
                "dimension_id": dimension.dimension_id,
                "score": 4,
                "reason": "结构证据明确。",
                "span_refs": [span]
            })).collect::<Vec<_>>()
        });
        let record = score_record_from_judge(
            &manifest,
            &rubric,
            &subject,
            artifact,
            &output,
            "judge-test",
        )
        .unwrap();
        assert_eq!(record["aggregate"]["verdict"], "pass");
        assert!(score_record_from_judge(
            &manifest,
            &rubric,
            &subject,
            artifact,
            &json!({"dimensions": []}),
            "judge-test"
        )
        .is_err());
    }

    #[test]
    fn blind_assignment_omits_prohibited_metadata_and_is_append_only() {
        let directory = tempfile::tempdir().unwrap();
        let service = service(directory.path().join("evaluation"));
        let assignment = service
            .create_blind_assignments(OFFLINE_DATASET, &["family_001".into()], "reviewer_01")
            .unwrap()
            .remove(0);
        let encoded = serde_json::to_string(&assignment).unwrap();
        for prohibited in [
            "split",
            "deepseek",
            "generator",
            "defect_key",
            "source_path",
        ] {
            assert!(!encoded.contains(prohibited));
        }
        let dimensions = valid_dimensions(&assignment);
        let record = service
            .submit_blind_review(&assignment.assignment_id, "reviewer_01", dimensions.clone())
            .unwrap();
        assert_eq!(record["rater"]["rater_type"], "internal_spot_check");
        assert!(service
            .submit_blind_review(&assignment.assignment_id, "reviewer_01", dimensions)
            .is_err());
        let residuals = std::fs::read_dir(service.data_root.join("human-scores"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("partial"))
            .count();
        assert_eq!(residuals, 0);
    }

    #[test]
    fn admission_rejects_missing_required_case_element() {
        let directory = tempfile::tempdir().unwrap();
        let service = service(directory.path().join("evaluation"));
        let subject = service
            .select_subjects(OFFLINE_DATASET, &["family_001".into()])
            .unwrap()
            .remove(0);
        let schema = service.package_schema().unwrap();
        let mut artifact = subject.artifact.clone().unwrap();
        replace_text(&mut artifact, "老房子", "旧住所");
        assert!(admission_failures(&schema, &subject, &artifact)
            .contains(&"required_elements".to_owned()));
    }

    #[test]
    fn public_catalog_and_blind_assignment_match_json_contracts() {
        let directory = tempfile::tempdir().unwrap();
        let service = service(directory.path().join("evaluation"));
        let catalog = serde_json::to_value(service.catalog().unwrap()).unwrap();
        let catalog_schema = service
            .repository_root
            .join("schemas/desktop-evaluation-catalog-v1.json");
        let catalog_validator = validator_for(&read_value(&catalog_schema).unwrap()).unwrap();
        assert!(catalog_validator.is_valid(&catalog));

        let assignment = service
            .create_blind_assignments(OFFLINE_DATASET, &["family_001".into()], "reviewer_02")
            .unwrap()
            .remove(0);
        let blind_schema = read_value(
            &service
                .repository_root
                .join("schemas/desktop-blind-assignment-v1.json"),
        )
        .unwrap();
        let package_schema = service.package_schema().unwrap();
        let blind_validator = jsonschema::options()
            .with_resource(
                "https://microcodex.local/schemas/story-package-v1.json",
                Resource::from_contents(package_schema).unwrap(),
            )
            .build(&blind_schema)
            .unwrap();
        assert!(blind_validator.is_valid(&serde_json::to_value(assignment).unwrap()));

        let batch = EvaluationBatchResult {
            schema: "desktop-evaluation-batch-result/v1",
            batch_id: "eval_0123456789abcdef".into(),
            dataset_id: OFFLINE_DATASET.into(),
            mode: "automatic",
            evidence_status: "partial_advisory",
            selected_count: 1,
            completed_count: 0,
            failed_count: 1,
            results: vec![failed_result("family_001", "artifact_missing")],
            occurred_at_unix_ms: 1,
        };
        let batch_schema = read_value(
            &service
                .repository_root
                .join("schemas/desktop-evaluation-batch-result-v1.json"),
        )
        .unwrap();
        let score_schema = read_value(
            &service
                .repository_root
                .join("schemas/eval-score-record-v1.json"),
        )
        .unwrap();
        let batch_validator = jsonschema::options()
            .with_resource(
                "https://microcodex.local/schemas/eval-score-record-v1.json",
                Resource::from_contents(score_schema).unwrap(),
            )
            .build(&batch_schema)
            .unwrap();
        assert!(batch_validator.is_valid(&serde_json::to_value(batch).unwrap()));
    }

    fn replace_text(value: &mut Value, from: &str, to: &str) {
        match value {
            Value::String(text) => *text = text.replace(from, to),
            Value::Array(items) => {
                for item in items {
                    replace_text(item, from, to);
                }
            }
            Value::Object(object) => {
                for item in object.values_mut() {
                    replace_text(item, from, to);
                }
            }
            _ => {}
        }
    }
}
