//! In-run decision policy. Deliberately separate from `story-eval`.
//!
//! Contract: `docs/ONLINE_POLICY_DESIGN.md`. This crate ranks options inside a
//! single production run under a hard budget and deadline, with no ground truth
//! and a sample size of one. It is a strategy, not a measuring instrument.
//!
//! Two properties distinguish it from offline evaluation, and both are
//! intentional:
//!
//! - ordinary defects **are** compensatory here, because the task is ranking,
//!   not certification;
//! - anything that must never be traded is a rule, never a weight, because a
//!   weight is a price and prices get paid.

use story_core::ArtifactSpanRef;

/// Shared vocabulary with offline evaluation. The vocabulary is shared; the
/// scoring is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProblemCode {
    HumanGeneric,
    MotiveExplicit,
    PlotConvenience,
    VoiceCollapse,
    EmotionUnearned,
    HookFake,
    TropeStack,
    Exposition,
    Continuity,
    Unshootable,
    SourceOverlap,
    Policy,
}

impl ProblemCode {
    /// Codes that can never be weighted, only rejected.
    pub fn is_hard_rule(self) -> bool {
        matches!(self, ProblemCode::SourceOverlap | ProblemCode::Policy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Minor,
    Major,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defect {
    pub code: ProblemCode,
    pub severity: Severity,
    /// None for artifact-wide defects; otherwise the exact repair target.
    pub span: Option<ArtifactSpanRef>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DefectWeights {
    pub human_generic: f32,
    pub motive_explicit: f32,
    pub plot_convenience: f32,
    pub voice_collapse: f32,
    pub emotion_unearned: f32,
    pub hook_fake: f32,
    pub trope_stack: f32,
    pub exposition: f32,
    pub continuity: f32,
    pub unshootable: f32,
}

impl DefectWeights {
    /// `None` for hard-rule codes: they have no price.
    pub fn weight(&self, code: ProblemCode) -> Option<f32> {
        match code {
            ProblemCode::HumanGeneric => Some(self.human_generic),
            ProblemCode::MotiveExplicit => Some(self.motive_explicit),
            ProblemCode::PlotConvenience => Some(self.plot_convenience),
            ProblemCode::VoiceCollapse => Some(self.voice_collapse),
            ProblemCode::EmotionUnearned => Some(self.emotion_unearned),
            ProblemCode::HookFake => Some(self.hook_fake),
            ProblemCode::TropeStack => Some(self.trope_stack),
            ProblemCode::Exposition => Some(self.exposition),
            ProblemCode::Continuity => Some(self.continuity),
            ProblemCode::Unshootable => Some(self.unshootable),
            ProblemCode::SourceOverlap | ProblemCode::Policy => None,
        }
    }
}

impl Default for DefectWeights {
    /// Placeholder values. Real weights are owned by `policy/online-policy-v*.json`
    /// and may only change through an offline evaluation gate.
    fn default() -> Self {
        Self {
            human_generic: 1.0,
            motive_explicit: 1.0,
            plot_convenience: 1.0,
            voice_collapse: 1.0,
            emotion_unearned: 1.0,
            hook_fake: 1.0,
            trope_stack: 1.0,
            exposition: 1.0,
            continuity: 1.0,
            unshootable: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeverityFactors {
    pub minor: f32,
    pub major: f32,
}

impl Default for SeverityFactors {
    fn default() -> Self {
        Self {
            minor: 1.0,
            major: 3.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Policy {
    pub weights: DefectWeights,
    pub severity: SeverityFactors,
    /// Applied to projected remaining cost. Sunk cost is never an input.
    pub cost_weight: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub candidate_id: String,
    /// Deliberately weak: completeness and constraint satisfaction, not taste.
    pub base_signal: f32,
    /// Projected cost to finish this lane, normalised. Never spend to date.
    pub projected_cost: f32,
    pub defects: Vec<Defect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ineligible {
    CriticalDefect,
    HardRuleCode,
}

impl Candidate {
    /// Hard rules run before any arithmetic, so no score can override them.
    pub fn ineligibility(&self) -> Option<Ineligible> {
        if self.defects.iter().any(|d| d.code.is_hard_rule()) {
            return Some(Ineligible::HardRuleCode);
        }
        if self
            .defects
            .iter()
            .any(|d| d.severity == Severity::Critical)
        {
            return Some(Ineligible::CriticalDefect);
        }
        None
    }

    pub fn major_defect_count(&self) -> usize {
        self.defects
            .iter()
            .filter(|d| d.severity == Severity::Major)
            .count()
    }

    pub fn penalty(&self, policy: &Policy) -> f32 {
        self.defects
            .iter()
            .filter_map(|defect| {
                let weight = policy.weights.weight(defect.code)?;
                let factor = match defect.severity {
                    Severity::Minor => policy.severity.minor,
                    Severity::Major => policy.severity.major,
                    Severity::Critical => return None,
                };
                Some(weight * factor)
            })
            .sum()
    }

    pub fn score(&self, policy: &Policy) -> f32 {
        self.base_signal - self.penalty(policy) - policy.cost_weight * self.projected_cost
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selected<'a> {
    pub candidate: &'a Candidate,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Selection<'a> {
    Chosen(Selected<'a>),
    AllRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionDecision {
    Complete,
    ReviseAgain,
    InputRequired,
}

/// `D3`: rank cited defects for targeted repair.
///
/// Hard-rule and critical findings are never hidden by a numeric weight.
/// Artifact-wide findings are excluded because they cannot drive a directed
/// revision until a reviewer supplies a stable span.
pub fn rank_repairs<'a>(
    defects: &'a [Defect],
    weights: &DefectWeights,
    severity: &SeverityFactors,
) -> Vec<&'a Defect> {
    let mut ranked = defects
        .iter()
        .filter(|defect| defect.span.is_some())
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        repair_class(left)
            .cmp(&repair_class(right))
            .then_with(|| {
                repair_score(right, weights, severity)
                    .total_cmp(&repair_score(left, weights, severity))
            })
            .then_with(|| {
                left.span
                    .as_ref()
                    .map(ArtifactSpanRef::as_str)
                    .cmp(&right.span.as_ref().map(ArtifactSpanRef::as_str))
            })
            .then_with(|| left.code.cmp(&right.code))
    });
    ranked
}

/// `D4`: decide whether another bounded repair round may run.
///
/// Only unresolved critical or hard-rule defects force another round. Reaching
/// the round or budget guard returns `InputRequired`; it never spends beyond
/// policy implicitly.
pub fn decide_revision(
    rounds_used: u8,
    max_rounds: u8,
    unresolved: &[Defect],
    remaining_budget_ratio: f32,
    min_remaining_budget_ratio: f32,
) -> RevisionDecision {
    let blocking = unresolved
        .iter()
        .any(|defect| defect.severity == Severity::Critical || defect.code.is_hard_rule());
    if !blocking {
        return RevisionDecision::Complete;
    }
    if rounds_used >= max_rounds
        || !(0.0..=1.0).contains(&remaining_budget_ratio)
        || !(0.0..=1.0).contains(&min_remaining_budget_ratio)
        || remaining_budget_ratio < min_remaining_budget_ratio
    {
        RevisionDecision::InputRequired
    } else {
        RevisionDecision::ReviseAgain
    }
}

fn repair_class(defect: &Defect) -> u8 {
    if defect.code.is_hard_rule() {
        0
    } else if defect.severity == Severity::Critical {
        1
    } else {
        2
    }
}

fn repair_score(defect: &Defect, weights: &DefectWeights, severity: &SeverityFactors) -> f32 {
    let factor = match defect.severity {
        Severity::Minor => severity.minor,
        Severity::Major => severity.major,
        Severity::Critical => f32::MAX,
    };
    weights.weight(defect.code).unwrap_or(f32::MAX) * factor
}

/// `D1`: pick one architecture candidate.
///
/// Determinism is a hard requirement — identical inputs must yield an identical
/// decision across retries, or the event log cannot explain a run. The tie-break
/// chain ends on `candidate_id`, which is total.
pub fn select<'a>(candidates: &'a [Candidate], policy: &Policy) -> Selection<'a> {
    let mut eligible: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| candidate.ineligibility().is_none())
        .collect();

    eligible.sort_by(|left, right| {
        right
            .score(policy)
            .total_cmp(&left.score(policy))
            .then_with(|| left.major_defect_count().cmp(&right.major_defect_count()))
            .then_with(|| left.projected_cost.total_cmp(&right.projected_cost))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });

    match eligible.first() {
        Some(candidate) => Selection::Chosen(Selected {
            candidate,
            score: candidate.score(policy),
        }),
        None => Selection::AllRejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, base: f32, defects: Vec<Defect>) -> Candidate {
        Candidate {
            candidate_id: id.into(),
            base_signal: base,
            projected_cost: 0.0,
            defects,
        }
    }

    fn defect(code: ProblemCode, severity: Severity) -> Defect {
        Defect {
            code,
            severity,
            span: None,
        }
    }

    fn chosen_id<'a>(selection: &Selection<'a>) -> &'a str {
        match selection {
            Selection::Chosen(selected) => &selected.candidate.candidate_id,
            Selection::AllRejected => panic!("expected a selection"),
        }
    }

    #[test]
    fn critical_defect_is_a_rule_not_a_price() {
        let policy = Policy::default();
        let best_but_critical = candidate(
            "a",
            100.0,
            vec![defect(ProblemCode::Continuity, Severity::Critical)],
        );
        let modest = candidate(
            "b",
            1.0,
            vec![defect(ProblemCode::Exposition, Severity::Minor)],
        );
        let pool = [best_but_critical, modest];
        assert_eq!(chosen_id(&select(&pool, &policy)), "b");
    }

    #[test]
    fn hard_rule_code_rejects_even_at_minor_severity() {
        let policy = Policy::default();
        let overlapping = candidate(
            "a",
            50.0,
            vec![defect(ProblemCode::SourceOverlap, Severity::Minor)],
        );
        assert_eq!(overlapping.ineligibility(), Some(Ineligible::HardRuleCode));
        let pool = [overlapping];
        assert_eq!(select(&pool, &policy), Selection::AllRejected);
    }

    /// Online ranking is compensatory for ordinary defects on purpose. This is
    /// the opposite of the offline pillar floors, and the difference is the
    /// point: ranking three options is not certifying one.
    #[test]
    fn ordinary_defects_compensate_against_a_stronger_base_signal() {
        let policy = Policy::default();
        let clean_but_weak = candidate("a", 4.0, vec![]);
        let strong_with_flaws = candidate(
            "b",
            8.0,
            vec![
                defect(ProblemCode::Exposition, Severity::Minor),
                defect(ProblemCode::TropeStack, Severity::Minor),
            ],
        );
        let pool = [clean_but_weak, strong_with_flaws];
        assert_eq!(chosen_id(&select(&pool, &policy)), "b");
    }

    #[test]
    fn major_severity_outweighs_several_minor_defects() {
        let policy = Policy::default();
        let one_major = candidate(
            "a",
            10.0,
            vec![defect(ProblemCode::VoiceCollapse, Severity::Major)],
        );
        let two_minor = candidate(
            "b",
            10.0,
            vec![
                defect(ProblemCode::Exposition, Severity::Minor),
                defect(ProblemCode::TropeStack, Severity::Minor),
            ],
        );
        let pool = [one_major, two_minor];
        assert_eq!(chosen_id(&select(&pool, &policy)), "b");
    }

    #[test]
    fn identical_candidates_break_ties_deterministically() {
        let policy = Policy::default();
        let pool = [
            candidate("c", 5.0, vec![]),
            candidate("a", 5.0, vec![]),
            candidate("b", 5.0, vec![]),
        ];
        assert_eq!(chosen_id(&select(&pool, &policy)), "a");
        assert_eq!(chosen_id(&select(&pool, &policy)), "a");
    }

    #[test]
    fn d3_prioritizes_hard_and_critical_cited_repairs() {
        let cited = |code, severity, index| Defect {
            code,
            severity,
            span: Some(ArtifactSpanRef::parse(format!("story-package/scene-{index}")).unwrap()),
        };
        let defects = vec![
            cited(ProblemCode::Exposition, Severity::Major, 3),
            cited(ProblemCode::Continuity, Severity::Critical, 2),
            cited(ProblemCode::Policy, Severity::Minor, 1),
            Defect {
                code: ProblemCode::MotiveExplicit,
                severity: Severity::Major,
                span: None,
            },
        ];
        let ranked = rank_repairs(
            &defects,
            &DefectWeights::default(),
            &SeverityFactors::default(),
        );
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].code, ProblemCode::Policy);
        assert_eq!(ranked[1].severity, Severity::Critical);
    }

    #[test]
    fn d4_never_silently_exceeds_round_or_budget_guard() {
        let blocking = vec![Defect {
            code: ProblemCode::Continuity,
            severity: Severity::Critical,
            span: Some(ArtifactSpanRef::parse("story-package/scene-1").unwrap()),
        }];
        assert_eq!(
            decide_revision(0, 2, &blocking, 0.8, 0.2),
            RevisionDecision::ReviseAgain
        );
        assert_eq!(
            decide_revision(2, 2, &blocking, 0.8, 0.2),
            RevisionDecision::InputRequired
        );
        assert_eq!(
            decide_revision(1, 2, &blocking, 0.1, 0.2),
            RevisionDecision::InputRequired
        );
        assert_eq!(
            decide_revision(0, 2, &[], 0.8, 0.2),
            RevisionDecision::Complete
        );
    }

    #[test]
    fn projected_cost_shifts_the_choice_when_quality_is_equal() {
        let policy = Policy {
            cost_weight: 2.0,
            ..Policy::default()
        };
        let expensive = Candidate {
            projected_cost: 3.0,
            ..candidate("a", 10.0, vec![])
        };
        let cheap = Candidate {
            projected_cost: 1.0,
            ..candidate("b", 10.0, vec![])
        };
        let pool = [expensive, cheap];
        assert_eq!(chosen_id(&select(&pool, &policy)), "b");
    }

    #[test]
    fn all_critical_pool_rejects_rather_than_picking_the_least_bad() {
        let policy = Policy::default();
        let pool = [
            candidate(
                "a",
                9.0,
                vec![defect(ProblemCode::Continuity, Severity::Critical)],
            ),
            candidate(
                "b",
                8.0,
                vec![defect(ProblemCode::HumanGeneric, Severity::Critical)],
            ),
        ];
        assert_eq!(select(&pool, &policy), Selection::AllRejected);
    }

    #[test]
    fn hard_rule_codes_have_no_weight() {
        let weights = DefectWeights::default();
        assert!(weights.weight(ProblemCode::SourceOverlap).is_none());
        assert!(weights.weight(ProblemCode::Policy).is_none());
        assert!(weights.weight(ProblemCode::HookFake).is_some());
    }

    #[test]
    fn empty_pool_rejects() {
        assert_eq!(select(&[], &Policy::default()), Selection::AllRejected);
    }

    #[test]
    fn defect_can_target_a_stable_artifact_span() {
        let value = Defect {
            code: ProblemCode::MotiveExplicit,
            severity: Severity::Major,
            span: Some(ArtifactSpanRef::parse("story-package/scene-2/dialogue-7").unwrap()),
        };
        assert_eq!(
            value.span.as_ref().map(ArtifactSpanRef::as_str),
            Some("story-package/scene-2/dialogue-7")
        );
    }
}
