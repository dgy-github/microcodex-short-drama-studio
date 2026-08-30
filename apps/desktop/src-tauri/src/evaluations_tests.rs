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
    assert_eq!(catalog.datasets[0].case_count, 120);
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
    assert!(
        admission_failures(&schema, &subject, &artifact).contains(&"required_elements".to_owned())
    );
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
