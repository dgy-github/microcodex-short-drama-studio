use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use story_core::ArtifactSpanRef;
use uuid::Uuid;

const MAX_TARGETED_ROUNDS: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionKind {
    Origin,
    Targeted,
    Rollback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionRecord {
    pub schema: String,
    pub revision_id: String,
    pub job_id: String,
    pub package_id: String,
    pub supersedes_package_id: Option<String>,
    pub kind: RevisionKind,
    pub round: u8,
    pub source_run_id: String,
    pub target_span: Option<ArtifactSpanRef>,
    pub requested_change: String,
    pub content_sha256: String,
    pub created_at_unix_ms: u64,
    pub node_correspondence_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalEvent {
    pub schema: String,
    pub approval_id: String,
    pub revision_id: String,
    pub decision: ApprovalDecision,
    pub actor: String,
    pub note: String,
    pub occurred_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionSummary {
    pub schema: String,
    pub record: RevisionRecord,
    pub approval: Option<ApprovalEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionComparison {
    pub from_revision_id: String,
    pub to_revision_id: String,
    pub changed_spans: Vec<String>,
    pub removed_spans: Vec<String>,
    pub added_spans: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RevisionError {
    #[error("revision repository configuration is invalid")]
    InvalidConfig,
    #[error("story package is invalid")]
    InvalidPackage,
    #[error("revision identifier is invalid")]
    InvalidRevisionId,
    #[error("revision does not exist")]
    MissingRevision,
    #[error("artifact span does not resolve to an addressable node")]
    MissingSpan,
    #[error("replacement must preserve the cited node identity")]
    NodeIdentity,
    #[error("targeted revision limit reached; explicit input is required")]
    InputRequired,
    #[error("revision approval is already final")]
    ApprovalFinal,
    #[error("revision is not approved")]
    NotApproved,
    #[error("export target is invalid or already exists")]
    InvalidExport,
    #[error("immutable revision storage is unavailable")]
    Storage,
}

pub struct RevisionRepository {
    root: PathBuf,
    package_schema_path: PathBuf,
}

impl RevisionRepository {
    pub fn new(root: PathBuf, package_schema_path: PathBuf) -> Result<Self, RevisionError> {
        if !root.is_absolute()
            || !package_schema_path.is_absolute()
            || !package_schema_path.is_file()
        {
            return Err(RevisionError::InvalidConfig);
        }
        let repository = Self {
            root,
            package_schema_path,
        };
        repository.load_schema()?;
        Ok(repository)
    }

    pub fn ensure_origin(
        &self,
        source_run_id: &str,
        package: &Value,
    ) -> Result<RevisionSummary, RevisionError> {
        self.validate_package(package)?;
        let package_id = required_string(package, "package_id")?;
        if let Some(existing) = self
            .list()?
            .into_iter()
            .find(|summary| summary.record.package_id == package_id)
        {
            return Ok(existing);
        }
        if source_run_id.trim().is_empty() {
            return Err(RevisionError::InvalidPackage);
        }
        let record = self.build_record(
            package,
            RevisionKind::Origin,
            0,
            source_run_id,
            None,
            String::new(),
        )?;
        self.persist(&record, package)?;
        Ok(RevisionSummary {
            schema: "desktop-revision-summary/v1".into(),
            record,
            approval: None,
        })
    }

    pub fn create_targeted(
        &self,
        base_revision_id: &str,
        target_span: &ArtifactSpanRef,
        replacement: Value,
        requested_change: &str,
    ) -> Result<RevisionSummary, RevisionError> {
        if requested_change.trim().is_empty() {
            return Err(RevisionError::InvalidPackage);
        }
        let base = self.read_record(base_revision_id)?;
        let next_round = base
            .round
            .checked_add(1)
            .filter(|round| *round <= MAX_TARGETED_ROUNDS)
            .ok_or(RevisionError::InputRequired)?;
        let mut package = self.read_package(base_revision_id)?;
        let expected_node_id = target_span
            .as_str()
            .rsplit('/')
            .next()
            .ok_or(RevisionError::MissingSpan)?;
        if replacement["node_id"].as_str() != Some(expected_node_id) {
            return Err(RevisionError::NodeIdentity);
        }
        let before = collect_nodes(&package);
        let target = find_node_mut(&mut package, "story-package", target_span.as_str())
            .ok_or(RevisionError::MissingSpan)?;
        *target = replacement;
        let new_package_id = revision_package_id(&base.package_id, next_round);
        package["package_id"] = json!(new_package_id);
        package["supersedes"] = json!(base.package_id);
        let correspondence = correspondence(&before, &collect_nodes(&package));
        package["node_correspondence"] = Value::Array(correspondence.clone());
        self.validate_package(&package)?;
        let record = self.build_record(
            &package,
            RevisionKind::Targeted,
            next_round,
            &base.source_run_id,
            Some(target_span.clone()),
            requested_change.trim().to_owned(),
        )?;
        debug_assert_eq!(record.node_correspondence_count, correspondence.len());
        self.persist(&record, &package)?;
        Ok(RevisionSummary {
            schema: "desktop-revision-summary/v1".into(),
            record,
            approval: None,
        })
    }

    pub fn rollback(
        &self,
        current_revision_id: &str,
        target_revision_id: &str,
        requested_change: &str,
    ) -> Result<RevisionSummary, RevisionError> {
        if current_revision_id == target_revision_id || requested_change.trim().is_empty() {
            return Err(RevisionError::InvalidPackage);
        }
        let current_record = self.read_record(current_revision_id)?;
        let target_record = self.read_record(target_revision_id)?;
        if current_record.job_id != target_record.job_id {
            return Err(RevisionError::InvalidPackage);
        }
        let current = self.read_package(current_revision_id)?;
        let mut package = self.read_package(target_revision_id)?;
        package["package_id"] = json!(revision_package_id(
            &current_record.package_id,
            current_record.round
        ));
        package["supersedes"] = json!(current_record.package_id);
        let mapping = correspondence(&collect_nodes(&current), &collect_nodes(&package));
        package["node_correspondence"] = Value::Array(mapping.clone());
        self.validate_package(&package)?;
        let record = self.build_record(
            &package,
            RevisionKind::Rollback,
            current_record.round,
            &current_record.source_run_id,
            None,
            requested_change.trim().to_owned(),
        )?;
        debug_assert_eq!(record.node_correspondence_count, mapping.len());
        self.persist(&record, &package)?;
        Ok(RevisionSummary {
            schema: "desktop-revision-summary/v1".into(),
            record,
            approval: None,
        })
    }

    pub fn approve(
        &self,
        revision_id: &str,
        decision: ApprovalDecision,
        actor: &str,
        note: &str,
    ) -> Result<RevisionSummary, RevisionError> {
        let record = self.read_record(revision_id)?;
        if actor.trim().is_empty() || actor.len() > 128 || note.len() > 2000 {
            return Err(RevisionError::InvalidPackage);
        }
        let approval = ApprovalEvent {
            schema: "story-approval-event/v1".into(),
            approval_id: format!("approval_{}", Uuid::new_v4().simple()),
            revision_id: revision_id.to_owned(),
            decision,
            actor: actor.trim().to_owned(),
            note: note.to_owned(),
            occurred_at_unix_ms: now_ms()?,
        };
        let path = self.revision_dir(revision_id)?.join("approval.json");
        write_new_json(&path, &approval).map_err(|error| {
            if path.exists() {
                RevisionError::ApprovalFinal
            } else {
                error
            }
        })?;
        Ok(RevisionSummary {
            schema: "desktop-revision-summary/v1".into(),
            record,
            approval: Some(approval),
        })
    }

    pub fn list(&self) -> Result<Vec<RevisionSummary>, RevisionError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut summaries = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(|_| RevisionError::Storage)? {
            let entry = entry.map_err(|_| RevisionError::Storage)?;
            if !entry
                .file_type()
                .map_err(|_| RevisionError::Storage)?
                .is_dir()
            {
                continue;
            }
            let revision_id = entry.file_name().to_string_lossy().into_owned();
            if !valid_revision_id(&revision_id) {
                continue;
            }
            let record = self.read_record(&revision_id)?;
            let approval_path = entry.path().join("approval.json");
            let approval = if approval_path.is_file() {
                Some(read_json(&approval_path)?)
            } else {
                None
            };
            summaries.push(RevisionSummary {
                schema: "desktop-revision-summary/v1".into(),
                record,
                approval,
            });
        }
        summaries.sort_by(|left, right| {
            left.record
                .created_at_unix_ms
                .cmp(&right.record.created_at_unix_ms)
                .then_with(|| left.record.revision_id.cmp(&right.record.revision_id))
        });
        Ok(summaries)
    }

    pub fn read_package(&self, revision_id: &str) -> Result<Value, RevisionError> {
        let package: Value = read_json(&self.revision_dir(revision_id)?.join("package.json"))?;
        self.validate_package(&package)?;
        Ok(package)
    }

    pub fn read_span(
        &self,
        revision_id: &str,
        span: &ArtifactSpanRef,
    ) -> Result<Value, RevisionError> {
        let package = self.read_package(revision_id)?;
        find_node(&package, "story-package", span.as_str())
            .cloned()
            .ok_or(RevisionError::MissingSpan)
    }

    pub fn compare(
        &self,
        from_revision_id: &str,
        to_revision_id: &str,
    ) -> Result<RevisionComparison, RevisionError> {
        let from = collect_nodes(&self.read_package(from_revision_id)?);
        let to = collect_nodes(&self.read_package(to_revision_id)?);
        let changed_spans = from
            .iter()
            .filter(|(span, value)| to.get(*span).is_some_and(|current| current != *value))
            .map(|(span, _)| span.clone())
            .collect();
        let removed_spans = from
            .keys()
            .filter(|span| !to.contains_key(*span))
            .cloned()
            .collect();
        let added_spans = to
            .keys()
            .filter(|span| !from.contains_key(*span))
            .cloned()
            .collect();
        Ok(RevisionComparison {
            from_revision_id: from_revision_id.to_owned(),
            to_revision_id: to_revision_id.to_owned(),
            changed_spans,
            removed_spans,
            added_spans,
        })
    }

    pub fn export_approved(&self, revision_id: &str, target: &Path) -> Result<(), RevisionError> {
        if !target.is_absolute()
            || target.extension().and_then(|value| value.to_str()) != Some("json")
            || target.exists()
            || !target.parent().is_some_and(Path::is_dir)
        {
            return Err(RevisionError::InvalidExport);
        }
        let approval: ApprovalEvent =
            read_json(&self.revision_dir(revision_id)?.join("approval.json"))
                .map_err(|_| RevisionError::NotApproved)?;
        if approval.decision != ApprovalDecision::Approved {
            return Err(RevisionError::NotApproved);
        }
        let package = self.read_package(revision_id)?;
        let bytes =
            serde_json::to_vec_pretty(&package).map_err(|_| RevisionError::InvalidPackage)?;
        let temporary = target.with_extension(format!("{}.partial", Uuid::new_v4().simple()));
        write_new(&temporary, &bytes)?;
        match std::fs::rename(&temporary, target) {
            Ok(()) => Ok(()),
            Err(_) => {
                let _ = std::fs::remove_file(&temporary);
                Err(RevisionError::InvalidExport)
            }
        }
    }

    /// Export approved revision with format support (JSON, Markdown, HTML, TXT)
    pub fn export_approved_with_format(
        &self,
        revision_id: &str,
        target: &Path,
    ) -> Result<(), RevisionError> {
        use crate::export_formats::{
            package_to_html, package_to_markdown, package_to_plain_text, ExportFormat,
            ExportOptions,
        };

        // Validate path
        if !target.is_absolute() || target.exists() || !target.parent().is_some_and(Path::is_dir) {
            return Err(RevisionError::InvalidExport);
        }

        // Determine format from extension
        let format = target
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(ExportFormat::from_extension)
            .ok_or(RevisionError::InvalidExport)?;

        // Check approval status
        let approval: ApprovalEvent =
            read_json(&self.revision_dir(revision_id)?.join("approval.json"))
                .map_err(|_| RevisionError::NotApproved)?;
        if approval.decision != ApprovalDecision::Approved {
            return Err(RevisionError::NotApproved);
        }

        // Read package
        let package = self.read_package(revision_id)?;

        // Convert to target format
        let bytes = match format {
            ExportFormat::Json => {
                serde_json::to_vec_pretty(&package).map_err(|_| RevisionError::InvalidPackage)?
            }
            ExportFormat::Markdown => {
                let options = ExportOptions::default();
                let content = package_to_markdown(&package, &options)
                    .map_err(|_| RevisionError::InvalidPackage)?;
                content.into_bytes()
            }
            ExportFormat::Html => {
                let options = ExportOptions::default();
                let content = package_to_html(&package, &options)
                    .map_err(|_| RevisionError::InvalidPackage)?;
                content.into_bytes()
            }
            ExportFormat::PlainText => {
                let options = ExportOptions::default();
                let content = package_to_plain_text(&package, &options)
                    .map_err(|_| RevisionError::InvalidPackage)?;
                content.into_bytes()
            }
        };

        // Write to temporary file and rename
        let temporary = target.with_extension(format!("{}.partial", Uuid::new_v4().simple()));
        write_new(&temporary, &bytes)?;
        match std::fs::rename(&temporary, target) {
            Ok(()) => Ok(()),
            Err(_) => {
                let _ = std::fs::remove_file(&temporary);
                Err(RevisionError::InvalidExport)
            }
        }
    }

    fn build_record(
        &self,
        package: &Value,
        kind: RevisionKind,
        round: u8,
        source_run_id: &str,
        target_span: Option<ArtifactSpanRef>,
        requested_change: String,
    ) -> Result<RevisionRecord, RevisionError> {
        let bytes = canonical_bytes(package)?;
        Ok(RevisionRecord {
            schema: "story-revision-record/v1".into(),
            revision_id: format!("rev_{}", Uuid::new_v4().simple()),
            job_id: required_string(package, "job_id")?,
            package_id: required_string(package, "package_id")?,
            supersedes_package_id: package
                .get("supersedes")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            kind,
            round,
            source_run_id: source_run_id.to_owned(),
            target_span,
            requested_change,
            content_sha256: format!("{:x}", Sha256::digest(bytes)),
            created_at_unix_ms: now_ms()?,
            node_correspondence_count: package["node_correspondence"]
                .as_array()
                .map_or(0, Vec::len),
        })
    }

    fn persist(&self, record: &RevisionRecord, package: &Value) -> Result<(), RevisionError> {
        std::fs::create_dir_all(&self.root).map_err(|_| RevisionError::Storage)?;
        let directory = self.root.join(&record.revision_id);
        std::fs::create_dir(&directory).map_err(|_| RevisionError::Storage)?;
        write_new_json(&directory.join("record.json"), record)?;
        write_new_json(&directory.join("package.json"), package)
    }

    fn read_record(&self, revision_id: &str) -> Result<RevisionRecord, RevisionError> {
        read_json(&self.revision_dir(revision_id)?.join("record.json"))
    }

    fn revision_dir(&self, revision_id: &str) -> Result<PathBuf, RevisionError> {
        if !valid_revision_id(revision_id) {
            return Err(RevisionError::InvalidRevisionId);
        }
        let path = self.root.join(revision_id);
        if !path.is_dir() {
            return Err(RevisionError::MissingRevision);
        }
        Ok(path)
    }

    fn validate_package(&self, package: &Value) -> Result<(), RevisionError> {
        let schema = self.load_schema()?;
        let validator =
            jsonschema::validator_for(&schema).map_err(|_| RevisionError::InvalidConfig)?;
        if validator.is_valid(package) {
            Ok(())
        } else {
            Err(RevisionError::InvalidPackage)
        }
    }

    fn load_schema(&self) -> Result<Value, RevisionError> {
        let bytes =
            std::fs::read(&self.package_schema_path).map_err(|_| RevisionError::InvalidConfig)?;
        serde_json::from_slice(&bytes).map_err(|_| RevisionError::InvalidConfig)
    }
}

fn required_string(value: &Value, key: &str) -> Result<String, RevisionError> {
    value[key]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or(RevisionError::InvalidPackage)
}

fn canonical_bytes(value: &Value) -> Result<Vec<u8>, RevisionError> {
    serde_json::to_vec(value).map_err(|_| RevisionError::InvalidPackage)
}

fn revision_package_id(base: &str, round: u8) -> String {
    format!(
        "{base}-r{round}-{}",
        &Uuid::new_v4().simple().to_string()[..8]
    )
}

fn now_ms() -> Result<u64, RevisionError> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RevisionError::Storage)?
        .as_millis();
    u64::try_from(value).map_err(|_| RevisionError::Storage)
}

fn valid_revision_id(value: &str) -> bool {
    value.strip_prefix("rev_").is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn write_new_json<T: Serialize>(path: &Path, value: &T) -> Result<(), RevisionError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| RevisionError::Storage)?;
    write_new(path, &bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), RevisionError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| RevisionError::Storage)?;
    file.write_all(bytes).map_err(|_| RevisionError::Storage)?;
    file.sync_all().map_err(|_| RevisionError::Storage)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, RevisionError> {
    let bytes = std::fs::read(path).map_err(|_| RevisionError::MissingRevision)?;
    serde_json::from_slice(&bytes).map_err(|_| RevisionError::Storage)
}

fn collect_nodes(value: &Value) -> BTreeMap<String, Value> {
    let mut nodes = BTreeMap::new();
    collect_nodes_inner(value, "story-package", &mut nodes);
    nodes
}

fn collect_nodes_inner(value: &Value, parent: &str, nodes: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(fields) => {
            let current = fields.get("node_id").and_then(Value::as_str).map_or_else(
                || parent.to_owned(),
                |node_id| format!("{parent}/{node_id}"),
            );
            if current != parent {
                nodes.insert(current.clone(), value.clone());
            }
            for child in fields.values() {
                collect_nodes_inner(child, &current, nodes);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_nodes_inner(child, parent, nodes);
            }
        }
        _ => {}
    }
}

