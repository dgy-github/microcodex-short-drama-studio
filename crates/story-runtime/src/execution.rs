use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionTask {
    pub id: &'static str,
    pub name: &'static str,
    pub depends_on: &'static [&'static str],
}

// Fixed 17-task story graph. This MUST stay in lock-step with the Python copy
// in `sidecar/campaign_adapter/workflow/graph.py` (`TASKS`): identical task ids,
// identical ordering, identical `depends_on`. `validate_fixed_story_execution_order`
// guards this copy; the Python `validate_task_graph()` guards the other, and
// `test_task_graph_matches_rust_order` in test_workflow.py pins both.
const FIXED_STORY_EXECUTION_ORDER: [ExecutionTask; 17] = [
    ExecutionTask {
        id: "t01",
        name: "classify_genre",
        depends_on: &[],
    },
    ExecutionTask {
        id: "t02",
        name: "retrieve_evidence",
        depends_on: &["t01"],
    },
    ExecutionTask {
        id: "t03",
        name: "propose_architecture_a",
        depends_on: &["t01", "t02"],
    },
    ExecutionTask {
        id: "t04",
        name: "propose_architecture_b",
        depends_on: &["t01", "t02"],
    },
    ExecutionTask {
        id: "t05",
        name: "propose_architecture_c",
        depends_on: &["t01", "t02"],
    },
    ExecutionTask {
        id: "t06",
        name: "debate_and_select",
        depends_on: &["t03", "t04", "t05"],
    },
    ExecutionTask {
        id: "t07",
        name: "deepen_characters",
        depends_on: &["t06"],
    },
    ExecutionTask {
        id: "t08",
        name: "build_story_beats",
        depends_on: &["t07"],
    },
    ExecutionTask {
        id: "t09",
        name: "plan_episodes",
        depends_on: &["t08"],
    },
    ExecutionTask {
        id: "t10",
        name: "write_sample_scenes",
        depends_on: &["t09"],
    },
    ExecutionTask {
        id: "t11",
        name: "continuity_review",
        depends_on: &["t08", "t09", "t10"],
    },
    ExecutionTask {
        id: "t12",
        name: "human_taste_review",
        depends_on: &["t07", "t09", "t10"],
    },
    ExecutionTask {
        id: "t13",
        name: "originality_review",
        depends_on: &["t02", "t08", "t10"],
    },
    ExecutionTask {
        id: "t14",
        name: "production_review",
        depends_on: &["t09", "t10"],
    },
    ExecutionTask {
        id: "t15",
        name: "targeted_revision",
        depends_on: &["t11", "t12", "t13", "t14"],
    },
    ExecutionTask {
        id: "t16",
        name: "final_review",
        depends_on: &["t15"],
    },
    ExecutionTask {
        id: "t17",
        name: "package_artifact",
        depends_on: &["t16"],
    },
];

pub fn fixed_story_execution_order() -> &'static [ExecutionTask] {
    &FIXED_STORY_EXECUTION_ORDER
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExecutionOrderError {
    #[error("fixed execution order contains a duplicate task id")]
    DuplicateTask,
    #[error("fixed execution order contains a missing or forward dependency")]
    InvalidDependency,
}

pub fn validate_fixed_story_execution_order() -> Result<(), ExecutionOrderError> {
    let mut seen = HashSet::new();
    for task in fixed_story_execution_order() {
        if !seen.insert(task.id) {
            return Err(ExecutionOrderError::DuplicateTask);
        }
        if task
            .depends_on
            .iter()
            .any(|dependency| !seen.contains(dependency))
        {
            return Err(ExecutionOrderError::InvalidDependency);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarState {
    Stopped,
    Starting,
    Ready,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarSignal {
    StartRequested,
    HealthReady,
    StopRequested,
    ProcessExited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("sidecar lifecycle signal is invalid for the current state")]
pub struct SidecarTransitionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidecarLifecycle {
    state: SidecarState,
}

impl Default for SidecarLifecycle {
    fn default() -> Self {
        Self {
            state: SidecarState::Stopped,
        }
    }
}

impl SidecarLifecycle {
    pub fn state(&self) -> SidecarState {
        self.state
    }

    pub fn can_accept_commands(&self) -> bool {
        self.state == SidecarState::Ready
    }

    pub fn transition(
        &mut self,
        signal: SidecarSignal,
    ) -> Result<SidecarState, SidecarTransitionError> {
        self.state = match (self.state, signal) {
            (SidecarState::Stopped | SidecarState::Failed, SidecarSignal::StartRequested) => {
                SidecarState::Starting
            }
            (SidecarState::Starting, SidecarSignal::HealthReady) => SidecarState::Ready,
            (SidecarState::Starting | SidecarState::Ready, SidecarSignal::StopRequested) => {
                SidecarState::Stopping
            }
            (SidecarState::Stopping, SidecarSignal::ProcessExited) => SidecarState::Stopped,
            (SidecarState::Starting | SidecarState::Ready, SidecarSignal::ProcessExited) => {
                SidecarState::Failed
            }
            _ => return Err(SidecarTransitionError),
        };
        Ok(self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_lifecycle_waits_for_process_exit() {
        let mut lifecycle = SidecarLifecycle::default();
        assert_eq!(
            lifecycle.transition(SidecarSignal::StartRequested),
            Ok(SidecarState::Starting)
        );
        assert_eq!(
            lifecycle.transition(SidecarSignal::HealthReady),
            Ok(SidecarState::Ready)
        );
        assert_eq!(
            lifecycle.transition(SidecarSignal::StopRequested),
            Ok(SidecarState::Stopping)
        );
        assert_eq!(
            lifecycle.transition(SidecarSignal::ProcessExited),
            Ok(SidecarState::Stopped)
        );
    }

    #[test]
    fn commands_require_ready_state() {
        let mut lifecycle = SidecarLifecycle::default();
        assert!(!lifecycle.can_accept_commands());
        lifecycle.transition(SidecarSignal::StartRequested).unwrap();
        assert!(!lifecycle.can_accept_commands());
        lifecycle.transition(SidecarSignal::HealthReady).unwrap();
        assert!(lifecycle.can_accept_commands());
        lifecycle.transition(SidecarSignal::StopRequested).unwrap();
        assert!(!lifecycle.can_accept_commands());
    }

    #[test]
    fn unexpected_exit_enters_failed_state() {
        let mut lifecycle = SidecarLifecycle::default();
        lifecycle.transition(SidecarSignal::StartRequested).unwrap();
        lifecycle.transition(SidecarSignal::HealthReady).unwrap();
        assert_eq!(
            lifecycle.transition(SidecarSignal::ProcessExited),
            Ok(SidecarState::Failed)
        );
        assert!(!lifecycle.can_accept_commands());
    }

    #[test]
    fn fixed_order_is_complete_and_topological() {
        let order = fixed_story_execution_order();
        assert_eq!(order.len(), 17);
        assert_eq!(order.first().map(|task| task.id), Some("t01"));
        assert_eq!(order.last().map(|task| task.id), Some("t17"));
        assert_eq!(validate_fixed_story_execution_order(), Ok(()));
    }
}
