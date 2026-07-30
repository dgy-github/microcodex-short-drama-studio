use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::CommandError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunSummary {
    pub schema: &'static str,
    pub run_id: String,
    pub job_id: String,
    pub status: String,
    pub promotion: String,
    pub generation_model: String,
    pub review_model: String,
    pub task_count: usize,
    pub review_count: usize,
    pub episode_count: usize,
    pub logline: String,
    pub completed_at_unix_ms: u64,
}

#[derive(Deserialize)]
struct WorkflowProjection {
    schema: String,
    run_id: String,
    job_id: String,
    status: String,
    promotion: String,
    tasks: Vec<Value>,
    reviews: Vec<Value>,
    package: PackageProjection,
    provider_routes: ProviderRoutes,
}

#[derive(Deserialize)]
struct PackageProjection {
    episodes: Vec<Value>,
    logline: LoglineProjection,
}

#[derive(Deserialize)]
struct LoglineProjection {
    text: String,
}

#[derive(Deserialize)]
struct ProviderRoutes {
    generation: String,
    review: String,
}

pub struct ArtifactRepository {
    root: PathBuf,
}

impl ArtifactRepository {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn list(&self) -> Result<Vec<RunSummary>, CommandError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut summaries = std::fs::read_dir(&self.root)
            .map_err(|_| CommandError::artifact_unavailable())?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter_map(|entry| {
                let run_id = entry.file_name().to_string_lossy().into_owned();
                self.read_summary(&run_id).ok()
            })
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            right
                .completed_at_unix_ms
                .cmp(&left.completed_at_unix_ms)
                .then_with(|| right.run_id.cmp(&left.run_id))
        });
        Ok(summaries)
    }

    pub fn read(&self, run_id: &str) -> Result<Value, CommandError> {
        let path = self.resolve(run_id)?;
        let value: Value = serde_json::from_slice(
            &std::fs::read(path).map_err(|_| CommandError::artifact_missing())?,
        )
        .map_err(|_| CommandError::artifact_invalid())?;
        let projection = parse_projection(&value)?;
        if projection.run_id != run_id {
            return Err(CommandError::artifact_invalid());
        }
        Ok(value)
    }

    pub fn write(&self, run_id: &str, value: &Value) -> Result<(), CommandError> {
        if !valid_run_id(run_id) {
            return Err(CommandError::invalid_run_id());
        }
        let projection = parse_projection(value)?;
        if projection.run_id != run_id {
            return Err(CommandError::artifact_invalid());
        }
        std::fs::create_dir_all(&self.root).map_err(|_| CommandError::artifact_unavailable())?;
        let run_dir = self.root.join(run_id);
        std::fs::create_dir_all(&run_dir).map_err(|_| CommandError::artifact_unavailable())?;
        let target = run_dir.join("workflow-result.json");
        let temporary = run_dir.join("workflow-result.partial.json");
        let bytes =
            serde_json::to_vec_pretty(value).map_err(|_| CommandError::artifact_invalid())?;
        std::fs::write(&temporary, bytes).map_err(|_| CommandError::artifact_unavailable())?;
        if target.exists() {
            std::fs::remove_file(&target).map_err(|_| CommandError::artifact_unavailable())?;
        }
        std::fs::rename(temporary, target).map_err(|_| CommandError::artifact_unavailable())
    }

    /// Persist what a failed run produced.
    ///
    /// Success and failure are written to different files with different
    /// schemas so the reading paths stay separate: `read`, `list` and
    /// `read_summary` all assume a completed workflow result and must not start
    /// seeing partial runs.
    ///
    /// This exists because the previous behaviour discarded everything a failed
    /// run had generated. A run that completed sixteen of seventeen tasks and
    /// five reviews left no trace, so the failure could not be diagnosed
    /// afterwards and the paid output was lost. That is the same mistake the
    /// online policy contract already forbids for rejected `t06` candidates.
    pub fn write_failure(&self, run_id: &str, value: &Value) -> Result<(), CommandError> {
        if !valid_run_id(run_id) {
            return Err(CommandError::invalid_run_id());
        }
        if value.get("schema").and_then(Value::as_str) != Some("story-run-failure/v1")
            || value.get("run_id").and_then(Value::as_str) != Some(run_id)
        {
            return Err(CommandError::artifact_invalid());
        }
        std::fs::create_dir_all(&self.root).map_err(|_| CommandError::artifact_unavailable())?;
        let run_dir = self.root.join(run_id);
        std::fs::create_dir_all(&run_dir).map_err(|_| CommandError::artifact_unavailable())?;
        let target = run_dir.join("run-failure.json");
        let temporary = run_dir.join("run-failure.partial.json");
        let bytes =
            serde_json::to_vec_pretty(value).map_err(|_| CommandError::artifact_invalid())?;
        std::fs::write(&temporary, bytes).map_err(|_| CommandError::artifact_unavailable())?;
        if target.exists() {
            std::fs::remove_file(&target).map_err(|_| CommandError::artifact_unavailable())?;
        }
        std::fs::rename(temporary, target).map_err(|_| CommandError::artifact_unavailable())
    }

    fn read_summary(&self, run_id: &str) -> Result<RunSummary, CommandError> {
        let completed_at_unix_ms = std::fs::metadata(self.resolve(run_id)?)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
            .unwrap_or(0);
        let value = self.read(run_id)?;
        let projection = parse_projection(&value)?;
        Ok(RunSummary {
            schema: "desktop-run-summary/v1",
            run_id: projection.run_id,
            job_id: projection.job_id,
            status: projection.status,
            promotion: projection.promotion,
            generation_model: projection.provider_routes.generation,
            review_model: projection.provider_routes.review,
            task_count: projection.tasks.len(),
            review_count: projection.reviews.len(),
            episode_count: projection.package.episodes.len(),
            logline: projection.package.logline.text,
            completed_at_unix_ms,
        })
    }

    fn resolve(&self, run_id: &str) -> Result<PathBuf, CommandError> {
        if !valid_run_id(run_id) {
            return Err(CommandError::invalid_run_id());
        }
        let root = self
            .root
            .canonicalize()
            .map_err(|_| CommandError::artifact_unavailable())?;
        let run_dir = root.join(run_id);
        let canonical = run_dir
            .canonicalize()
            .map_err(|_| CommandError::artifact_missing())?;
        if !canonical.starts_with(&root) {
            return Err(CommandError::invalid_run_id());
        }
        Ok(canonical.join("workflow-result.json"))
    }
}

