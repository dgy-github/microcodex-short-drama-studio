//! Deterministic gates and score aggregation used before expensive review.
//!
//! Contract: `docs/STORY_EVAL_V1.md`. Thresholds live in
//! `eval/manifests/eval-v*.json` and are passed in, never hard-coded at a call
//! site.
//!
//! Aggregation is non-compensatory by design: pillars combine geometrically and
//! floors are absolute, so a strong pillar cannot buy a dead one.
//!
//! # Why the rubric shape is data, not types
//!
//! The pillar set and the critical-dimension set are read from the manifest
//! rather than declared as struct fields. Only the scripted-drama content form
//! is implemented today, but a knowledge/explainer form has no character
//! pillar at all, and a four-field struct would force a code change and a
//! recompile to express that. Keeping the shape in configuration means adding
//! a content form supplies an artifact schema, a rubric and a case set — and
//! touches none of this arithmetic.
//!
//! What is genuinely form-agnostic lives here: arithmetic within a pillar,
//! geometric mean across pillars, absolute floors, and the three-tier verdict.
//! What is form-specific — which pillars exist, which dimensions are critical —
//! lives in the manifest.

use std::collections::BTreeMap;

use serde_json::Value;

/// Lowest score that is not an automatic critical failure.
pub const DIMENSION_HARD_FLOOR: f32 = 1.0;
/// Default pillar and critical-dimension floor.
pub const DEFAULT_FLOOR: f32 = 3.0;
/// Default boundary between `Consider` and `Pass`.
pub const DEFAULT_PASS_THRESHOLD: f32 = 3.5;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RubricError {
    #[error("manifest is missing the `{0}` section")]
    MissingSection(&'static str),
    #[error("pillar `{0}` declares no dimensions")]
    EmptyPillar(String),
    #[error("rubric declares no pillars")]
    NoPillars,
    #[error("no score supplied for dimension `{0}`")]
    MissingScore(String),
}

/// One pillar: a name and the dimensions averaged inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PillarSpec {
    pub name: String,
    pub dimensions: Vec<String>,
}

/// The rubric's shape. Loaded from a manifest; never declared in code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RubricSpec {
    pillars: Vec<PillarSpec>,
    critical_dimensions: Vec<String>,
}

impl RubricSpec {
    pub fn new(
        pillars: Vec<PillarSpec>,
        critical_dimensions: Vec<String>,
    ) -> Result<Self, RubricError> {
        if pillars.is_empty() {
            return Err(RubricError::NoPillars);
        }
        if let Some(empty) = pillars.iter().find(|pillar| pillar.dimensions.is_empty()) {
            return Err(RubricError::EmptyPillar(empty.name.clone()));
        }
        Ok(Self {
            pillars,
            critical_dimensions,
        })
    }

