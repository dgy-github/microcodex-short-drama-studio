"""Deterministic sustained check for durable StartRun/cancel event semantics."""

from __future__ import annotations

import argparse
import asyncio
import tempfile

from campaign.core.events import SqliteEventLog

from campaign_adapter.server import RunService


def start_command(job_id: str) -> dict:
    return {
        "schema": "start-run-command/v1",
        "job": {
            "schema": "story-job/v1",
            "job_id": job_id,
            "content_form": "scripted_short_drama",
            "input": "sustained reliability check",
            "genre_mode": "fixed",
            "allowed_genres": ["family"],
            "audience": "test",
            "format": {"episodes": 6, "minutes_per_episode": 2},
            "content_limits": [],
            "budget": {
                "max_tokens": 1000,
                "max_cny_fen": 100,
                "deadline_seconds": 60,
            },
        },
    }


async def run_check(iterations: int) -> dict[str, int]:
    if iterations < 1:
        raise ValueError("iterations must be positive")
    with tempfile.TemporaryDirectory() as directory:
        event_log = SqliteEventLog(f"{directory}/events.db")
        service = RunService(event_log)
        try:
            for index in range(iterations):
                key = f"sustained-start-key-{index:08d}"
                command = start_command(f"job_sustained_{index:08d}")
                first = await service.start_run(command, key)
                replay = await service.start_run(command, key)
                if not replay.replayed or first.acceptance != replay.acceptance:
                    raise RuntimeError("idempotent replay diverged")
                await service.cancel_run(first.acceptance["run_id"])
                await service.cancel_run(first.acceptance["run_id"])
            events = await event_log.replay(0)
        finally:
            await service.close()
            event_log.close()
    accepted = sum(event.type == "run.accepted" for event in events)
    cancelled = sum(event.type == "run.cancelled" for event in events)
    if accepted != iterations or cancelled != iterations:
        raise RuntimeError("durable terminal event count diverged")
    return {
        "iterations": iterations,
        "accepted": accepted,
        "cancelled": cancelled,
        "events": len(events),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--iterations", type=int, default=250)
    args = parser.parse_args()
    result = asyncio.run(run_check(args.iterations))
    print(
        "Sustained event check: "
        f"{result['iterations']} runs, {result['events']} durable events"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
