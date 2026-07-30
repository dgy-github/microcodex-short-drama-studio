import asyncio
import json
import tempfile
import unittest

from aiohttp import ClientSession
from campaign.core.events import SqliteEventLog

from campaign_adapter.server import (
    CONTROL_PROTOCOL,
    IdempotencyConflict,
    RunService,
    create_app,
    load_auth_token,
    serve,
    validate_genre_context,
    validate_loopback_host,
)

TOKEN = "test-token-that-is-longer-than-32-bytes"
IDEMPOTENCY_KEY = "start-run-idempotency-key-0001"


class FakeHandler:
    async def handle_rpc(self, request, headers=None):
        return {"jsonrpc": "2.0", "id": request.get("id"), "result": {"ok": True}}

class BlockingWorkflow:
    def __init__(self) -> None:
        self.started = asyncio.Event()

    async def run(self, job, run_id, request_id, genre_context=None):
        self.started.set()
        await asyncio.Event().wait()


class CapturingWorkflow:
    def __init__(self) -> None:
        self.genre_context = None
        self.completed = asyncio.Event()

    async def run(self, job, run_id, request_id, genre_context=None):
        self.genre_context = genre_context
        self.completed.set()
        return {"status": "ok"}


def start_command(job_id: str = "job_1") -> dict:
    return {
        "schema": "start-run-command/v1",
        "job": {
            "schema": "story-job/v1",
            "job_id": job_id,
            "content_form": "scripted_short_drama",
            "input": "两名维修工必须在商场开门前修好同一部故障电梯。",
            "genre_mode": "auto",
            "allowed_genres": ["family"],
            "audience": "25-45",
            "format": {"episodes": 8, "minutes_per_episode": 2},
            "content_limits": [],
            "budget": {
                "max_tokens": 100000,
                "max_cny_fen": 1000,
                "deadline_seconds": 600,
            },
        },
    }


def genre_context() -> dict:
    return {
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
                "t16": ["复核人物声音与因果。"],
            },
        },
        "retrieval_sources": [
            {
                "source_id": "internal-story-craft-v1",
                "license_id": "MIT",
                "content_sha256": "0" * 64,
                "usage": "genre_pack_guidance",
            }
        ],
    }


async def read_replay(response) -> list[dict]:
    events = []
    while True:
        line = await asyncio.wait_for(response.content.readline(), timeout=2)
        if line == b": replay-complete\n":
            await response.content.readline()
            return events
        if line.startswith(b"data: "):
            events.append(json.loads(line[6:]))


