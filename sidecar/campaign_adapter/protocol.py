"""Versioned durable event envelope shared with the Rust runtime."""

from __future__ import annotations

from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from typing import Any
from uuid import uuid4

PROTOCOL = "story-agent-event/v1"


@dataclass(frozen=True)
class DurableEvent:
    protocol: str
    event_id: str
    seq: int
    occurred_at: str
    causation_id: str
    correlation_id: str
    job_id: str
    run_id: str
    task_id: str | None
    agent_id: str | None
    event_type: str
    schema_version: int
    payload: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def make_event(
    *,
    seq: int,
    job_id: str,
    run_id: str,
    event_type: str,
    payload: dict[str, Any],
    causation_id: str = "",
    correlation_id: str = "",
    task_id: str | None = None,
    agent_id: str | None = None,
) -> DurableEvent:
    if seq < 1:
        raise ValueError("durable event sequence must be positive")
    if not job_id.strip() or not run_id.strip() or not event_type.strip():
        raise ValueError("job_id, run_id, and event_type must not be blank")
    return DurableEvent(
        protocol=PROTOCOL,
        event_id=f"evt_{uuid4().hex}",
        seq=seq,
        occurred_at=datetime.now(timezone.utc).isoformat(),
        causation_id=causation_id,
        correlation_id=correlation_id,
        job_id=job_id,
        run_id=run_id,
        task_id=task_id,
        agent_id=agent_id,
        event_type=event_type,
        schema_version=1,
        payload=payload,
    )