    /// Parse the `pillars` and `floors.critical_dimensions` sections of a
    /// manifest. Pillars are ordered by name so aggregation is deterministic
    /// regardless of JSON key order.
    pub fn from_manifest(manifest: &Value) -> Result<Self, RubricError> {
        let raw = manifest
            .get("pillars")
            .and_then(Value::as_object)
            .ok_or(RubricError::MissingSection("pillars"))?;

        let mut pillars: Vec<PillarSpec> = raw
            .iter()
            .filter(|(name, _)| !name.starts_with('_'))
            .map(|(name, body)| PillarSpec {
                name: name.clone(),
                dimensions: body
                    .get("dimensions")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect();
        pillars.sort_by(|left, right| left.name.cmp(&right.name));

        let critical = manifest
            .get("floors")
            .and_then(|floors| floors.get("critical_dimensions"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();

        Self::new(pillars, critical)
    }

    pub fn pillars(&self) -> &[PillarSpec] {
        &self.pillars
    }

    pub fn critical_dimensions(&self) -> &[String] {
        &self.critical_dimensions
    }

    pub fn pillar_count(&self) -> usize {
        self.pillars.len()
    }

    /// Every dimension named by any pillar, deduplicated and ordered.
    pub fn dimension_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .pillars
            .iter()
            .flat_map(|pillar| pillar.dimensions.iter().cloned())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

/// Per-dimension scores keyed by dimension id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DimensionScores(BTreeMap<String, f32>);

impl DimensionScores {
    pub fn get(&self, dimension: &str) -> Option<f32> {
        self.0.get(dimension).copied()
    }

    pub fn insert(&mut self, dimension: impl Into<String>, score: f32) {
        self.0.insert(dimension.into(), score);
    }

    pub fn values(&self) -> impl Iterator<Item = f32> + '_ {
        self.0.values().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<K: Into<String>> FromIterator<(K, f32)> for DimensionScores {
    fn from_iter<I: IntoIterator<Item = (K, f32)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(key, score)| (key.into(), score))
                .collect(),
        )
    }
}

/// Arithmetic mean of the dimensions inside one pillar.
///
/// Compensation within a pillar is intended: its dimensions measure closely
/// related craft. Compensation *across* pillars is what the geometric mean and
/// the floors prevent.
pub fn pillar_from_dimensions(dimensions: &[f32]) -> Option<f32> {
    if dimensions.is_empty() {
        return None;
    }
    let sum: f32 = dimensions.iter().sum();
    Some(sum / dimensions.len() as f32)
}

/// Aggregated pillar values, however many the rubric declares.
#[derive(Debug, Clone, PartialEq)]
pub struct PillarScores(Vec<f32>);

impl PillarScores {
    pub fn from_scores(spec: &RubricSpec, scores: &DimensionScores) -> Result<Self, RubricError> {
        let mut values = Vec::with_capacity(spec.pillar_count());
        for pillar in spec.pillars() {
            let mut collected = Vec::with_capacity(pillar.dimensions.len());
            for dimension in &pillar.dimensions {
                let score = scores
                    .get(dimension)
                    .ok_or_else(|| RubricError::MissingScore(dimension.clone()))?;
                collected.push(score);
            }
            let mean = pillar_from_dimensions(&collected)
                .ok_or_else(|| RubricError::EmptyPillar(pillar.name.clone()))?;
            values.push(mean);
        }
        Ok(Self(values))
    }

    pub fn from_values(values: Vec<f32>) -> Self {
        Self(values)
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Gating aggregate, the n-th root of the product over n pillars.
    ///
    /// Returns `0.0` if any pillar is non-positive, which is out of range for a
    /// 1-5 rubric and indicates a malformed record rather than a bad story.
    pub fn geometric_mean(&self) -> f32 {
        if self.0.is_empty() {
            return 0.0;
        }
        if self.0.iter().any(|value| *value <= 0.0) {
            return 0.0;
        }
        let product: f32 = self.0.iter().product();
        product.powf(1.0 / self.0.len() as f32)
    }

    /// Shadow metric only. Stored beside the geometric mean so the two
    /// aggregations can be compared on real data. Never gates.
    pub fn legacy_arithmetic_mean(&self) -> f32 {
        if self.0.is_empty() {
            return 0.0;
        }
        self.0.iter().sum::<f32>() / self.0.len() as f32
    }

    pub fn lowest(&self) -> f32 {
        self.0.iter().copied().fold(f32::INFINITY, f32::min)
    }

    pub fn floors_pass(&self, minimum: f32) -> bool {
        !self.0.is_empty() && self.0.iter().all(|value| *value >= minimum)
    }
}

pub fn critical_dimensions_pass(
    spec: &RubricSpec,
    scores: &DimensionScores,
    minimum: f32,
) -> Result<bool, RubricError> {
    for dimension in spec.critical_dimensions() {
        let score = scores
            .get(dimension)
            .ok_or_else(|| RubricError::MissingScore(dimension.clone()))?;
        if score < minimum {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Three-tier outcome, mirroring the shape industry script coverage uses:
/// the diagnostic grid and the holistic verdict stay separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Reject,
    Consider,
    Pass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    pub pillar_floor: f32,
    pub critical_floor: f32,
    pub pass_threshold: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            pillar_floor: DEFAULT_FLOOR,
            critical_floor: DEFAULT_FLOOR,
            pass_threshold: DEFAULT_PASS_THRESHOLD,
        }
    }
}

impl Thresholds {
    pub fn from_manifest(manifest: &Value) -> Self {
        let number = |path: [&str; 2], fallback: f32| -> f32 {
            manifest
                .get(path[0])
                .and_then(|section| section.get(path[1]))
                .and_then(Value::as_f64)
                .map(|value| value as f32)
                .unwrap_or(fallback)
        };
        Self {
            pillar_floor: number(["floors", "pillar_minimum"], DEFAULT_FLOOR),
            critical_floor: number(["floors", "critical_dimension_minimum"], DEFAULT_FLOOR),
            pass_threshold: number(["verdict", "pass_threshold"], DEFAULT_PASS_THRESHOLD),
        }
    }
}

/// Everything the verdict depends on, gathered so no caller can skip a floor.
#[derive(Debug, Clone)]
pub struct Assessment<'a> {
    pub admission_passed: bool,
    pub spec: &'a RubricSpec,
    pub scores: &'a DimensionScores,
    pub pillars: PillarScores,
}

impl<'a> Assessment<'a> {
    pub fn new(
        admission_passed: bool,
        spec: &'a RubricSpec,
        scores: &'a DimensionScores,
    ) -> Result<Self, RubricError> {
        let pillars = PillarScores::from_scores(spec, scores)?;
        Ok(Self {
            admission_passed,
            spec,
            scores,
            pillars,
        })
    }

    /// A single `1` anywhere is a critical failure regardless of the average.
    pub fn has_hard_floor_breach(&self) -> bool {
        self.scores
            .values()
            .any(|score| score <= DIMENSION_HARD_FLOOR)
    }

    pub fn floors_pass(&self, thresholds: &Thresholds) -> Result<bool, RubricError> {
        if self.has_hard_floor_breach() {
            return Ok(false);
        }
        if !self.pillars.floors_pass(thresholds.pillar_floor) {
            return Ok(false);
        }
        critical_dimensions_pass(self.spec, self.scores, thresholds.critical_floor)
    }
}

pub fn verdict(assessment: &Assessment, thresholds: &Thresholds) -> Result<Verdict, RubricError> {
    if !assessment.admission_passed || !assessment.floors_pass(thresholds)? {
        return Ok(Verdict::Reject);
    }
    Ok(
        if assessment.pillars.geometric_mean() >= thresholds.pass_threshold {
            Verdict::Pass
        } else {
            Verdict::Consider
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    /// The scripted-drama rubric, built the way a caller would from config.
    fn drama_spec() -> RubricSpec {
        RubricSpec::new(
            vec![
                PillarSpec {
                    name: "character_credibility".into(),
                    dimensions: vec![
                        "human_credibility".into(),
                        "character_distinction".into(),
                        "dialogue_subtext".into(),
                    ],
                },
                PillarSpec {
                    name: "originality_delivery".into(),
                    dimensions: vec!["originality".into(), "producibility".into()],
                },
                PillarSpec {
                    name: "structure_causality".into(),
                    dimensions: vec!["causal_coherence".into(), "continuity".into()],
                },
                PillarSpec {
                    name: "viewing_drive".into(),
                    dimensions: vec![
                        "emotional_progression".into(),
                        "short_drama_pacing".into(),
                        "genre_fulfillment".into(),
                    ],
                },
            ],
            vec![
                "human_credibility".into(),
                "originality".into(),
                "causal_coherence".into(),
            ],
        )
        .expect("valid spec")
    }

    fn uniform(spec: &RubricSpec, score: f32) -> DimensionScores {
        spec.dimension_ids()
            .into_iter()
            .map(|id| (id, score))
            .collect()
    }

    fn scored(spec: &RubricSpec, overrides: &[(&str, f32)], base: f32) -> DimensionScores {
        let mut scores = uniform(spec, base);
        for (dimension, score) in overrides {
            scores.insert(*dimension, *score);
        }
        scores
    }

    #[test]
    fn one_low_critical_dimension_blocks_promotion() {
        let spec = drama_spec();
        let scores = scored(&spec, &[("originality", 2.0)], 4.0);
        assert!(!critical_dimensions_pass(&spec, &scores, 3.0).expect("scores present"));
    }

    #[test]
    fn geometric_mean_never_exceeds_arithmetic_mean() {
        let pillars = PillarScores::from_values(vec![2.2, 4.6, 4.7, 4.6]);
        assert!(pillars.geometric_mean() < pillars.legacy_arithmetic_mean());
    }

    /// Records a real limitation rather than asserting the design works.
    ///
    /// On a 1-5 scale the geometric mean only bites when a pillar approaches
    /// zero. A pillar at 2.2 still yields roughly 3.85, above the default pass
    /// threshold, so the *floor* is what actually rejects this artifact. The
    /// geometric mean narrows the compensation window; it does not close it.
    #[test]
    fn geometric_mean_alone_does_not_reject_a_dead_pillar() {
        let pillars = PillarScores::from_values(vec![2.2, 4.6, 4.7, 4.6]);
        assert!(pillars.geometric_mean() > DEFAULT_PASS_THRESHOLD);

        let spec = drama_spec();
        let scores = scored(
            &spec,
            &[
                ("human_credibility", 2.0),
                ("character_distinction", 2.2),
                ("dialogue_subtext", 2.4),
            ],
            4.6,
        );
        let assessment = Assessment::new(true, &spec, &scores).expect("complete scores");
        assert_eq!(
            verdict(&assessment, &Thresholds::default()).expect("complete scores"),
            Verdict::Reject,
            "the pillar floor, not the geometric mean, is doing the work here"
        );
    }

    /// The hole in the parent contract: gates were relative, so a uniformly
    /// mediocre candidate passed as long as it did not regress.
    #[test]
    fn uniformly_mediocre_artifact_lands_in_consider_not_pass() {
        let spec = drama_spec();
        let scores = uniform(&spec, 3.0);
        let assessment = Assessment::new(true, &spec, &scores).expect("complete scores");
        assert_eq!(
            verdict(&assessment, &Thresholds::default()).expect("complete scores"),
            Verdict::Consider
        );
    }

    #[test]
    fn single_dimension_at_one_rejects_despite_healthy_pillars() {
        let spec = drama_spec();
        let scores = scored(&spec, &[("continuity", 1.0)], 4.0);
        let assessment = Assessment::new(true, &spec, &scores).expect("complete scores");
        assert_eq!(
            verdict(&assessment, &Thresholds::default()).expect("complete scores"),
            Verdict::Reject
        );
    }

    #[test]
    fn failed_admission_rejects_before_any_score_matters() {
        let spec = drama_spec();
        let scores = uniform(&spec, 5.0);
        let assessment = Assessment::new(false, &spec, &scores).expect("complete scores");
        assert_eq!(
            verdict(&assessment, &Thresholds::default()).expect("complete scores"),
            Verdict::Reject
        );
    }

    #[test]
    fn strong_artifact_passes() {
        let spec = drama_spec();
        let scores = scored(&spec, &[("human_credibility", 4.5)], 4.0);
        let assessment = Assessment::new(true, &spec, &scores).expect("complete scores");
        assert_eq!(
            verdict(&assessment, &Thresholds::default()).expect("complete scores"),
            Verdict::Pass
        );
    }

    #[test]
    fn pillar_mean_of_empty_dimension_set_is_none() {
        assert!(pillar_from_dimensions(&[]).is_none());
        let mean = pillar_from_dimensions(&[3.0, 4.0, 5.0]).expect("non-empty");
        assert!((mean - 4.0).abs() < EPSILON);
    }

    #[test]
    fn malformed_non_positive_pillar_yields_zero_not_nan() {
        let pillars = PillarScores::from_values(vec![0.0, 4.0, 4.0, 4.0]);
        assert!((pillars.geometric_mean() - 0.0).abs() < EPSILON);
    }

    // --- the point of making the shape data ---

    #[test]
    fn rubric_shape_loads_from_the_repository_manifest() {
        let raw = include_str!("../../../eval/manifests/eval-v0.1.0.json");
        let manifest: Value = serde_json::from_str(raw).expect("manifest parses");
        let spec = RubricSpec::from_manifest(&manifest).expect("rubric section present");
        assert_eq!(spec.pillar_count(), 4);
        assert_eq!(spec.dimension_ids().len(), 10);
        assert_eq!(spec.critical_dimensions().len(), 3);

        let thresholds = Thresholds::from_manifest(&manifest);
        assert!((thresholds.pillar_floor - 3.0).abs() < EPSILON);
        assert!((thresholds.pass_threshold - 3.5).abs() < EPSILON);
    }

    /// A content form with a different rubric shape must need no code change.
    /// A knowledge/explainer form has no character pillar at all, which a
    /// four-field struct could not express without a recompile.
    #[test]
    fn a_three_pillar_form_aggregates_without_touching_this_crate() {
        let spec = RubricSpec::new(
            vec![
                PillarSpec {
                    name: "factual_soundness".into(),
                    dimensions: vec!["accuracy".into(), "source_traceability".into()],
                },
                PillarSpec {
                    name: "explanatory_clarity".into(),
                    dimensions: vec!["structure".into(), "example_quality".into()],
                },
                PillarSpec {
                    name: "watch_drive".into(),
                    dimensions: vec!["hook".into(), "pacing".into()],
                },
            ],
            vec!["accuracy".into()],
        )
        .expect("valid spec");

        let scores = uniform(&spec, 4.0);
        let assessment = Assessment::new(true, &spec, &scores).expect("complete scores");
        assert_eq!(assessment.pillars.len(), 3);
        assert_eq!(
            verdict(&assessment, &Thresholds::default()).expect("complete scores"),
            Verdict::Pass
        );

        let weak = scored(&spec, &[("accuracy", 2.0)], 4.0);
        let assessment = Assessment::new(true, &spec, &weak).expect("complete scores");
        assert_eq!(
            verdict(&assessment, &Thresholds::default()).expect("complete scores"),
            Verdict::Reject,
            "the critical-dimension floor applies to whatever the form declares critical"
        );
    }

    #[test]
    fn a_missing_dimension_score_is_an_error_not_a_silent_zero() {
        let spec = drama_spec();
        let mut scores = uniform(&spec, 4.0);
        scores = scores
            .values()
            .zip(spec.dimension_ids())
            .filter(|(_, id)| id != "continuity")
            .map(|(score, id)| (id, score))
            .collect();
        assert_eq!(
            PillarScores::from_scores(&spec, &scores),
            Err(RubricError::MissingScore("continuity".into()))
        );
    }

    #[test]
    fn a_pillar_with_no_dimensions_is_rejected_at_construction() {
        let error = RubricSpec::new(
            vec![PillarSpec {
                name: "empty".into(),
                dimensions: vec![],
            }],
            vec![],
        )
        .expect_err("empty pillar must not build");
        assert_eq!(error, RubricError::EmptyPillar("empty".into()));
    }
}