fn parse_projection(value: &Value) -> Result<WorkflowProjection, CommandError> {
    let projection: WorkflowProjection =
        serde_json::from_value(value.clone()).map_err(|_| CommandError::artifact_invalid())?;
    if projection.schema != "story-workflow-result/v1"
        || projection.status != "advisory"
        || projection.promotion != "non-promotable"
    {
        return Err(CommandError::artifact_invalid());
    }
    Ok(projection)
}

fn valid_run_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("run_") else {
        return false;
    };
    (16..=64).contains(&suffix.len())
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

pub fn default_artifact_root() -> PathBuf {
    std::env::var_os("MICROCODEX_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join("artifacts/advisory-runs")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_ids_are_bounded_and_path_safe() {
        assert!(valid_run_id("run_0148aa190ce842c8b103d3885a68dfcb"));
        assert!(!valid_run_id("../workflow-result.json"));
        assert!(!valid_run_id("run_A148aa190ce842c8"));
        assert!(!valid_run_id("run_short"));
    }

    #[test]
    fn invalid_workflow_projection_fails_closed() {
        let value = serde_json::json!({
            "schema": "story-workflow-result/v1",
            "run_id": "run_0148aa190ce842c8",
            "job_id": "job_1",
            "status": "released",
            "promotion": "promotable",
            "tasks": [],
            "reviews": [],
            "package": {"episodes": [], "logline": {"text": "测试故事"}},
            "provider_routes": {"generation": "a", "review": "b"}
        });
        assert!(parse_projection(&value).is_err());
    }

    fn failure_record(run_id: &str) -> Value {
        serde_json::json!({
            "schema": "story-run-failure/v1",
            "run_id": run_id,
            "job_id": "job_1",
            "status": "failed",
            "error": "fixed workflow failed: final_review_rejected at t17",
            "tasks_total": 17,
            "tasks_completed": 16,
            "reviews_completed": 5,
            "events": []
        })
    }

    /// A failed run used to leave nothing on disk, so sixteen completed task
    /// artifacts and five reviews were discarded and the failure could not be
    /// diagnosed afterwards.
    #[test]
    fn a_failed_run_is_persisted_instead_of_discarded() {
        let dir = std::env::temp_dir().join(format!("mx-fail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repository = ArtifactRepository::new(dir.clone());
        let run_id = "run_0148aa190ce842c8b103d3885a68dfcb";

        repository
            .write_failure(run_id, &failure_record(run_id))
            .expect("failure record is written");

        let stored = dir.join(run_id).join("run-failure.json");
        assert!(stored.exists(), "run-failure.json must exist");
        let value: Value =
            serde_json::from_slice(&std::fs::read(&stored).unwrap()).unwrap();
        assert_eq!(value["tasks_completed"], 16);
        assert!(value["error"].as_str().unwrap().contains("final_review_rejected"));

        // The success path must not start seeing partial runs.
        assert!(repository.read(run_id).is_err());
        assert!(repository.list().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failure_records_reject_a_mismatched_schema_or_run_id() {
        let dir = std::env::temp_dir().join(format!("mx-fail-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repository = ArtifactRepository::new(dir.clone());
        let run_id = "run_0148aa190ce842c8b103d3885a68dfcb";

        let mut wrong_schema = failure_record(run_id);
        wrong_schema["schema"] = Value::from("story-workflow-result/v1");
        assert!(repository.write_failure(run_id, &wrong_schema).is_err());

        let other = failure_record("run_28f176fffad642f7ab70fee5f7e74f84");
        assert!(repository.write_failure(run_id, &other).is_err());

        assert!(repository.write_failure("../escape", &failure_record("../escape")).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
