use serde_json::{json, Value};
use std::collections::BTreeSet;
use story_eval::{verdict, Assessment, DimensionScores, RubricSpec, Thresholds, Verdict};
use uuid::Uuid;

use super::evaluation_support::{collect_spans, content_hash};
use super::{
    EvaluationSubject, HumanDimensionInput, JudgeDimension, JudgeResponse, PrivateBlindAssignment,
    RubricDocument,
};
use crate::CommandError;

pub(super) fn score_record_from_judge(
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
        json!({"rater_id": format!("judge_{model}"), "rater_type": "llm_judge", "model_id": model, "seed": null, "sample_index": 0, "credential": null, "blind_assignment_id": null, "rater_blinded": true}),
        format!("auto_{}", Uuid::new_v4().simple()),
        format!("eval_auto_{}", Uuid::new_v4().simple()),
    )
}

pub(super) fn human_score_record(
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
        json!({"rater_id": private.rater_id, "rater_type": "internal_spot_check", "model_id": null, "seed": null, "sample_index": null, "credential": null, "blind_assignment_id": private.public.assignment_id, "rater_blinded": true}),
        format!("human_{}", Uuid::new_v4().simple()),
        private.public.assignment_id.clone(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_score_record(
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
    Ok(
        json!({"schema":"eval-score-record/v1","record_id":record_id,"run_id":run_id,"case_id":subject.case_id,"artifact_id":subject.artifact_id.as_deref().unwrap_or("unknown"),"artifact_content_hash":content_hash(artifact)?,"artifact_char_count":String::from_utf8_lossy(&artifact_bytes).chars().count(),"rubric_version":rubric.version,"rater":rater,"admission":{"passed":true,"failed_gates":[]},"dimensions":dimensions.iter().map(|item| json!({"dimension_id":item.dimension_id,"score":item.score,"reason":item.reason,"span_refs":item.span_refs,"valid":true,"invalid_reason":null})).collect::<Vec<_>>(),"located_defect_spans":[],"aggregate":{"pillars":pillars,"geometric_mean":assessment.pillars.geometric_mean(),"legacy_weighted_sum":assessment.pillars.legacy_arithmetic_mean(),"floors_passed":assessment.floors_pass(&thresholds).map_err(|_| CommandError::evaluation_failed())?,"verdict":match outcome { Verdict::Reject=>"reject", Verdict::Consider=>"consider", Verdict::Pass=>"pass"}},"adjudication_required":false}),
    )
}
