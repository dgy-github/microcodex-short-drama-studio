//! Storage boundary for product jobs, projections, immutable artifacts, and rights metadata.

mod export_formats;
mod operations;
mod revisions;

pub use export_formats::{
    package_to_html, package_to_markdown, package_to_plain_text, ExportFormat, ExportOptions,
};
pub use operations::{
    create_backup, migrate_store, repair_store, restore_backup, BackupFile, BackupManifest,
    RepairReport, StoreOperationError,
};
pub use revisions::{
    ApprovalDecision, ApprovalEvent, RevisionComparison, RevisionError, RevisionKind,
    RevisionRecord, RevisionRepository, RevisionSummary,
};

use std::collections::HashSet;

pub trait ArtifactStore {
    type Error;

    fn put_if_absent(&mut self, content_hash: &str, bytes: &[u8]) -> Result<(), Self::Error>;
    fn get(&self, content_hash: &str) -> Result<Option<Vec<u8>>, Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateDisposition {
    Selected,
    Rejected,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateDecisionEntry {
    pub candidate_id: String,
    pub artifact_hash: String,
    pub online_score: f32,
    /// One-based deterministic rank after policy tie-breaking.
    pub rank: u32,
    pub disposition: CandidateDisposition,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateDecisionTrace {
    pub job_id: String,
    pub run_id: String,
    /// Idempotency key for one decision point, for example `t06`.
    pub decision_id: String,
    pub policy_version: String,
    pub candidates: Vec<CandidateDecisionEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateTraceError {
    MissingIdentity,
    TooFewCandidates,
    SelectedCount,
    DuplicateCandidateId,
    MissingArtifactHash,
    InvalidScore,
    InvalidRank,
    DuplicateRank,
    NonContiguousRanks,
}

impl CandidateDecisionTrace {
    /// Reject incomplete traces before they can permanently bias proxy fidelity.
    pub fn validate(&self) -> Result<(), CandidateTraceError> {
        if [
            self.job_id.as_str(),
            self.run_id.as_str(),
            self.decision_id.as_str(),
            self.policy_version.as_str(),
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        {
            return Err(CandidateTraceError::MissingIdentity);
        }
        if self.candidates.len() < 2 {
            return Err(CandidateTraceError::TooFewCandidates);
        }
        if self
            .candidates
            .iter()
            .filter(|entry| entry.disposition == CandidateDisposition::Selected)
            .count()
            != 1
        {
            return Err(CandidateTraceError::SelectedCount);
        }

        let mut candidate_ids = HashSet::new();
        let mut ranks = HashSet::new();
        for entry in &self.candidates {
            if !candidate_ids.insert(entry.candidate_id.as_str()) {
                return Err(CandidateTraceError::DuplicateCandidateId);
            }
            if entry.artifact_hash.trim().is_empty() {
                return Err(CandidateTraceError::MissingArtifactHash);
            }
            if !entry.online_score.is_finite() {
                return Err(CandidateTraceError::InvalidScore);
            }
            if entry.rank == 0 {
                return Err(CandidateTraceError::InvalidRank);
            }
            if !ranks.insert(entry.rank) {
                return Err(CandidateTraceError::DuplicateRank);
            }
        }
        if !(1..=self.candidates.len() as u32).all(|rank| ranks.contains(&rank)) {
            return Err(CandidateTraceError::NonContiguousRanks);
        }
        Ok(())
    }
}

/// Durable, atomic retention boundary for one complete online decision.
///
/// Implementations use `decision_id` as an idempotency key. They must never
/// persist a subset of `candidates`, because missing losers make future
/// proxy-fidelity measurements invalid.
pub trait CandidateDecisionStore {
    type Error;

    fn put_if_absent(&mut self, trace: &CandidateDecisionTrace) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, rank: u32, disposition: CandidateDisposition) -> CandidateDecisionEntry {
        CandidateDecisionEntry {
            candidate_id: id.into(),
            artifact_hash: format!("sha256-{id}"),
            online_score: 10.0 - rank as f32,
            rank,
            disposition,
        }
    }

    fn trace() -> CandidateDecisionTrace {
        CandidateDecisionTrace {
            job_id: "job-1".into(),
            run_id: "run-1".into(),
            decision_id: "t06".into(),
            policy_version: "online-policy/v1.0.0".into(),
            candidates: vec![
                entry("a", 1, CandidateDisposition::Selected),
                entry("b", 2, CandidateDisposition::Rejected),
                entry("c", 3, CandidateDisposition::Rejected),
            ],
        }
    }

    #[test]
    fn complete_trace_is_valid() {
        assert_eq!(trace().validate(), Ok(()));
    }

    #[test]
    fn exactly_one_candidate_must_be_selected() {
        let mut value = trace();
        value.candidates[1].disposition = CandidateDisposition::Selected;
        assert_eq!(value.validate(), Err(CandidateTraceError::SelectedCount));
    }

    #[test]
    fn candidate_ids_must_be_unique() {
        let mut value = trace();
        value.candidates[1].candidate_id = "a".into();
        assert_eq!(
            value.validate(),
            Err(CandidateTraceError::DuplicateCandidateId)
        );
    }

    #[test]
    fn ranks_must_be_unique_and_contiguous() {
        let mut duplicate = trace();
        duplicate.candidates[1].rank = 1;
        assert_eq!(
            duplicate.validate(),
            Err(CandidateTraceError::DuplicateRank)
        );

        let mut gap = trace();
        gap.candidates[2].rank = 4;
        assert_eq!(gap.validate(), Err(CandidateTraceError::NonContiguousRanks));
    }

    #[test]
    fn scores_must_be_finite() {
        let mut value = trace();
        value.candidates[0].online_score = f32::NAN;
        assert_eq!(value.validate(), Err(CandidateTraceError::InvalidScore));
    }
}
