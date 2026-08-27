from __future__ import annotations

import json
import os
import asyncio
import tempfile
import unittest
import sys
from pathlib import Path
from typing import Any
from unittest.mock import patch

from campaign.core.events import SqliteEventLog

from campaign_adapter.workflow import (
    AdvisoryStoryWorkflow,
    EPISODE_WRITER_CONCURRENCY,
    REVIEW_TASKS,
    TASKS,
    WorkflowContext,
    character_reference,
    normalize_artifact,
    package_schema,
    validate_task_graph,
)


class FakeCapability:
    def __init__(self) -> None:
        package_path = (
            Path(__file__).parents[1]
            / "eval"
            / "baselines"
            / "baseline-deepseek-v4-pro-20260727"
            / "family_001.story-package.json"
        )
        if not package_path.is_file():
            raise FileNotFoundError(f"tracked workflow fixture missing: {package_path}")
        self.package = json.loads(package_path.read_text(encoding="utf-8"))
        self.validations = 0
        self.prompts: dict[str, str] = {}
        self.episode_calls: list[int] = []
        self.episode_prompts: list[str] = []

    async def generate(
        self, route: str, system: str, prompt: str
    ) -> tuple[dict[str, Any], dict[str, Any], str]:
        task_name = prompt.splitlines()[0].split("=", 1)[1]
        if task_name.startswith("write_episode_"):
            episode_index = int(task_name.rsplit("_", 1)[1])
            self.episode_calls.append(episode_index)
            self.episode_prompts.append(prompt)
            return (
                {
                    "schema": "sample-scenes/v1",
                    "scenes": [
                        {
                            "location": f"第{episode_index}集场景",
                            "lines": [
                                {
                                    "kind": "action",
                                    "text": f"第{episode_index}集行动。",
                                },
                                {
                                    "kind": "dialogue",
                                    "speaker": "主角",
                                    "text": f"第{episode_index}集对白。",
                                    "subtext": "没有说出口的目的。",
                                },
                            ],
                        }
                    ],
                },
                {"total_tokens": 2},
                "qwen-test",
            )
        spec = next(spec for spec in TASKS if spec.name == task_name)
        self.prompts[spec.task_id] = prompt
        if spec.task_id == "t15":
            artifact = dict(self.package)
            artifact["job_id"] = "job_workflow_test"
        elif spec.task_id == "t09":
            artifact = {
                "schema": "episode-plan/v1",
                "episodes": [
                    {
                        "opening_state": f"第{index}集开场",
                        "conflict": f"第{index}集冲突",
                        "turn": f"第{index}集转折",
                        "end_hook": f"第{index}集钩子",
                    }
                    for index in range(1, len(self.package["episodes"]) + 1)
                ],
            }
        elif spec.task_id in REVIEW_TASKS:
            artifact = {
                "schema": "story-review-record/v1",
                "review_id": f"review_{spec.task_id}",
                "task_id": spec.task_id,
                "review_type": {
                    "t11": "continuity",
                    "t12": "human_taste",
                    "t13": "originality",
                    "t14": "production",
                    "t16": "final",
                }[spec.task_id],
                "status": "pass",
                "summary": "测试审查通过。",
                "findings": [],
            }
        else:
            artifact = {"schema": spec.artifact_schema, "value": spec.name}
        return artifact, {"total_tokens": 1}, (
            "glm-test" if route == "review" else "qwen-test"
        )

    async def validate_package(
        self, package: dict[str, Any], expected_episodes: int
    ) -> dict[str, Any]:
        self.validations += 1
        if len(package["episodes"]) != expected_episodes:
            raise RuntimeError("episode mismatch")
        return {"schema": "story-capability-response/v1", "status": "ok"}


class FailingCapability(FakeCapability):
    def __init__(self, failure: Exception) -> None:
        super().__init__()
        self.failure = failure

    async def generate(self, route: str, system: str, prompt: str):
        raise self.failure


class TrackingCapability(FakeCapability):
    def __init__(self) -> None:
        super().__init__()
        self.active = 0
        self.max_active = 0
        self._overlap_observed = asyncio.Event()

    async def generate(self, route: str, system: str, prompt: str):
        task_name = prompt.splitlines()[0].split("=", 1)[1]
        if not task_name.startswith("write_episode_"):
            return await super().generate(route, system, prompt)

        self.active += 1
        self.max_active = max(self.max_active, self.active)
        if self.active >= 2:
            self._overlap_observed.set()
        try:
            await asyncio.wait_for(self._overlap_observed.wait(), timeout=1.0)
            return await super().generate(route, system, prompt)
        finally:
            self.active -= 1