fn correspondence(
    previous: &BTreeMap<String, Value>,
    current: &BTreeMap<String, Value>,
) -> Vec<Value> {
    previous
        .iter()
        .map(|(span, old)| {
            let new = current.get(span);
            json!({
                "previous": span,
                "current": if new.is_some() { Value::String(span.clone()) } else { Value::String("removed".into()) },
                "changed": new != Some(old)
            })
        })
        .collect()
}

fn find_node<'a>(value: &'a Value, parent: &str, target: &str) -> Option<&'a Value> {
    let current = value
        .as_object()
        .and_then(|fields| fields.get("node_id"))
        .and_then(Value::as_str)
        .map_or_else(
            || parent.to_owned(),
            |node_id| format!("{parent}/{node_id}"),
        );
    if current == target {
        return Some(value);
    }
    match value {
        Value::Object(fields) => fields
            .values()
            .find_map(|child| find_node(child, &current, target)),
        Value::Array(items) => items
            .iter()
            .find_map(|child| find_node(child, parent, target)),
        _ => None,
    }
}

fn find_node_mut<'a>(value: &'a mut Value, parent: &str, target: &str) -> Option<&'a mut Value> {
    let current = value
        .as_object()
        .and_then(|fields| fields.get("node_id"))
        .and_then(Value::as_str)
        .map_or_else(
            || parent.to_owned(),
            |node_id| format!("{parent}/{node_id}"),
        );
    if current == target {
        return Some(value);
    }
    match value {
        Value::Object(fields) => fields
            .values_mut()
            .find_map(|child| find_node_mut(child, &current, target)),
        Value::Array(items) => items
            .iter_mut()
            .find_map(|child| find_node_mut(child, parent, target)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> (tempfile::TempDir, RevisionRepository, Value) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let package: Value = serde_json::from_slice(
            &std::fs::read(root.join(
                "eval/baselines/baseline-deepseek-v4-pro-20260727/family_001.story-package.json",
            ))
            .unwrap(),
        )
        .unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let repository = RevisionRepository::new(
            temporary.path().join("revisions"),
            root.join("schemas/story-package-v1.json"),
        )
        .unwrap();
        (temporary, repository, package)
    }

    #[test]
    fn targeted_revision_populates_correspondence_and_preserves_origin() {
        let (_temporary, repository, package) = repository();
        let origin = repository.ensure_origin("run_test", &package).unwrap();
        let span = ArtifactSpanRef::parse("story-package/logline-1").unwrap();
        let mut replacement = repository
            .read_span(&origin.record.revision_id, &span)
            .unwrap();
        replacement["text"] = json!("新的故事梗概");
        let revision = repository
            .create_targeted(
                &origin.record.revision_id,
                &span,
                replacement,
                "让梗概更明确",
            )
            .unwrap();

        assert_eq!(revision.record.kind, RevisionKind::Targeted);
        assert_eq!(revision.record.round, 1);
        assert!(revision.record.node_correspondence_count > 0);
        assert_eq!(
            repository
                .read_span(&origin.record.revision_id, &span)
                .unwrap()["text"],
            package["logline"]["text"]
        );
        assert_eq!(
            repository
                .read_span(&revision.record.revision_id, &span)
                .unwrap()["text"],
            "新的故事梗概"
        );
    }

    #[test]
    fn third_targeted_round_requires_explicit_input() {
        let (_temporary, repository, package) = repository();
        let origin = repository.ensure_origin("run_test", &package).unwrap();
        let span = ArtifactSpanRef::parse("story-package/logline-1").unwrap();
        let mut base = origin;
        for round in 1..=2 {
            let mut replacement = repository
                .read_span(&base.record.revision_id, &span)
                .unwrap();
            replacement["text"] = json!(format!("revision {round}"));
            base = repository
                .create_targeted(&base.record.revision_id, &span, replacement, "revise")
                .unwrap();
        }
        let replacement = repository
            .read_span(&base.record.revision_id, &span)
            .unwrap();
        assert!(matches!(
            repository.create_targeted(&base.record.revision_id, &span, replacement, "third"),
            Err(RevisionError::InputRequired)
        ));
    }

    #[test]
    fn approval_rollback_and_export_are_append_only() {
        let (temporary, repository, package) = repository();
        let origin = repository.ensure_origin("run_test", &package).unwrap();
        let span = ArtifactSpanRef::parse("story-package/logline-1").unwrap();
        let mut replacement = repository
            .read_span(&origin.record.revision_id, &span)
            .unwrap();
        replacement["text"] = json!("changed");
        let revision = repository
            .create_targeted(&origin.record.revision_id, &span, replacement, "change")
            .unwrap();
        let rollback = repository
            .rollback(
                &revision.record.revision_id,
                &origin.record.revision_id,
                "restore origin",
            )
            .unwrap();
        assert_eq!(rollback.record.kind, RevisionKind::Rollback);
        assert_ne!(rollback.record.package_id, origin.record.package_id);
        assert_eq!(repository.list().unwrap().len(), 3);

        let approved = repository
            .approve(
                &rollback.record.revision_id,
                ApprovalDecision::Approved,
                "operator",
                "ready",
            )
            .unwrap();
        assert_eq!(
            approved.approval.unwrap().decision,
            ApprovalDecision::Approved
        );
        assert!(matches!(
            repository.approve(
                &rollback.record.revision_id,
                ApprovalDecision::Rejected,
                "operator",
                ""
            ),
            Err(RevisionError::ApprovalFinal)
        ));
        let export = temporary.path().join("approved.json");
        repository
            .export_approved(&rollback.record.revision_id, &export)
            .unwrap();
        let exported: Value = serde_json::from_slice(&std::fs::read(export).unwrap()).unwrap();
        assert_eq!(exported["schema"], "story-package/v1");
        assert_eq!(repository.list().unwrap().len(), 3);
    }
}
