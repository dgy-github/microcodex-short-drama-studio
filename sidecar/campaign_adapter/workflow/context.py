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
    ) -> None:
        self.event_log = event_log
        self.job = job
        self.run_id = run_id
        self.request_id = request_id
        self.genre_context = genre_context
        self.artifacts: dict[str, dict[str, Any]] = {}
        self.records: list[dict[str, str]] = []
        self.routes: dict[str, str] = {}
        self.consumed_tokens = 0
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
        self.consumed_tokens = restored
        if restored:
            await self.emit(
                "run.budget.restored", "story-runtime", None,
                {
                    "consumed_tokens": restored,
                    "max_tokens": int(self.job["budget"]["max_tokens"]),
                    "reason": "an earlier attempt of this run already spent tokens",
                },
            )
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
    ) -> str:
        encoded = json.dumps(
            artifact, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")
        digest = hashlib.sha256(encoded).hexdigest()
        async with self._lock:
            total_tokens = usage.get("total_tokens", 0)
            if not isinstance(total_tokens, int) or total_tokens < 0:
                raise RuntimeError("provider usage is invalid")
            if self.consumed_tokens + total_tokens > int(
                self.job["budget"]["max_tokens"]
            ):
                self.failure_code = "token_budget_exceeded"
                raise RuntimeError("token budget exceeded")
            self.consumed_tokens += total_tokens
            self.artifacts[spec.task_id] = artifact
            self.records.append(
                {
                    "task_id": spec.task_id,
                    "agent_id": spec.agent_id,
                    "artifact_schema": spec.artifact_schema,
                    "content_sha256": digest,
                }
            )
            if model != "deterministic":
                self.routes["review" if spec.task_id in REVIEW_TASKS else "generation"] = model
        return digest