def story_job(capability: FakeCapability, max_tokens: int = 100000) -> dict[str, Any]:
    return {
        "schema": "story-job/v1",
        "job_id": "job_workflow_test",
        "content_form": "scripted_short_drama",
        "input": "测试故事种子",
        "genre_mode": "auto",
        "allowed_genres": ["family"],
        "audience": "25-45",
        "format": {
            "episodes": len(capability.package["episodes"]),
            "minutes_per_episode": 2,
        },
        "content_limits": [],
        "budget": {
            "max_tokens": max_tokens,
            "max_cny_fen": 1000,
            "deadline_seconds": 60,
        },
    }


class TaskGraphTests(unittest.TestCase):
    """Python TASKS must stay in lock-step with the Rust fixed order.

    `RUST_ORDER` below is a literal transcription of
    `FIXED_STORY_EXECUTION_ORDER` in crates/story-runtime/src/execution.rs.
    Changing either copy without the other fails `test_python_tasks_match_rust_fixed_order`.
    """

    RUST_ORDER = (
        ("t01", ()),
        ("t02", ("t01",)),
        ("t03", ("t01", "t02")),
        ("t04", ("t01", "t02")),
        ("t05", ("t01", "t02")),
        ("t06", ("t03", "t04", "t05")),
        ("t07", ("t06",)),
        ("t08", ("t07",)),
        ("t09", ("t08",)),
        ("t10", ("t09",)),
        ("t11", ("t08", "t09", "t10")),
        ("t12", ("t07", "t09", "t10")),
        ("t13", ("t02", "t08", "t10")),
        ("t14", ("t09", "t10")),
        ("t15", ("t11", "t12", "t13", "t14")),
        ("t16", ("t15",)),
        ("t17", ("t16",)),
    )

    def test_python_tasks_match_rust_fixed_order(self) -> None:
        self.assertEqual(
            [(spec.task_id, spec.depends_on) for spec in TASKS],
            list(self.RUST_ORDER),
        )

    def test_task_graph_is_complete_and_topological(self) -> None:
        validate_task_graph()

    def test_task_graph_rejects_duplicate_and_forward_dependency(self) -> None:
        from campaign_adapter.workflow import TaskSpec

        duplicate = (
            TaskSpec("t01", "a", "a", "s", "schema", ()),
            TaskSpec("t01", "b", "b", "s", "schema", ()),
        )
        with self.assertRaisesRegex(ValueError, "t01"):
            validate_task_graph(duplicate)

        forward = (
            TaskSpec("t01", "a", "a", "s", "schema", ("t02",)),
            TaskSpec("t02", "b", "b", "s", "schema", ()),
        )
        with self.assertRaisesRegex(ValueError, "t02"):
            validate_task_graph(forward)


