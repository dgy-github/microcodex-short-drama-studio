"""Run-scoped state: artifacts, budget, routes and the first failure cause."""

from __future__ import annotations

import asyncio
import hashlib
import json
from typing import Any

from .graph import REVIEW_TASKS, TaskSpec

class WorkflowContext:
    def __init__(
        self,
        event_log: Any,
        job: dict[str, Any],
        run_id: str,
        request_id: str,
        genre_context: dict[str, Any] | None = None,
        capability: Any = None,
    ) -> None:
        self.event_log = event_log
        self.job = job
        self.run_id = run_id
        self._capability = capability
        self.request_id = request_id
        self.genre_context = genre_context
        self.artifacts: dict[str, dict[str, Any]] = {}
        self.records: list[dict[str, str]] = []
        self.routes: dict[str, str] = {}
        self.consumed_tokens = 0
        self.consumed_cny_fen = 0
        self.failure_code: str | None = None
        self.failure_task_id: str | None = None
        self.failure_detail: str | None = None
        self._lock = asyncio.Lock()

    async def restore_consumed_tokens(self) -> int:
        """Re-seed the budget counter from this run's durable `task.completed`.

        A recovered run re-enters `run()` with a fresh context, so without this
        `max_tokens` would cap each attempt instead of the run, and a restart
        loop could spend an unbounded multiple of the operator's budget. The
        Rust projection already accumulates the same field across the whole run
        (`run_controller.rs`), so restoring here also stops enforcement and the
        displayed consumption from disagreeing after a recovery.
        """
        replay = getattr(self.event_log, "replay", None)
        if replay is None:
            return 0
        restored = 0
        restored_cny_fen = 0
        for event in await replay(0):
            if getattr(event, "type", None) != "task.completed":
                continue
            payload = getattr(event, "payload", None)
            if not isinstance(payload, dict) or payload.get("run_id") != self.run_id:
                continue
            data = payload.get("data")
            usage = data.get("usage") if isinstance(data, dict) else None
            total = usage.get("total_tokens") if isinstance(usage, dict) else None
            if isinstance(total, int) and total > 0:
                restored += total
            cost = usage.get("cost_cny_fen") if isinstance(usage, dict) else None
            if isinstance(cost, int) and cost > 0:
                restored_cny_fen += cost
        self.consumed_tokens = restored
        self.consumed_cny_fen = restored_cny_fen
        if restored or restored_cny_fen:
            await self.emit(
                "run.budget.restored", "story-runtime", None,
                {
                    "consumed_tokens": restored,
                    "max_tokens": int(self.job["budget"]["max_tokens"]),
                    "consumed_cny_fen": restored_cny_fen,
                    "max_cny_fen": int(self.job["budget"]["max_cny_fen"]),
                    "reason": "an earlier attempt of this run already spent tokens",
                },
            )
        return restored

    async def restore_artifacts(self, specs: tuple[TaskSpec, ...]) -> set[str]:
        """Load completed task artifacts whose durable references are known.

        Older events contain only a digest, so they remain compatible but must
        be recomputed. A load failure also falls back to recomputation: the
        runtime must not claim a task is complete without its artifact.
        """
        replay = getattr(self.event_log, "replay", None)
        load = getattr(self._capability, "load_artifact", None)
        if replay is None or load is None:
            return set()
        by_task: dict[str, dict[str, Any]] = {}
        completed: set[str] = set()
        for event in await replay(0):
            payload = getattr(event, "payload", {})
            if not isinstance(payload, dict) or payload.get("run_id") != self.run_id:
                continue
            task_id = payload.get("task_id")
            data = payload.get("data")
            if getattr(event, "type", None) == "task.completed" and isinstance(task_id, str):
                completed.add(task_id)
            elif (
                getattr(event, "type", None) == "task.artifact.ready"
                and isinstance(task_id, str)
                and isinstance(data, dict)
            ):
                by_task[task_id] = data
        restored: set[str] = set()
        for spec in specs:
            if spec.task_id not in completed:
                continue
            if any(dependency not in restored for dependency in spec.depends_on):
                continue
            data = by_task.get(spec.task_id, {})
            ref, digest = data.get("content_ref"), data.get("content_sha256")
            if not isinstance(ref, str) or not isinstance(digest, str):
                continue
            try:
                artifact = await load(ref, digest)
            except Exception:
                continue
            if artifact.get("schema") != spec.artifact_schema:
                continue
            self.artifacts[spec.task_id] = artifact
            self.records.append({
                "task_id": spec.task_id,
                "agent_id": spec.agent_id,
                "artifact_schema": spec.artifact_schema,
                "content_sha256": digest,
                "content_ref": ref,
            })
            restored.add(spec.task_id)
        return restored

    def record_failure(self, code: str, task_id: str | None, detail: str) -> None:
        """Keep the first failure only.

        Later tasks can fail as a consequence of the first one; overwriting
        would replace the cause with a symptom.
        """
        if self.failure_code is not None:
            return
        self.failure_code = code
        self.failure_task_id = task_id
        self.failure_detail = detail

    async def emit(
        self, event_type: str, agent_id: str, task_id: str | None, data: dict[str, Any]
    ) -> None:
        await self.event_log.append(
            event_type,
            agent_id,
            {
                "job_id": self.job["job_id"],
                "run_id": self.run_id,
                "causation_id": self.request_id,
                "correlation_id": self.request_id,
                "task_id": task_id,
                "agent_id": agent_id,
                "data": data,
            },
        )

    async def retain(
        self,
        spec: TaskSpec,
        artifact: dict[str, Any],
        model: str,
        usage: dict[str, Any],
    ) -> dict[str, str]:
        encoded = json.dumps(
            artifact, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")
        digest = hashlib.sha256(encoded).hexdigest()
        async with self._lock:
            total_tokens = usage.get("total_tokens", 0)
            if not isinstance(total_tokens, int) or total_tokens < 0:
                raise RuntimeError("provider usage is invalid")
            cost_cny_fen = usage.get("cost_cny_fen")
            if model != "deterministic" and (
                not isinstance(cost_cny_fen, int) or cost_cny_fen < 0
            ):
                self.failure_code = "provider_cost_unknown"
                raise RuntimeError("provider cost is unknown")
            if model == "deterministic" and cost_cny_fen is None:
                cost_cny_fen = 0
            if self.consumed_tokens + total_tokens > int(
                self.job["budget"]["max_tokens"]
            ):
                self.failure_code = "token_budget_exceeded"
                raise RuntimeError("token budget exceeded")
            if self.consumed_cny_fen + cost_cny_fen > int(
                self.job["budget"]["max_cny_fen"]
            ):
                self.failure_code = "cost_budget_exceeded"
                raise RuntimeError("cost budget exceeded")
            self.consumed_tokens += total_tokens
            self.consumed_cny_fen += cost_cny_fen
            self.artifacts[spec.task_id] = artifact
            record = {
                "task_id": spec.task_id,
                "agent_id": spec.agent_id,
                "artifact_schema": spec.artifact_schema,
                "content_sha256": digest,
            }
            # Durable retention through the trusted Rust store: rejected
            # candidates and every task artifact must survive the run, so a
            # retention failure fails the run instead of dropping state.
            retained = None
            retain_via_capability = getattr(self._capability, "retain_artifact", None)
            if retain_via_capability is not None:
                retained = await retain_via_capability(
                    self.run_id,
                    spec.task_id,
                    spec.artifact_schema,
                    artifact,
                )
            if retained is not None:
                record["content_ref"] = retained["content_ref"]
            self.records.append(record)
            if model != "deterministic":
                self.routes["review" if spec.task_id in REVIEW_TASKS else "generation"] = model
        return record
