"""The fixed 17-task story graph and its execution order.

Kept together because the graph is a contract: it must stay in lock-step with
the Rust copy in `crates/story-runtime/src/execution.rs`.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from campaign.core.models import ExecutionOrder, Task

REVIEW_TASKS = {"t11", "t12", "t13", "t14", "t16"}
REVIEW_TYPES = {
    "t11": "continuity",
    "t12": "human_taste",
    "t13": "originality",
    "t14": "production",
    "t16": "final",
}


@dataclass(frozen=True)
class TaskSpec:
    task_id: str
    name: str
    agent_id: str
    skill: str
    artifact_schema: str
    depends_on: tuple[str, ...]


# 17-task fixed story graph. This MUST stay in lock-step with the Rust copy in
# `crates/story-runtime/src/execution.rs` (`FIXED_STORY_EXECUTION_ORDER`):
# identical task ids, identical ordering, identical `depends_on`. Rust validates
# the graph topologically at compile time; `validate_task_graph()` below guards
# this Python copy, and `test_task_graph_matches_rust_order` in test_workflow.py
# pins the ids/dependencies against the Rust-ordered expectation.
TASKS = (
    TaskSpec("t01", "classify_genre", "genre-analyst", "genre-classification", "genre-analysis/v1", ()),
    TaskSpec("t02", "retrieve_evidence", "life-detail-retriever", "licensed-rag", "retrieval-manifest/v1", ("t01",)),
    TaskSpec("t03", "propose_architecture_a", "story-architect-a", "architecture-a", "story-architecture/v1", ("t01", "t02")),
    TaskSpec("t04", "propose_architecture_b", "story-architect-b", "architecture-b", "story-architecture/v1", ("t01", "t02")),
    TaskSpec("t05", "propose_architecture_c", "story-architect-c", "architecture-c", "story-architecture/v1", ("t01", "t02")),
    TaskSpec("t06", "debate_and_select", "story-coordinator", "architecture-selection", "architecture-decision/v1", ("t03", "t04", "t05")),
    TaskSpec("t07", "deepen_characters", "character-room", "character-arc", "character-bible/v1", ("t06",)),
    TaskSpec("t08", "build_story_beats", "story-beat-architect", "story-beats", "story-beats/v1", ("t07",)),
    TaskSpec("t09", "plan_episodes", "episode-planner", "episode-hooks", "episode-plan/v1", ("t08",)),
    TaskSpec("t10", "write_sample_scenes", "scene-writer", "scene-dialogue", "sample-scenes/v1", ("t09",)),
    TaskSpec("t11", "continuity_review", "continuity-editor", "continuity-review", "story-review-record/v1", ("t08", "t09", "t10")),
    TaskSpec("t12", "human_taste_review", "human-taste-editor", "human-taste-review", "story-review-record/v1", ("t07", "t09", "t10")),
    TaskSpec("t13", "originality_review", "originality-editor", "originality-review", "story-review-record/v1", ("t02", "t08", "t10")),
    TaskSpec("t14", "production_review", "production-editor", "production-review", "story-review-record/v1", ("t09", "t10")),
    TaskSpec("t15", "targeted_revision", "reserve-writer", "targeted-revision", "story-package/v1", ("t11", "t12", "t13", "t14")),
    TaskSpec("t16", "final_review", "final-editor", "final-review", "story-review-record/v1", ("t15",)),
    TaskSpec("t17", "package_artifact", "artifact-packager", "package-artifact", "story-package/v1", ("t16",)),
)


def validate_task_graph(specs: tuple[TaskSpec, ...] = TASKS) -> None:
    """Assert the fixed graph is complete and topologically ordered.

    Mirrors `validate_fixed_story_execution_order` on the Rust side. Any
    duplicate id or forward reference raises; callers rely on this to catch a
    drift between the Python and Rust copies at test time.
    """
    expected = [f"t{index:02}" for index in range(1, len(specs) + 1)]
    seen: set[str] = set()
    for spec, expected_id in zip(specs, expected):
        if spec.task_id != expected_id:
            raise ValueError(
                f"task graph must be t01..t{len(specs):02} in order; "
                f"found {spec.task_id!r} where {expected_id!r} was expected"
            )
        if spec.task_id in seen:
            raise ValueError(f"duplicate task id {spec.task_id!r}")
        seen.add(spec.task_id)
        for dependency in spec.depends_on:
            if dependency not in seen:
                raise ValueError(
                    f"task {spec.task_id!r} depends on {dependency!r}, "
                    "which is missing or appears later in the graph"
                )
    if len(specs) != 17:
        raise ValueError(f"task graph must contain 17 tasks, found {len(specs)}")


def relevant_context(task_id: str) -> tuple[str, ...]:
    if task_id == "t06":
        return ("t03", "t04", "t05")
    if task_id == "t07":
        return ("t01", "t06")
    if task_id == "t08":
        return ("t06", "t07")
    if task_id == "t09":
        return ("t07", "t08")
    if task_id == "t10":
        return ("t07", "t08", "t09")
    if task_id in {"t11", "t12", "t13", "t14"}:
        return ("t02", "t07", "t08", "t09", "t10")
    if task_id == "t15":
        return ("t07", "t08", "t09", "t11", "t12", "t13", "t14")
    if task_id == "t16":
        return ("t15",)
    return ()


def execution_order(job: dict[str, Any]) -> ExecutionOrder:
    return ExecutionOrder(
        objective=f"为 {job['job_id']} 生成并审查完整短剧故事",
        constraints=["advisory/non-promotable", "authorized retrieval only"],
        budget=job["budget"],
        tasks=[
            Task(
                id=spec.task_id,
                goal=spec.name,
                difficulty="hard" if spec.task_id in {"t06", "t15", "t16"} else "medium",
                degradable=False,
                required_skills=[spec.skill],
                acceptance=f"return {spec.artifact_schema}",
                depends_on=list(spec.depends_on),
            )
            for spec in TASKS
        ],
    )
