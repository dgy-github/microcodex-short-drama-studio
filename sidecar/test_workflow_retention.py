"""Trusted artifact-retention tests for the advisory workflow."""

from __future__ import annotations

import os
import hashlib
import json
import tempfile
import unittest

from campaign.core.events import SqliteEventLog

from campaign_adapter.workflow import AdvisoryStoryWorkflow, TASKS, WorkflowContext
from test_workflow import FakeCapability, story_job


class InMemoryRetentionCapability(FakeCapability):
    def __init__(self) -> None:
        super().__init__()
        self.store: dict[str, dict] = {}
        self.generate_calls = 0
        self.load_calls = 0

    async def generate(self, route, system, prompt):
        self.generate_calls += 1
        return await super().generate(route, system, prompt)

    async def retain_artifact(self, run_id, task_id, artifact_schema, artifact):
        encoded = json.dumps(
            artifact, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")
        digest = hashlib.sha256(encoded).hexdigest()
        content_ref = f"artifact://sha256/{digest}"
        self.store[content_ref] = artifact
        return {"content_ref": content_ref, "content_sha256": digest}

    async def load_artifact(self, content_ref, content_sha256):
        self.load_calls += 1
        if content_ref != f"artifact://sha256/{content_sha256}":
            raise RuntimeError("digest mismatch")
        return self.store[content_ref]


class RetentionTests(unittest.IsolatedAsyncioTestCase):
    """Every task artifact must survive the run through CAP-006."""

    async def asyncSetUp(self) -> None:
        descriptor, self.path = tempfile.mkstemp(
            suffix=".db", prefix="story_retention_"
        )
        os.close(descriptor)
        self.log = SqliteEventLog(self.path)

    async def asyncTearDown(self) -> None:
        self.log.close()
        try:
            os.unlink(self.path)
        except OSError:
            pass

    async def test_every_task_artifact_is_retained_through_the_capability(self) -> None:
        class RetainingCapability(FakeCapability):
            def __init__(self) -> None:
                super().__init__()
                self.retained: list[tuple[str, str]] = []

            async def retain_artifact(self, run_id, task_id, artifact_schema, artifact):
                self.retained.append((run_id, task_id))
                return {
                    "content_ref": f"artifact://sha256/{task_id.lstrip('t'):0<64}",
                    "content_sha256": f"{task_id.lstrip('t'):0<64}",
                }

        capability = RetainingCapability()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        result = await workflow.run(
            story_job(capability), "run_retention", "req_retention"
        )
        self.assertEqual(result["status"], "advisory")
        self.assertEqual(len(capability.retained), len(TASKS))
        self.assertTrue(all(run == "run_retention" for run, _ in capability.retained))
        retained_tasks = {task for _, task in capability.retained}
        self.assertEqual(retained_tasks, {spec.task_id for spec in TASKS})
        refs = [record for record in result["tasks"] if "content_ref" in record]
        self.assertEqual(len(refs), len(TASKS))

    async def test_retention_failure_fails_the_run(self) -> None:
        class FailingRetention(FakeCapability):
            async def retain_artifact(self, run_id, task_id, artifact_schema, artifact):
                raise RuntimeError("store unavailable")

        capability = FailingRetention()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        with self.assertRaises(RuntimeError):
            await workflow.run(
                story_job(capability), "run_retention_fail", "req_retention_fail"
            )

    async def test_recovery_loads_completed_artifacts_without_regeneration(self) -> None:
        capability = InMemoryRetentionCapability()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        job = story_job(capability)
        first = await workflow.run(job, "run_resume", "req_resume")
        calls_after_first = capability.generate_calls

        recovered = await workflow.run(job, "run_resume", "req_resume")

        self.assertEqual(capability.generate_calls, calls_after_first)
        self.assertEqual(capability.load_calls, len(TASKS))
        self.assertEqual(recovered["package"], first["package"])
        self.assertEqual(len(recovered["tasks"]), len(TASKS))

    async def test_artifact_ready_without_task_completed_is_not_restored(self) -> None:
        capability = InMemoryRetentionCapability()
        spec = TASKS[0]
        artifact = {"schema": spec.artifact_schema, "value": "partial"}
        retained = await capability.retain_artifact(
            "run_partial", spec.task_id, spec.artifact_schema, artifact
        )
        context = WorkflowContext(
            self.log,
            story_job(capability),
            "run_partial",
            "req_partial",
            capability=capability,
        )
        await context.emit(
            "task.artifact.ready",
            spec.agent_id,
            spec.task_id,
            {"artifact_schema": spec.artifact_schema, **retained},
        )

        self.assertEqual(await context.restore_artifacts(TASKS), set())
        self.assertEqual(capability.load_calls, 0)

    async def test_missing_upstream_invalidates_its_restored_dependants(self) -> None:
        capability = InMemoryRetentionCapability()
        workflow = AdvisoryStoryWorkflow(self.log, capability)
        job = story_job(capability)
        await workflow.run(job, "run_broken_chain", "req_broken_chain")
        first_ref = next(iter(capability.store))
        capability.store.pop(first_ref)

        context = WorkflowContext(
            self.log,
            job,
            "run_broken_chain",
            "req_broken_chain",
            capability=capability,
        )
        restored = await context.restore_artifacts(TASKS)

        self.assertEqual(restored, set())


class BudgetSurvivesRecoveryTests(unittest.IsolatedAsyncioTestCase):
    """Run-wide token and cost counters survive a restart."""

    async def asyncSetUp(self) -> None:
        descriptor, self.path = tempfile.mkstemp(suffix=".db", prefix="story_budget_recovery_")
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
        await AdvisoryStoryWorkflow(self.log, capability).run(
            story_job(capability), run_id, "req_budget_recovery"
        )
        spent = sum(
            event.payload["data"].get("usage", {}).get("total_tokens", 0)
            for event in await self.log.replay(0)
            if event.type == "task.completed" and event.payload.get("run_id") == run_id
        )
        context = WorkflowContext(self.log, story_job(capability), run_id, "req_budget_recovery")
        self.assertGreater(spent, 0)
        self.assertEqual(await context.restore_consumed_tokens(), spent)
        self.assertEqual(context.consumed_tokens, spent)

    async def test_a_different_run_does_not_inherit_consumption(self) -> None:
        capability = FakeCapability()
        await AdvisoryStoryWorkflow(self.log, capability).run(
            story_job(capability), "run_budget_other", "req_other"
        )
        context = WorkflowContext(self.log, story_job(capability), "run_budget_fresh", "req_fresh")
        self.assertEqual(await context.restore_consumed_tokens(), 0)

    async def test_consumed_cost_is_restored_with_tokens(self) -> None:
        capability = FakeCapability()
        await self.log.append("task.completed", "fixture-agent", {
            "job_id": "job_workflow_test", "run_id": "run_cost_restore",
            "causation_id": "req_cost_restore", "correlation_id": "req_cost_restore",
            "task_id": "t01", "agent_id": "fixture-agent",
            "data": {"usage": {"total_tokens": 10, "cost_cny_fen": 7}},
        })
        context = WorkflowContext(self.log, story_job(capability), "run_cost_restore", "req_cost_restore")
        await context.restore_consumed_tokens()
        self.assertEqual((context.consumed_tokens, context.consumed_cny_fen), (10, 7))

    async def test_cost_budget_fails_before_artifact_retention(self) -> None:
        capability = FakeCapability()
        job = story_job(capability)
        job["budget"]["max_cny_fen"] = 0
        context = WorkflowContext(self.log, job, "run_cost_limit", "req_cost_limit", capability=capability)
        with self.assertRaisesRegex(RuntimeError, "cost budget exceeded"):
            await context.retain(TASKS[0], {"schema": TASKS[0].artifact_schema}, "fixture-model", {"total_tokens": 1, "cost_cny_fen": 1})
        self.assertEqual(context.failure_code, "cost_budget_exceeded")
        self.assertEqual(context.records, [])


if __name__ == "__main__":
    unittest.main()
