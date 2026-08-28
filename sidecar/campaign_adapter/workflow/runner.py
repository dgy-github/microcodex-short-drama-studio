"""The advisory workflow: wires the graph, the agents and the runtime."""

from __future__ import annotations

from typing import Any

from campaign.app.config import Config
from campaign.app.runtime import Runtime
from campaign.core.models import AgentSpec

from .agents import ContractReviewer, StoryAgent
from .capability import Capability
from .context import WorkflowContext
from .graph import REVIEW_TASKS, TASKS, execution_order

class AdvisoryStoryWorkflow:
    def __init__(self, event_log: Any, capability: Capability) -> None:
        self._event_log = event_log
        self._capability = capability

    async def run(
        self,
        job: dict[str, Any],
        run_id: str,
        request_id: str,
        genre_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        context = WorkflowContext(
            self._event_log, job, run_id, request_id, genre_context
        )
        await context.emit(
            "run.started", "story-runtime", None,
            {"status": "advisory", "promotion": "non-promotable"},
        )
        await context.restore_consumed_tokens()
        runtime = Runtime(self._event_log, Config(privacy_strict=False))
        runtime.set_concurrency(3)
        runtime.set_require_reviewer(True)
        runtime.set_task_timeout(float(job["budget"]["deadline_seconds"]))
        for task_spec in TASKS:
            role = "reviewer" if task_spec.task_id in REVIEW_TASKS else (
                "retriever" if task_spec.task_id == "t02" else "executor"
            )
            agent = StoryAgent(
                AgentSpec(
                    id=task_spec.agent_id,
                    role=role,
                    model_tier="flagship",
                    skills=[task_spec.skill],
                ),
                self._event_log,
                task_spec,
                context,
                self._capability,
            )
            runtime.register_agent(agent)
        runtime.register_agent(
            ContractReviewer(
                AgentSpec(
                    id="contract-reviewer",
                    role="reviewer",
                    model_tier="flagship",
                    skills=[],
                ),
                self._event_log,
            )
        )

        result = await runtime.run(execution_order(job))
        statuses = [item.get("status") for item in result["results"]]
        if statuses != ["done"] * len(TASKS):
            detail = context.failure_detail or "no detail captured"
            task = context.failure_task_id or "unknown task"
            raise RuntimeError(
                f"fixed workflow failed: {context.failure_code or 'task_failure'} "
                f"at {task}: {detail}"
            )
        expected_ids = [spec.task_id for spec in TASKS]
        if sorted(item["task_id"] for item in context.records) != expected_ids:
            raise RuntimeError("fixed workflow artifact order is incomplete")
        records_by_id = {item["task_id"]: item for item in context.records}
        reviews = [context.artifacts[task_id] for task_id in ("t11", "t12", "t13", "t14", "t16")]
        workflow_result = {
            "schema": "story-workflow-result/v1",
            "run_id": run_id,
            "job_id": job["job_id"],
            "status": "advisory",
            "promotion": "non-promotable",
            "tasks": [records_by_id[task_id] for task_id in expected_ids],
            "reviews": reviews,
            "package": context.artifacts["t17"],
            "provider_routes": {
                "generation": context.routes.get("generation", "unknown"),
                "review": context.routes.get("review", "unknown"),
            },
        }
        return workflow_result

    async def close(self) -> None:
        close = getattr(self._capability, "close", None)
        if callable(close):
            await close()