class ServerContractTests(unittest.IsolatedAsyncioTestCase):
    async def test_health_and_rpc_require_the_launch_token(self) -> None:
        ready = asyncio.Future()
        stop = asyncio.Event()
        task = asyncio.create_task(
            serve(
                FakeHandler(),
                host="127.0.0.1",
                port=0,
                token=TOKEN,
                ready=ready.set_result,
                stop=stop.wait(),
            )
        )
        details = await asyncio.wait_for(ready, timeout=2)
        base_url = f"http://127.0.0.1:{details['port']}"
        try:
            async with ClientSession() as client:
                async with client.get(f"{base_url}/health") as response:
                    self.assertEqual(response.status, 401)

                headers = {"Authorization": f"Bearer {TOKEN}"}
                async with client.get(f"{base_url}/health", headers=headers) as response:
                    self.assertEqual(response.status, 200)
                    self.assertEqual((await response.json())["protocol"], CONTROL_PROTOCOL)

                request = {"jsonrpc": "2.0", "id": "req_1", "method": "agent/cards"}
                async with client.post(
                    f"{base_url}/rpc", json=request, headers=headers
                ) as response:
                    self.assertEqual(response.status, 200)
                    self.assertTrue((await response.json())["result"]["ok"])
        finally:
            stop.set()
            await asyncio.wait_for(task, timeout=2)

    async def test_non_loopback_bind_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            validate_loopback_host("0.0.0.0")
        with self.assertRaises(ValueError):
            validate_loopback_host("localhost")

    async def test_short_or_missing_tokens_are_rejected(self) -> None:
        with self.assertRaises(ValueError):
            load_auth_token({})
        with self.assertRaises(ValueError):
            create_app(FakeHandler(), "short")

    async def test_duplicate_start_run_appends_no_duplicate_work(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            event_log = SqliteEventLog(f"{directory}/events.db")
            runs = RunService(event_log)
            try:
                first = await runs.start_run(start_command(), IDEMPOTENCY_KEY)
                second = await runs.start_run(start_command(), IDEMPOTENCY_KEY)
                events = await runs.replay(first.acceptance["run_id"], 0)
            finally:
                event_log.close()

        self.assertFalse(first.replayed)
        self.assertTrue(second.replayed)
        self.assertEqual(first.acceptance, second.acceptance)
        self.assertEqual(
            [event["event_type"] for event in events],
            ["run.accepted", "task.queued"],
        )
        self.assertEqual(events[1]["task_id"], "t01")

    async def test_typed_genre_context_is_forwarded_to_workflow(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            event_log = SqliteEventLog(f"{directory}/events.db")
            workflow = CapturingWorkflow()
            runs = RunService(event_log, workflow)
            command = start_command()
            command["genre_context"] = genre_context()
            try:
                await runs.start_run(command, IDEMPOTENCY_KEY)
                await asyncio.wait_for(workflow.completed.wait(), timeout=2)
            finally:
                await runs.close()
                event_log.close()

        self.assertEqual(workflow.genre_context, genre_context())

    async def test_human_writing_context_requires_all_five_task_directives(self) -> None:
        context = genre_context()
        del context["human_writing"]["task_directives"]["t16"]
        with self.assertRaisesRegex(ValueError, "invalid human writing context"):
            validate_genre_context(context)

    async def test_incomplete_run_is_recovered_and_result_survives_service_restart(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            event_log = SqliteEventLog(f"{directory}/events.db")
            first = RunService(event_log)
            started = await first.start_run(start_command(), IDEMPOTENCY_KEY)
            workflow = CapturingWorkflow()
            recovered_service = RunService(event_log, workflow)
            try:
                recovered = await recovered_service.recover_incomplete()
                await asyncio.wait_for(workflow.completed.wait(), timeout=2)
                await asyncio.sleep(0.05)
                restarted = RunService(event_log)
                result = await restarted.result(started.acceptance["run_id"])
                events = await restarted.replay(started.acceptance["run_id"], 0)
            finally:
                await recovered_service.close()
                event_log.close()

        self.assertEqual(recovered, 1)
        self.assertEqual(result, {"status": "ok"})
        self.assertIn("run.recovered", [event["event_type"] for event in events])
        self.assertEqual(events[-1]["event_type"], "run.completed")

    async def test_idempotency_key_cannot_change_input(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            event_log = SqliteEventLog(f"{directory}/events.db")
            runs = RunService(event_log)
            try:
                await runs.start_run(start_command(), IDEMPOTENCY_KEY)
                with self.assertRaises(IdempotencyConflict):
                    await runs.start_run(start_command("job_2"), IDEMPOTENCY_KEY)
            finally:
                event_log.close()

    async def test_last_event_id_replays_only_later_events(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            event_log = SqliteEventLog(f"{directory}/events.db")
            runs = RunService(event_log)
            ready = asyncio.Future()
            stop = asyncio.Event()
            task = asyncio.create_task(
                serve(
                    FakeHandler(),
                    host="127.0.0.1",
                    port=0,
                    token=TOKEN,
                    runs=runs,
                    ready=ready.set_result,
                    stop=stop.wait(),
                )
            )
            details = await asyncio.wait_for(ready, timeout=2)
            base_url = f"http://127.0.0.1:{details['port']}"
            headers = {
                "Authorization": f"Bearer {TOKEN}",
                "Idempotency-Key": IDEMPOTENCY_KEY,
            }
            try:
                async with ClientSession() as client:
                    async with client.post(
                        f"{base_url}/v1/runs",
                        json=start_command(),
                        headers=headers,
                    ) as response:
                        self.assertEqual(response.status, 202)
                        acceptance = await response.json()

                    stream_url = f"{base_url}{acceptance['event_stream_url']}"
                    async with client.get(
                        stream_url,
                        headers={"Authorization": f"Bearer {TOKEN}"},
                    ) as response:
                        initial = await read_replay(response)

                    resume_headers = {
                        "Authorization": f"Bearer {TOKEN}",
                        "Last-Event-ID": str(initial[0]["seq"]),
                    }
                    async with client.get(stream_url, headers=resume_headers) as response:
                        resumed = await read_replay(response)
            finally:
                stop.set()
                await asyncio.wait_for(task, timeout=2)
                event_log.close()

        self.assertEqual(len(initial), 2)
        self.assertEqual(resumed, [initial[1]])

    async def test_cancel_is_idempotent_and_records_one_terminal_event(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            event_log = SqliteEventLog(f"{directory}/events.db")
            workflow = BlockingWorkflow()
            runs = RunService(event_log, workflow)
            try:
                started = await runs.start_run(start_command(), IDEMPOTENCY_KEY)
                await asyncio.wait_for(workflow.started.wait(), timeout=2)
                first, created = await runs.cancel_run(started.acceptance["run_id"])
                second, replayed = await runs.cancel_run(started.acceptance["run_id"])
                events = await runs.replay(started.acceptance["run_id"], 0)
            finally:
                await runs.close()
                event_log.close()

        self.assertTrue(created)
        self.assertFalse(replayed)
        self.assertEqual(first, second)
        self.assertEqual(first["event_type"], "run.cancelled")
        self.assertEqual(
            [event["event_type"] for event in events].count("run.cancelled"), 1
        )


if __name__ == "__main__":
    unittest.main()
