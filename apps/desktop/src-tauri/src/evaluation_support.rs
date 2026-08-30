use super::*;

pub(super) fn collect_spans(artifact: &Value) -> Vec<String> {
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

pub(super) fn blinded_artifact(source: &Value, assignment_id: &str) -> Result<Value, CommandError> {
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

pub(super) fn failed_result(case_id: &str, gate: &str) -> EvaluationCaseResult {
    EvaluationCaseResult {
        case_id: case_id.to_owned(),
        status: "failed",
        failed_gates: vec![gate.into()],
        score_record: None,
    }
}

pub(super) fn required_text(value: &Value, key: &str) -> Result<String, CommandError> {
    value[key]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(CommandError::evaluation_unavailable)
}

pub(super) fn content_hash(value: &Value) -> Result<String, CommandError> {
    let bytes = serde_json::to_vec(value).map_err(|_| CommandError::evaluation_failed())?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

pub(super) fn validate_rater_id(value: &str) -> Result<(), CommandError> {
    if !(2..=64).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(CommandError::invalid_evaluation());
    }
    Ok(())
}

pub(super) fn validate_assignment_id(value: &str) -> Result<(), CommandError> {
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

pub(super) fn unix_millis() -> Result<u64, CommandError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CommandError::evaluation_unavailable())?;
    u64::try_from(duration.as_millis()).map_err(|_| CommandError::evaluation_unavailable())
}

pub(super) fn read_value(path: &Path) -> Result<Value, CommandError> {
    serde_json::from_slice(
        &std::fs::read(path).map_err(|_| CommandError::evaluation_unavailable())?,
    )
    .map_err(|_| CommandError::evaluation_unavailable())
}

pub(super) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, CommandError> {
    serde_json::from_slice(
        &std::fs::read(path).map_err(|_| CommandError::evaluation_unavailable())?,
    )
    .map_err(|_| CommandError::evaluation_unavailable())
}

pub(super) fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), CommandError> {
    let encoded =
        serde_json::to_vec_pretty(value).map_err(|_| CommandError::evaluation_unavailable())?;
    write_bytes_atomic(path, &encoded)
}

pub(super) fn write_value_atomic(path: &Path, value: &Value) -> Result<(), CommandError> {
    let encoded =
        serde_json::to_vec_pretty(value).map_err(|_| CommandError::evaluation_unavailable())?;
    write_bytes_atomic(path, &encoded)
}

pub(super) fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
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

pub(super) fn dataset_projection(
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

pub(super) fn baseline_packages(
    repository_root: &Path,
) -> Result<BTreeMap<String, PathBuf>, CommandError> {
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

pub(super) fn admission_failures(
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

pub(super) fn automatic_system_prompt(rubric: &RubricDocument) -> String {
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
