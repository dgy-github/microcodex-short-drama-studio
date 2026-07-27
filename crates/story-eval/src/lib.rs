//! Deterministic hard gates used before expensive model and human review.

#[derive(Debug, Clone, PartialEq)]
pub struct CriticalScores {
    pub human_credibility: f32,
    pub originality: f32,
    pub causal_coherence: f32,
}

pub fn critical_dimensions_pass(scores: &CriticalScores, minimum: f32) -> bool {
    [
        scores.human_credibility,
        scores.originality,
        scores.causal_coherence,
    ]
    .into_iter()
    .all(|score| score >= minimum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_low_critical_dimension_blocks_promotion() {
        let scores = CriticalScores {
            human_credibility: 4.0,
            originality: 2.0,
            causal_coherence: 4.0,
        };
        assert!(!critical_dimensions_pass(&scores, 3.0));
    }
}