class AdvisoryWorkflowTests(unittest.IsolatedAsyncioTestCase):
    def test_character_reference_preserves_distinct_named_speakers(self) -> None:
        characters = [{"name": "陈远"}, {"name": "陈建国"}, {"name": "李慧"}]
        self.assertEqual(
            character_reference("陈建国（压低声音）", characters),
            "story-package/ch-2",
        )
        self.assertEqual(
            character_reference("story-package/ch-3", characters),
            "story-package/ch-3",
        )

    def test_package_schema_uses_pyinstaller_runtime_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            schema_path = Path(directory) / "schemas" / "story-package-v1.json"
            schema_path.parent.mkdir()
            schema_path.write_text('{"schema":"frozen-test"}', encoding="utf-8")

            with patch.object(sys, "_MEIPASS", directory, create=True):
                self.assertEqual(package_schema(), '{"schema":"frozen-test"}')

    def test_package_schema_resolves_the_repository_copy_when_not_frozen(self) -> None:
        """The unfrozen branch is `__file__`-relative, so moving this module
        between directories silently breaks it. Only the frozen branch used to
        be covered, and splitting workflow.py into a package did break it."""
        schema = json.loads(package_schema())
        self.assertEqual(
            schema["$id"], "https://microcodex.local/schemas/story-package-v1.json"
        )

    def test_review_severity_aliases_are_normalized_to_contract_values(self) -> None:
        spec = next(spec for spec in TASKS if spec.task_id == "t11")
        artifact = {
            "findings": [
                {"severity": "high"},
                {"severity": "medium"},
                {"severity": "low"},
            ]
        }

        normalize_artifact(spec, artifact)

        self.assertEqual(
            [finding["severity"] for finding in artifact["findings"]],
            ["major", "minor", "note"],
        )

    async def asyncSetUp(self) -> None:
        descriptor, self.path = tempfile.mkstemp(suffix=".db", prefix="story_workflow_")
        os.close(descriptor)
        self.log = SqliteEventLog(self.path)

    async def asyncTearDown(self) -> None:
        self.log.close()
        try:
            os.unlink(self.path)
        except OSError:
            pass

    async def test_fixed_workflow_retains_all_tasks_reviews_and_package(self) -> None:
        capability = FakeCapability()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        result = await workflow.run(
            story_job(capability),
            "run_workflow_test",
            "req_workflow_test",
            {
                "schema": "genre-context/v1",
                "pack_id": "family-grounded-v1",
                "constraint_profile_id": "short-vertical-v1",
                "genre": "family",
                "architect_directives": ["使用现实家庭关系冲突。"],
                "reviewer_directives": ["检查人物行为是否可信。"],
                "human_writing": {
                    "profile_id": "short-drama-human-writing-v1",
                    "task_directives": {
                        "t07": ["区分人物声音。"],
                        "t10": ["使用潜台词和可拍摄动作。"],
                        "t12": ["检查工具化对白。"],
                        "t15": ["只修订有证据的缺陷。"],
                        "t16": ["复核人物声音与因果。"]
                    }
                },
                "retrieval_sources": [
                    {
                        "source_id": "internal-story-craft-v1",
                        "license_id": "MIT",
                        "content_sha256": "0" * 64,
                        "usage": "genre_pack_guidance",
                    }
                ],
            },
        )

        self.assertEqual(
            [task["task_id"] for task in result["tasks"]],
            [f"t{index:02}" for index in range(1, 18)],
        )
        self.assertEqual(len(result["reviews"]), 5)
        self.assertEqual(result["package"]["schema"], "story-package/v1")
        self.assertEqual(result["promotion"], "non-promotable")
        self.assertEqual(capability.validations, 2)
        self.assertEqual(
            capability.episode_calls,
            list(range(1, len(capability.package["episodes"]) + 1)),
        )
        self.assertEqual(
            {scene["episode_ref"] for scene in result["package"]["scenes"]},
            {
                f"story-package/ep-{index}"
                for index in range(1, len(capability.package["episodes"]) + 1)
            },
        )
        self.assertIn("使用现实家庭关系冲突", capability.prompts["t03"])
        self.assertIn("检查人物行为是否可信", capability.prompts["t11"])
        self.assertIn("internal-story-craft-v1", capability.prompts["t03"])
        for task_id, marker in (
            ("t07", "区分人物声音"),
            ("t12", "检查工具化对白"),
            ("t15", "只修订有证据的缺陷"),
            ("t16", "复核人物声音与因果"),
        ):
            self.assertIn(marker, capability.prompts[task_id])
        self.assertNotIn("第1集行动。", capability.prompts["t15"])
        self.assertIn('"review_findings": []', capability.prompts["t15"])
        self.assertIn('"scene_findings": []', capability.prompts["t15"])
        self.assertNotIn("JSON_SCHEMA=", capability.prompts["t15"])
        self.assertIn("不要复写 scenes", capability.prompts["t15"])
        self.assertTrue(capability.episode_prompts)
        self.assertTrue(
            all(
                "使用潜台词和可拍摄动作" in prompt
                for prompt in capability.episode_prompts
            )
        )
        events = await self.log.replay()
        product_events = [
            event
            for event in events
            if event.payload.get("run_id") == "run_workflow_test"
        ]
        self.assertEqual(
            len([event for event in product_events if event.type == "task.completed"]),
            17,
        )
        self.assertEqual(
            len([event for event in product_events if event.type == "episode.completed"]),
            len(capability.package["episodes"]),
        )
        self.assertEqual(product_events[-1].type, "task.completed")

    async def test_token_budget_fails_closed_before_retaining_overage(self) -> None:
        capability = FakeCapability()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        with self.assertRaisesRegex(RuntimeError, "token_budget_exceeded"):
            await workflow.run(
                story_job(capability, max_tokens=1),
                "run_budget_test",
                "req_budget_test",
            )

    async def test_provider_failure_and_timeout_are_not_degraded_to_fake_output(self) -> None:
        for failure, expected in (
            (RuntimeError("provider unavailable"), "provider_or_task_failure"),
            (asyncio.TimeoutError(), "capability_timeout"),
        ):
            capability = FailingCapability(failure)
            workflow = AdvisoryStoryWorkflow(self.log, capability)
            with self.assertRaisesRegex(RuntimeError, expected):
                await workflow.run(
                    story_job(capability),
                    f"run_failure_{expected.replace(' ', '_')}",
                    "req_failure",
                )

    async def test_independent_lanes_use_bounded_parallelism(self) -> None:
        capability = TrackingCapability()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        await workflow.run(
            story_job(capability),
            "run_concurrency_test",
            "req_concurrency_test",
        )
        self.assertGreaterEqual(capability.max_active, 2)
        self.assertLessEqual(capability.max_active, EPISODE_WRITER_CONCURRENCY)


