use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use story_core::ArtifactSpanRef;
use story_storage::{
    ApprovalDecision, RevisionComparison, RevisionError, RevisionRepository, RevisionSummary,
};

use crate::CommandError;

#[derive(Debug, Clone, Serialize)]
pub struct RevisionWorkspace {
    pub run_id: String,
    pub job_id: String,
    pub revisions: Vec<RevisionSummary>,
    pub findings: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportReceipt {
    pub revision_id: String,
    pub target_path: String,
    pub status: &'static str,
}

pub struct RevisionService {
    repository: Mutex<RevisionRepository>,
}

impl RevisionService {
    pub fn new(repository_root: &Path) -> Result<Self, CommandError> {
        let revision_root = std::env::var_os("MICROCODEX_REVISION_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| repository_root.join("artifacts/revisions"));
        Self::new_at(repository_root, revision_root)
    }

    fn new_at(repository_root: &Path, revision_root: PathBuf) -> Result<Self, CommandError> {
        let repository = RevisionRepository::new(
            revision_root,
            repository_root.join("schemas/story-package-v1.json"),
        )
        .map_err(map_revision_error)?;
        Ok(Self {
            repository: Mutex::new(repository),
        })
    }

    pub fn open(&self, run_id: &str, workflow: &Value) -> Result<RevisionWorkspace, CommandError> {
        let package = workflow
            .get("package")
            .ok_or_else(CommandError::artifact_invalid)?;
        let job_id = package["job_id"]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(CommandError::artifact_invalid)?
            .to_owned();
        let repository = self.lock()?;
        repository
            .ensure_origin(run_id, package)
            .map_err(map_revision_error)?;
        let revisions = repository
            .list()
            .map_err(map_revision_error)?
            .into_iter()
            .filter(|summary| summary.record.job_id == job_id)
            .collect();
        let findings = workflow["reviews"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|review| review["findings"].as_array().into_iter().flatten())
            .filter(|finding| finding["span_ref"].as_str().is_some())
            .cloned()
            .collect();
        Ok(RevisionWorkspace {
            run_id: run_id.to_owned(),
            job_id,
            revisions,
            findings,
        })
    }

    pub fn read_span(&self, revision_id: &str, span: &str) -> Result<Value, CommandError> {
        let span = ArtifactSpanRef::parse(span).map_err(|_| CommandError::invalid_revision())?;
        self.lock()?
            .read_span(revision_id, &span)
            .map_err(map_revision_error)
    }

    pub fn create(
        &self,
        base_revision_id: &str,
        span: &str,
        replacement: Value,
        requested_change: &str,
    ) -> Result<RevisionSummary, CommandError> {
        let span = ArtifactSpanRef::parse(span).map_err(|_| CommandError::invalid_revision())?;
        self.lock()?
            .create_targeted(base_revision_id, &span, replacement, requested_change)
            .map_err(map_revision_error)
    }

    pub fn approve(
        &self,
        revision_id: &str,
        decision: &str,
        actor: &str,
        note: &str,
    ) -> Result<RevisionSummary, CommandError> {
        let decision = match decision {
            "approved" => ApprovalDecision::Approved,
            "rejected" => ApprovalDecision::Rejected,
            _ => return Err(CommandError::invalid_revision()),
        };
        self.lock()?
            .approve(revision_id, decision, actor, note)
            .map_err(map_revision_error)
    }

    pub fn compare(
        &self,
        from_revision_id: &str,
        to_revision_id: &str,
    ) -> Result<RevisionComparison, CommandError> {
        self.lock()?
            .compare(from_revision_id, to_revision_id)
            .map_err(map_revision_error)
    }

    pub fn rollback(
        &self,
        current_revision_id: &str,
        target_revision_id: &str,
        requested_change: &str,
    ) -> Result<RevisionSummary, CommandError> {
        self.lock()?
            .rollback(current_revision_id, target_revision_id, requested_change)
            .map_err(map_revision_error)
    }

    pub fn export(
        &self,
        revision_id: &str,
        target_path: &str,
    ) -> Result<ExportReceipt, CommandError> {
        let target = PathBuf::from(target_path);

        // Use new format-aware export if extension is supported
        let extension = target.extension().and_then(|e| e.to_str()).unwrap_or("");
        let result = if matches!(extension, "md" | "markdown" | "html" | "htm" | "txt") {
            self.lock()?
                .export_approved_with_format(revision_id, &target)
        } else {
            self.lock()?.export_approved(revision_id, &target)
        };

        result.map_err(map_revision_error)?;

        Ok(ExportReceipt {
            revision_id: revision_id.to_owned(),
            target_path: target.to_string_lossy().into_owned(),
            status: "exported",
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, RevisionRepository>, CommandError> {
        self.repository
            .lock()
            .map_err(|_| CommandError::revision_unavailable())
    }
}

fn map_revision_error(error: RevisionError) -> CommandError {
    match error {
        RevisionError::InputRequired => CommandError::revision_limit(),
        RevisionError::MissingSpan => CommandError::span_missing(),
        RevisionError::ApprovalFinal => CommandError::approval_final(),
        RevisionError::NotApproved => CommandError::revision_not_approved(),
        RevisionError::InvalidExport => CommandError::invalid_export(),
        RevisionError::InvalidRevisionId
        | RevisionError::InvalidPackage
        | RevisionError::NodeIdentity => CommandError::invalid_revision(),
        RevisionError::InvalidConfig | RevisionError::MissingRevision | RevisionError::Storage => {
            CommandError::revision_unavailable()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn desktop_service_opens_real_package_and_creates_cited_revision() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let revision_root = repository_root
            .join("target/desktop-revision-tests")
            .join(Uuid::new_v4().simple().to_string());
        let service = RevisionService::new_at(&repository_root, revision_root).unwrap();
        let package: Value = serde_json::from_slice(
            &std::fs::read(repository_root.join(
                "eval/baselines/baseline-deepseek-v4-pro-20260727/family_001.story-package.json",
            ))
            .unwrap(),
        )
        .unwrap();
        let workflow = serde_json::json!({
            "package": package,
            "reviews": [{
                "findings": [{
                    "defect_id": "defect_desktop_1",
                    "severity": "major",
                    "span_ref": "story-package/logline-1",
                    "evidence": "test",
                    "requested_change": "revise logline"
                }]
            }]
        });
        let workspace = service
            .open("run_desktop_revision_test", &workflow)
            .unwrap();
        assert_eq!(workspace.revisions.len(), 1);
        assert_eq!(workspace.findings.len(), 1);
        let origin = &workspace.revisions[0].record.revision_id;
        let mut replacement = service
            .read_span(origin, "story-package/logline-1")
            .unwrap();
        replacement["text"] = serde_json::json!("desktop revision");
        let revised = service
            .create(
                origin,
                "story-package/logline-1",
                replacement,
                "revise logline",
            )
            .unwrap();
        assert_eq!(revised.record.round, 1);
    }
}
