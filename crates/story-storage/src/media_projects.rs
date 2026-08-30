//! Append-only prompt revision and generation request history for media projects.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImagePromptRevision {
    pub schema: String,
    pub project_id: String,
    pub revision_id: String,
    pub parent_revision_id: Option<String>,
    pub prompt: String,
    pub source_spans: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaProjectRecord {
    pub schema: String,
    pub seq: u64,
    pub project_id: String,
    pub record_id: String,
    pub record_type: String,
    pub data: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum MediaProjectError {
    #[error("media project record is invalid")]
    InvalidRecord,
    #[error("media project record already exists")]
    DuplicateRecord,
    #[error("media prompt parent revision is missing")]
    MissingParent,
    #[error("media project history is corrupt")]
    Corrupt,
    #[error("media project history lock failed")]
    Lock,
    #[error("media project history io failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("media project encoding failed: {0}")]
    Encoding(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct MediaProjectRepository {
    root: PathBuf,
    projects: Arc<Mutex<HashMap<String, ProjectState>>>,
}

#[derive(Default)]
struct ProjectState {
    records: Vec<MediaProjectRecord>,
    ids: HashSet<String>,
    prompt_revisions: HashSet<String>,
}

impl ProjectState {
    fn from_records(records: Vec<MediaProjectRecord>) -> Result<Self, MediaProjectError> {
        let mut state = Self {
            records: Vec::new(),
            ids: HashSet::new(),
            prompt_revisions: HashSet::new(),
        };
        for record in records {
            state.insert(record)?;
        }
        Ok(state)
    }

    fn insert(&mut self, record: MediaProjectRecord) -> Result<(), MediaProjectError> {
        if record.seq != self.records.len() as u64 + 1 || !self.ids.insert(record.record_id.clone())
        {
            return Err(MediaProjectError::Corrupt);
        }
        if record.record_type == "image_prompt_revision" {
            self.prompt_revisions.insert(record.record_id.clone());
        }
        self.records.push(record);
        Ok(())
    }
}

impl MediaProjectRepository {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, MediaProjectError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            projects: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn append_prompt_revision(
        &self,
        revision: &ImagePromptRevision,
    ) -> Result<MediaProjectRecord, MediaProjectError> {
        validate_revision(revision)?;
        let mut projects = self.projects.lock().map_err(|_| MediaProjectError::Lock)?;
        let state = self.load_project(&mut projects, &revision.project_id)?;
        if state.ids.contains(&revision.revision_id) {
            return Err(MediaProjectError::DuplicateRecord);
        }
        if let Some(parent) = &revision.parent_revision_id {
            if !state.prompt_revisions.contains(parent) {
                return Err(MediaProjectError::MissingParent);
            }
        }
        self.append_unlocked(
            state,
            &revision.project_id,
            &revision.revision_id,
            "image_prompt_revision",
            serde_json::to_value(revision)?,
        )
    }

    pub fn append_generation_request(
        &self,
        project_id: &str,
        request_id: &str,
        request: &Value,
    ) -> Result<MediaProjectRecord, MediaProjectError> {
        if !valid_id(project_id)
            || !valid_id(request_id)
            || !matches!(
                request["schema"].as_str(),
                Some("image-generation-request/v1" | "video-generation-request/v1")
            )
            || request["project_id"] != project_id
            || request["request_id"] != request_id
        {
            return Err(MediaProjectError::InvalidRecord);
        }
        let mut projects = self.projects.lock().map_err(|_| MediaProjectError::Lock)?;
        let state = self.load_project(&mut projects, project_id)?;
        if state.ids.contains(request_id) {
            return Err(MediaProjectError::DuplicateRecord);
        }
        if request["schema"] == "image-generation-request/v1" {
            let revision_id = request["prompt_revision_id"]
                .as_str()
                .ok_or(MediaProjectError::InvalidRecord)?;
            if !state.prompt_revisions.contains(revision_id) {
                return Err(MediaProjectError::MissingParent);
            }
        }
        self.append_unlocked(
            state,
            project_id,
            request_id,
            "generation_request",
            request.clone(),
        )
    }

    pub fn history(&self, project_id: &str) -> Result<Vec<MediaProjectRecord>, MediaProjectError> {
        if !valid_id(project_id) {
            return Err(MediaProjectError::InvalidRecord);
        }
        let mut projects = self.projects.lock().map_err(|_| MediaProjectError::Lock)?;
        Ok(self
            .load_project(&mut projects, project_id)?
            .records
            .clone())
    }

    fn append_unlocked(
        &self,
        state: &mut ProjectState,
        project_id: &str,
        record_id: &str,
        record_type: &str,
        data: Value,
    ) -> Result<MediaProjectRecord, MediaProjectError> {
        let record = MediaProjectRecord {
            schema: "media-project-record/v1".into(),
            seq: state
                .records
                .last()
                .map_or(1, |record| record.seq.saturating_add(1)),
            project_id: project_id.into(),
            record_id: record_id.into(),
            record_type: record_type.into(),
            data,
        };
        let path = self.path(project_id);
        let mut line = serde_json::to_vec(&record)?;
        line.push(b'\n');
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(&line)?;
        file.sync_all()?;
        state.insert(record.clone())?;
        Ok(record)
    }

    fn load_project<'a>(
        &self,
        projects: &'a mut HashMap<String, ProjectState>,
        project_id: &str,
    ) -> Result<&'a mut ProjectState, MediaProjectError> {
        if !projects.contains_key(project_id) {
            projects.insert(
                project_id.into(),
                ProjectState::from_records(self.read_project(project_id)?)?,
            );
        }
        projects
            .get_mut(project_id)
            .ok_or(MediaProjectError::Corrupt)
    }

    fn read_project(&self, project_id: &str) -> Result<Vec<MediaProjectRecord>, MediaProjectError> {
        let path = self.path(project_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut ids = HashSet::new();
        let mut records = Vec::new();
        for line in fs::read_to_string(path)?.lines() {
            let record: MediaProjectRecord =
                serde_json::from_str(line).map_err(|_| MediaProjectError::Corrupt)?;
            if record.schema != "media-project-record/v1"
                || record.project_id != project_id
                || record.seq != records.len() as u64 + 1
                || !valid_id(&record.record_id)
                || !ids.insert(record.record_id.clone())
            {
                return Err(MediaProjectError::Corrupt);
            }
            records.push(record);
        }
        Ok(records)
    }

    fn path(&self, project_id: &str) -> PathBuf {
        self.root.join(format!("{project_id}.jsonl"))
    }
}

fn validate_revision(revision: &ImagePromptRevision) -> Result<(), MediaProjectError> {
    if revision.schema != "image-prompt-revision/v1"
        || !valid_id(&revision.project_id)
        || !valid_id(&revision.revision_id)
        || revision
            .parent_revision_id
            .as_ref()
            .is_some_and(|value| !valid_id(value))
        || revision.prompt.trim().is_empty()
        || revision.prompt.len() > 20_000
        || revision.source_spans.is_empty()
        || revision
            .source_spans
            .iter()
            .any(|span| span.trim().is_empty() || span.len() > 512)
    {
        return Err(MediaProjectError::InvalidRecord);
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn revisions_and_requests_survive_reopen_without_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let repository = MediaProjectRepository::open(directory.path()).unwrap();
        let first = ImagePromptRevision {
            schema: "image-prompt-revision/v1".into(),
            project_id: "project_1".into(),
            revision_id: "prompt_1".into(),
            parent_revision_id: None,
            prompt: "厨房白天，母女隔桌对视".into(),
            source_spans: vec!["story-package/scene-1".into()],
        };
        repository.append_prompt_revision(&first).unwrap();
        let second = ImagePromptRevision {
            revision_id: "prompt_2".into(),
            parent_revision_id: Some("prompt_1".into()),
            prompt: "厨房夜晚，钥匙位于前景".into(),
            ..first.clone()
        };
        repository.append_prompt_revision(&second).unwrap();
        repository
            .append_generation_request(
                "project_1",
                "img_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                &json!({
                    "schema":"image-generation-request/v1",
                    "request_id":"img_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "project_id":"project_1",
                    "prompt_revision_id":"prompt_2",
                    "prompt":second.prompt,
                    "source_spans":second.source_spans
                }),
            )
            .unwrap();
        let reopened = MediaProjectRepository::open(directory.path()).unwrap();
        let history = reopened.history("project_1").unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].data["prompt"], first.prompt);
        assert!(reopened.append_prompt_revision(&first).is_err());
    }

    #[test]
    fn missing_parent_and_untracked_prompt_request_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let repository = MediaProjectRepository::open(directory.path()).unwrap();
        let orphan = ImagePromptRevision {
            schema: "image-prompt-revision/v1".into(),
            project_id: "project_1".into(),
            revision_id: "prompt_2".into(),
            parent_revision_id: Some("prompt_missing".into()),
            prompt: "有效提示词".into(),
            source_spans: vec!["story-package/scene-1".into()],
        };
        assert!(matches!(
            repository.append_prompt_revision(&orphan),
            Err(MediaProjectError::MissingParent)
        ));
    }

    #[test]
    fn long_revision_history_reopens_with_parent_and_duplicate_indexes() {
        let directory = tempfile::tempdir().unwrap();
        let repository = MediaProjectRepository::open(directory.path()).unwrap();
        for index in 0..250 {
            repository
                .append_prompt_revision(&ImagePromptRevision {
                    schema: "image-prompt-revision/v1".into(),
                    project_id: "project_long".into(),
                    revision_id: format!("prompt_{index}"),
                    parent_revision_id: (index > 0).then(|| format!("prompt_{}", index - 1)),
                    prompt: format!("第 {index} 版画面提示词"),
                    source_spans: vec!["story-package/scene-1".into()],
                })
                .unwrap();
        }
        let reopened = MediaProjectRepository::open(directory.path()).unwrap();
        assert_eq!(reopened.history("project_long").unwrap().len(), 250);
        assert!(matches!(
            reopened.append_prompt_revision(&ImagePromptRevision {
                schema: "image-prompt-revision/v1".into(),
                project_id: "project_long".into(),
                revision_id: "prompt_249".into(),
                parent_revision_id: Some("prompt_248".into()),
                prompt: "重复修订".into(),
                source_spans: vec!["story-package/scene-1".into()],
            }),
            Err(MediaProjectError::DuplicateRecord)
        ));
    }
}