if __name__ == "__main__":
    unittest.main()


class FinalReviewRejectingCapability(FakeCapability):
    """Final review completes as a task but refuses the package."""

    def __init__(self, status: str = "fail", critical: int = 0) -> None:
        super().__init__()
        self._status = status
        self._critical = critical

    async def generate(
        self, route: str, system: str, prompt: str
    ) -> tuple[dict[str, Any], dict[str, Any], str]:
        artifact, usage, model = await super().generate(route, system, prompt)
        task_name = prompt.splitlines()[0].split("=", 1)[1]
        if task_name == "final_review":
            artifact = dict(artifact)
            artifact["status"] = self._status
            artifact["findings"] = [
                {"severity": "critical", "detail": "seeded"} for _ in range(self._critical)
            ]
        return artifact, usage, model


class PackageRejectingCapability(FakeCapability):
    """Everything generates, but the package fails the artifact contract."""

    async def validate_package(
        self, package: dict[str, Any], expected_episodes: int
    ) -> dict[str, Any]:
        raise RuntimeError("episode count mismatch")


class FailureClassificationTests(unittest.IsolatedAsyncioTestCase):
    """A single failure bucket sends the operator down the wrong path.

    "Final review refused the story" and "the provider broke" need opposite
    responses; reporting both as provider_or_task_failure told the operator to
    retry when the correct action was to fix the story.
    """

    async def asyncSetUp(self) -> None:
        descriptor, self.path = tempfile.mkstemp(
            suffix=".db", prefix="story_failure_class_"
        )
        os.close(descriptor)
        self.log = SqliteEventLog(self.path)

    async def asyncTearDown(self) -> None:
        self.log.close()
        try:
            os.unlink(self.path)
        except OSError:
            pass

    async def test_failed_final_review_is_not_a_provider_failure(self) -> None:
        capability = FinalReviewRejectingCapability(status="fail")
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        with self.assertRaises(RuntimeError) as caught:
            await workflow.run(
                story_job(capability), "run_review_reject", "req_review_reject"
            )
        message = str(caught.exception)
        self.assertIn("final_review_rejected", message)
        self.assertNotIn("provider_or_task_failure", message)
        self.assertIn("t17", message, "the failing task must be named")

    async def test_critical_finding_alone_rejects_even_when_status_passes(self) -> None:
        capability = FinalReviewRejectingCapability(status="pass", critical=1)
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        with self.assertRaisesRegex(RuntimeError, "final_review_rejected"):
            await workflow.run(
                story_job(capability), "run_critical_finding", "req_critical_finding"
            )

    async def test_package_validation_failure_has_its_own_code(self) -> None:
        capability = PackageRejectingCapability()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        with self.assertRaises(RuntimeError) as caught:
            await workflow.run(
                story_job(capability), "run_pkg_invalid", "req_pkg_invalid"
            )
        message = str(caught.exception)
        self.assertIn("artifact_validation_failed", message)
        self.assertNotIn("provider_or_task_failure", message)

    async def test_terminal_error_carries_the_detail_not_just_the_bucket(self) -> None:
        capability = FailingCapability(RuntimeError("upstream 503"))
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        with self.assertRaises(RuntimeError) as caught:
            await workflow.run(story_job(capability), "run_detail", "req_detail")
        self.assertIn("upstream 503", str(caught.exception))

    async def test_first_failure_wins_so_the_cause_is_not_replaced_by_a_symptom(
        self,
    ) -> None:
        context = WorkflowContext(self.log, {"job_id": "j"}, "run_x", "req_x")
        context.record_failure("final_review_rejected", "t17", "first")
        context.record_failure("provider_or_task_failure", "t18", "second")
        self.assertEqual(context.failure_code, "final_review_rejected")
        self.assertEqual(context.failure_task_id, "t17")
        self.assertEqual(context.failure_detail, "first")


class FailedRunStopsSpendingTests(unittest.IsolatedAsyncioTestCase):
    """A failed run used to keep paying for work it had already thrown away.

    `asyncio.gather` hands the first exception to the caller but does not
    cancel its siblings, so one failing episode writer left the rest running:
    real flagship calls, discarded output, and usage that never reaches
    `retain()` to be counted against the budget.
    """

    async def asyncSetUp(self) -> None:
        descriptor, self.path = tempfile.mkstemp(
            suffix=".db", prefix="story_spend_stop_"
        )
        os.close(descriptor)
        self.log = SqliteEventLog(self.path)

    async def asyncTearDown(self) -> None:
        self.log.close()
        try:
            os.unlink(self.path)
        except OSError:
            pass

    async def test_one_failed_episode_cancels_its_siblings(self) -> None:
        class OneEpisodeFails(FakeCapability):
            def __init__(self) -> None:
                super().__init__()
                self.completed_after_failure: list[int] = []
                self.failed = False

            async def generate(self, route, system, prompt):
                task_name = prompt.splitlines()[0].split("=", 1)[1]
                if task_name.startswith("write_episode_"):
                    index = int(task_name.rsplit("_", 1)[1])
                    if index == 2:
                        self.failed = True
                        raise RuntimeError("episode 2 provider failure")
                    await asyncio.sleep(0.2)
                    if self.failed:
                        self.completed_after_failure.append(index)
                return await super().generate(route, system, prompt)

        capability = OneEpisodeFails()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        with self.assertRaisesRegex(RuntimeError, "episode 2 provider failure"):
            await workflow.run(
                story_job(capability), "run_episode_fail", "req_episode_fail"
            )
        await asyncio.sleep(0.5)
        self.assertEqual(
            capability.completed_after_failure,
            [],
            "no episode writer may finish a paid call after the run has failed",
        )


class BudgetSurvivesRecoveryTests(unittest.IsolatedAsyncioTestCase):
    """`max_tokens` has to cap the run, not each attempt.

    A recovered run re-enters `run()` with a fresh context. While the counter
    restarted at zero, a restart loop could spend an unbounded multiple of the
    operator's budget, and the sidecar disagreed with the Rust projection,
    which accumulates consumption across the whole run.
    """

    async def asyncSetUp(self) -> None:
        descriptor, self.path = tempfile.mkstemp(
            suffix=".db", prefix="story_budget_recovery_"
        )
        os.close(descriptor)
        self.log = SqliteEventLog(self.path)

    async def asyncTearDown(self) -> None:
        self.log.close()
        try:
            os.unlink(self.path)
        except OSError:
            pass

    async def test_consumed_tokens_are_restored_from_the_event_log(self) -> None:
        capability = FakeCapability()
        run_id = "run_budget_recovery"
        first = AdvisoryStoryWorkflow(self.log, capability)
        await first.run(story_job(capability), run_id, "req_budget_recovery")

        spent = sum(
            event.payload["data"].get("usage", {}).get("total_tokens", 0)
            for event in await self.log.replay(0)
            if event.type == "task.completed"
            and event.payload.get("run_id") == run_id
        )
        self.assertGreater(spent, 0, "the first attempt must have spent tokens")

        context = WorkflowContext(
            self.log, story_job(capability), run_id, "req_budget_recovery"
        )
        self.assertEqual(await context.restore_consumed_tokens(), spent)
        self.assertEqual(context.consumed_tokens, spent)

    async def test_a_different_run_does_not_inherit_consumption(self) -> None:
        capability = FakeCapability()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        await workflow.run(story_job(capability), "run_budget_other", "req_other")

        context = WorkflowContext(
            self.log, story_job(capability), "run_budget_fresh", "req_fresh"
        )
        self.assertEqual(await context.restore_consumed_tokens(), 0)
