"""Authenticated loopback HTTP host for the pinned Campaign handler."""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import hmac
import ipaddress
import json
import os
import socket
import time
from dataclasses import dataclass
from collections.abc import Awaitable, Callable, Mapping
from datetime import datetime
from typing import Any, Protocol
from uuid import uuid4

from aiohttp import web

CONTROL_PROTOCOL = "story-sidecar-control/v1"
TOKEN_ENV = "MICROCODEX_SIDECAR_TOKEN"
MAX_REQUEST_BYTES = 1024 * 1024
HEARTBEAT_SECONDS = 15
POLL_SECONDS = 0.1

# Wire-level command/job schemas. These must match the Rust side: the StartRun
# command schema in story-runtime/src/run_protocol.rs, the job schema in
# story-core/src/lib.rs, and the content-form enum (`ContentForm`), which
# currently exposes only `scripted_short_drama`.
START_RUN_SCHEMA = "start-run-command/v1"
STORY_JOB_SCHEMA = "story-job/v1"
SUPPORTED_CONTENT_FORMS = frozenset({"scripted_short_drama"})


class RpcHandler(Protocol):
    async def handle_rpc(
        self, request: dict[str, Any], headers: dict[str, str] | None = None
    ) -> dict[str, Any]: ...


class EventRecord(Protocol):
    seq: int
    ts: datetime
    type: str
    actor: str
    payload: dict[str, Any]


class EventLog(Protocol):
    async def append(
        self, type: str, actor: str, payload: dict[str, Any]
    ) -> EventRecord: ...

    async def replay(self, since: int = 0) -> list[EventRecord]: ...


class IdempotencyConflict(Exception):
    pass


@dataclass(frozen=True)
class StartResult:
    acceptance: dict[str, Any]
    replayed: bool


class Workflow(Protocol):
    async def run(
        self,
        job: dict[str, Any],
        run_id: str,
        request_id: str,
        genre_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]: ...


class RunService:
    def __init__(self, event_log: EventLog, workflow: Workflow | None = None) -> None:
        self._event_log = event_log
        self._workflow = workflow
        self._command_lock = asyncio.Lock()
        self._background: dict[str, asyncio.Task[None]] = {}
        self._results: dict[str, dict[str, Any]] = {}

    async def start_run(
        self, command: dict[str, Any], idempotency_key: str
    ) -> StartResult:
        validate_start_command(command)
        validate_idempotency_key(idempotency_key)
        fingerprint = command_fingerprint(command)
        async with self._command_lock:
            for event in await self._event_log.replay(0):
                if event.type != "run.accepted":
                    continue
                payload = event.payload
                if payload.get("idempotency_key") != idempotency_key:
                    continue
                if payload.get("command_fingerprint") != fingerprint:
                    raise IdempotencyConflict
                return StartResult(acceptance_from_event(event), replayed=True)

            job = command["job"]
            request_id = f"req_{uuid4().hex}"
            run_id = f"run_{uuid4().hex}"
            accepted = await self._event_log.append(
                "run.accepted",
                "story-runtime",
                {
                    "job_id": job["job_id"],
                    "run_id": run_id,
                    "request_id": request_id,
                    "idempotency_key": idempotency_key,
                    "command_fingerprint": fingerprint,
                    "causation_id": request_id,
                    "correlation_id": request_id,
                    "task_id": None,
                    "agent_id": None,
                    "data": {
                        "command": "StartRun",
                        "command_payload": command,
                    },
                },
            )
            await self._event_log.append(
                "task.queued",
                "story-runtime",
                {
                    "job_id": job["job_id"],
                    "run_id": run_id,
                    "causation_id": request_id,
                    "correlation_id": request_id,
                    "task_id": "t01",
                    "agent_id": None,
                    "data": {
                        "task_name": "classify_genre",
                        "depends_on": [],
                    },
                },
            )
            if self._workflow is not None:
                self._background[run_id] = asyncio.create_task(
                    self._execute_workflow(
                        job, run_id, request_id, command.get("genre_context")
                    )
                )
            return StartResult(acceptance_from_event(accepted), replayed=False)

    async def _execute_workflow(
        self,
        job: dict[str, Any],
        run_id: str,
        request_id: str,
        genre_context: dict[str, Any] | None,
    ) -> None:
        try:
            result = await self._workflow.run(job, run_id, request_id, genre_context)
            self._results[run_id] = result
            await self._event_log.append(
                "workflow.result.stored",
                "story-runtime",
                {
                    "job_id": job["job_id"],
                    "run_id": run_id,
                    "causation_id": request_id,
                    "correlation_id": request_id,
                    "task_id": None,
                    "agent_id": None,
                    "data": {"result": result},
                },
            )
            await self._event_log.append(
                "run.completed",
                "story-runtime",
                {
                    "job_id": job["job_id"],
                    "run_id": run_id,
                    "causation_id": request_id,
                    "correlation_id": request_id,
                    "task_id": None,
                    "agent_id": None,
                    "data": {
                        "status": "advisory",
                        "promotion": "non-promotable",
                        "tasks_completed": 17,
                        "reviews_completed": 5,
                    },
                },
            )
        except Exception as exc:
            await self._event_log.append(
                "run.failed",
                "story-runtime",
                {
                    "job_id": job["job_id"],
                    "run_id": run_id,
                    "causation_id": request_id,
                    "correlation_id": request_id,
                    "task_id": None,
                    "agent_id": None,
                    "data": {"error": str(exc)},
                },
            )
        finally:
            self._background.pop(run_id, None)

    async def result(self, run_id: str) -> dict[str, Any] | None:
        cached = self._results.get(run_id)
        if cached is not None:
            return cached
        for event in reversed(await self._event_log.replay(0)):
            if (
                event.type == "workflow.result.stored"
                and event.payload.get("run_id") == run_id
            ):
                result = event.payload.get("data", {}).get("result")
                if isinstance(result, dict):
                    self._results[run_id] = result
                    return result
        return None

    async def recover_incomplete(self) -> int:
        if self._workflow is None:
            return 0
        events = await self._event_log.replay(0)
        terminal_runs = {
            event.payload.get("run_id")
            for event in events
            if event.type in {"run.completed", "run.failed", "run.cancelled"}
        }
        recovered = 0
        for event in events:
            if event.type != "run.accepted":
                continue
            run_id = event.payload.get("run_id")
            if (
                not isinstance(run_id, str)
                or run_id in terminal_runs
                or run_id in self._background
            ):
                continue
            command = event.payload.get("data", {}).get("command_payload")
            if not isinstance(command, dict):
                continue
            try:
                validate_start_command(command)
            except ValueError:
                continue
            job = command["job"]
            request_id = event.payload["request_id"]
            await self._event_log.append(
                "run.recovered",
                "story-runtime",
                {
                    "job_id": job["job_id"],
                    "run_id": run_id,
                    "causation_id": request_id,
                    "correlation_id": request_id,
                    "task_id": None,
                    "agent_id": None,
                    "data": {"reason": "process_restart"},
                },
            )
            self._background[run_id] = asyncio.create_task(
                self._execute_workflow(
                    job, run_id, request_id, command.get("genre_context")
                )
            )
            recovered += 1
        return recovered

    async def cancel_run(self, run_id: str) -> tuple[dict[str, Any], bool]:
        events = [
            event
            for event in await self._event_log.replay(0)
            if event.payload.get("run_id") == run_id
        ]
        accepted = next((event for event in events if event.type == "run.accepted"), None)
        if accepted is None:
            raise KeyError(run_id)
        terminal = next(
            (
                event
                for event in reversed(events)
                if event.type in {"run.completed", "run.failed", "run.cancelled"}
            ),
            None,
        )
        if terminal is not None:
            return story_event(terminal), False

        task = self._background.get(run_id)
        if task is not None:
            task.cancel()
            await asyncio.gather(task, return_exceptions=True)
        payload = accepted.payload
        cancelled = await self._event_log.append(
            "run.cancelled",
            "story-runtime",
            {
                "job_id": payload["job_id"],
                "run_id": run_id,
                "causation_id": payload["request_id"],
                "correlation_id": payload["request_id"],
                "task_id": None,
                "agent_id": None,
                "data": {"reason": "user_requested"},
            },
        )
        return story_event(cancelled), True

    async def close(self) -> None:
        tasks = list(self._background.values())
        for task in tasks:
            task.cancel()
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)
        if self._workflow is not None:
            close = getattr(self._workflow, "close", None)
            if callable(close):
                await close()

    async def run_exists(self, run_id: str) -> bool:
        return any(
            event.type == "run.accepted"
            and event.payload.get("run_id") == run_id
            for event in await self._event_log.replay(0)
        )

    async def replay(self, run_id: str, since: int) -> list[dict[str, Any]]:
        return [
            story_event(event)
            for event in await self._event_log.replay(since)
            if event.payload.get("run_id") == run_id
        ]


def validate_idempotency_key(value: str) -> None:
    if not 16 <= len(value) <= 128 or any(
        ord(character) < 0x21 or ord(character) > 0x7E for character in value
    ):
        raise ValueError("invalid idempotency key")


def validate_start_command(command: dict[str, Any]) -> None:
    if (
        not {"schema", "job"} <= set(command)
        or not set(command) <= {"schema", "job", "genre_context"}
        or command.get("schema") != START_RUN_SCHEMA
    ):
        raise ValueError("invalid StartRun command")
    job = command.get("job")
    if not isinstance(job, dict):
        raise ValueError("invalid StoryJob")
    if (
        job.get("schema") != STORY_JOB_SCHEMA
        or job.get("content_form") not in SUPPORTED_CONTENT_FORMS
    ):
        raise ValueError("invalid StoryJob")
    if not isinstance(job.get("job_id"), str) or not job["job_id"].strip():
        raise ValueError("invalid StoryJob")
    if "genre_context" in command:
        validate_genre_context(command["genre_context"])


def validate_genre_context(context: Any) -> None:
    required_fields = {
        "schema",
        "pack_id",
        "constraint_profile_id",
        "genre",
        "architect_directives",
        "reviewer_directives",
        "retrieval_sources",
    }
    allowed_fields = required_fields | {"human_writing"}
    if (
        not isinstance(context, dict)
        or not required_fields <= set(context)
        or not set(context) <= allowed_fields
    ):
        raise ValueError("invalid genre context")
    if context.get("schema") != "genre-context/v1":
        raise ValueError("invalid genre context")
    for field in ("pack_id", "constraint_profile_id", "genre"):
        if not isinstance(context.get(field), str) or not context[field].strip():
            raise ValueError("invalid genre context")
    for field in ("architect_directives", "reviewer_directives"):
        values = context.get(field)
        if (
            not isinstance(values, list)
            or not values
            or any(not isinstance(value, str) or not value.strip() for value in values)
        ):
            raise ValueError("invalid genre context")
    sources = context.get("retrieval_sources")
    if not isinstance(sources, list):
        raise ValueError("invalid genre context")
    for source in sources:
        if not isinstance(source, dict) or set(source) != {
            "source_id",
            "license_id",
            "content_sha256",
            "usage",
        }:
            raise ValueError("invalid genre context")
        if any(
            not isinstance(source.get(field), str) or not source[field].strip()
            for field in source
        ):
            raise ValueError("invalid genre context")
    if "human_writing" in context:
        validate_human_writing_context(context["human_writing"])


def validate_human_writing_context(context: Any) -> None:
    if not isinstance(context, dict) or set(context) != {
        "profile_id",
        "task_directives",
    }:
        raise ValueError("invalid human writing context")
    if not isinstance(context.get("profile_id"), str) or not context["profile_id"].strip():
        raise ValueError("invalid human writing context")
    task_directives = context.get("task_directives")
    expected_tasks = {"t07", "t10", "t12", "t15", "t16"}
    if not isinstance(task_directives, dict) or set(task_directives) != expected_tasks:
        raise ValueError("invalid human writing context")
    for directives in task_directives.values():
        if (
            not isinstance(directives, list)
            or not directives
            or any(
                not isinstance(directive, str) or not directive.strip()
                for directive in directives
            )
        ):
            raise ValueError("invalid human writing context")


def command_fingerprint(command: dict[str, Any]) -> str:
    canonical = json.dumps(
        command, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def acceptance_from_event(event: EventRecord) -> dict[str, Any]:
    payload = event.payload
    run_id = payload["run_id"]
    return {
        "schema": "story-command-acceptance/v1",
        "command": "StartRun",
        "request_id": payload["request_id"],
        "job_id": payload["job_id"],
        "run_id": run_id,
        "event_stream_url": f"/v1/runs/{run_id}/events",
        "accepted_event_seq": event.seq,
        "status": "accepted",
    }


def story_event(event: EventRecord) -> dict[str, Any]:
    payload = event.payload
    data = payload.get("data", {})
    if event.type == "workflow.result.stored":
        result = data.get("result", {})
        data = {
            "schema": result.get("schema"),
            "status": "durable",
        }
    return {
        "protocol": "story-agent-event/v1",
        "event_id": f"evt_{event.seq}",
        "seq": event.seq,
        "occurred_at": event.ts.isoformat(),
        "causation_id": payload.get("causation_id", ""),
        "correlation_id": payload.get("correlation_id", ""),
        "job_id": payload["job_id"],
        "run_id": payload["run_id"],
        "task_id": payload.get("task_id"),
        "agent_id": payload.get("agent_id"),
        "event_type": event.type,
        "schema_version": 1,
        "payload": data,
    }


def validate_loopback_host(host: str) -> str:
    try:
        address = ipaddress.ip_address(host)
    except ValueError as exc:
        raise ValueError("sidecar host must be a literal loopback address") from exc
    if not address.is_loopback:
        raise ValueError("sidecar host must be a loopback address")
    return host


def load_auth_token(environ: Mapping[str, str] = os.environ) -> str:
    token = environ.get(TOKEN_ENV, "")
    if len(token) < 32 or not token.isascii():
        raise ValueError("sidecar auth token must be at least 32 ASCII characters")
    return token


def _is_authorized(request: web.Request, token: str) -> bool:
    supplied = request.headers.get("Authorization", "")
    return hmac.compare_digest(supplied, f"Bearer {token}")


def create_app(
    handler: RpcHandler, token: str, runs: RunService | None = None
) -> web.Application:
    if len(token) < 32 or not token.isascii():
        raise ValueError("sidecar auth token must be at least 32 ASCII characters")
    app = web.Application(client_max_size=MAX_REQUEST_BYTES)

    async def health(request: web.Request) -> web.Response:
        if not _is_authorized(request, token):
            raise web.HTTPUnauthorized()
        return web.json_response(
            {"protocol": CONTROL_PROTOCOL, "status": "ready"},
            headers={"Cache-Control": "no-store"},
        )

    async def rpc(request: web.Request) -> web.Response:
        if not _is_authorized(request, token):
            raise web.HTTPUnauthorized()
        try:
            payload = await request.json()
        except (json.JSONDecodeError, web.HTTPBadRequest) as exc:
            raise web.HTTPBadRequest(text="request body must be JSON") from exc
        if not isinstance(payload, dict):
            raise web.HTTPBadRequest(text="request body must be a JSON object")
        response = await handler.handle_rpc(payload, dict(request.headers))
        return web.json_response(response, headers={"Cache-Control": "no-store"})

    async def start_run(request: web.Request) -> web.Response:
        if not _is_authorized(request, token):
            raise web.HTTPUnauthorized()
        if runs is None:
            raise web.HTTPServiceUnavailable()
        idempotency_key = request.headers.get("Idempotency-Key", "")
        try:
            payload = await request.json()
            if not isinstance(payload, dict):
                raise ValueError("command must be an object")
            result = await runs.start_run(payload, idempotency_key)
        except IdempotencyConflict as exc:
            raise web.HTTPConflict(text="idempotency key conflict") from exc
        except (json.JSONDecodeError, ValueError, web.HTTPBadRequest) as exc:
            raise web.HTTPBadRequest(text="invalid StartRun command") from exc
        headers = {
            "Cache-Control": "no-store",
            "Idempotent-Replayed": "true" if result.replayed else "false",
        }
        return web.json_response(result.acceptance, status=202, headers=headers)

    async def stream_events(request: web.Request) -> web.StreamResponse:
        if not _is_authorized(request, token):
            raise web.HTTPUnauthorized()
        if runs is None:
            raise web.HTTPServiceUnavailable()
        run_id = request.match_info["run_id"]
        try:
            since = int(request.headers.get("Last-Event-ID", "0"))
            if since < 0:
                raise ValueError
        except ValueError as exc:
            raise web.HTTPBadRequest(text="Last-Event-ID must be non-negative") from exc
        if not await runs.run_exists(run_id):
            raise web.HTTPNotFound()

        response = web.StreamResponse(
            status=200,
            headers={
                "Content-Type": "text/event-stream",
                "Cache-Control": "no-store",
                "Connection": "keep-alive",
                "X-Accel-Buffering": "no",
            },
        )
        await response.prepare(request)
        cursor = since
        try:
            for event in await runs.replay(run_id, cursor):
                await write_sse_event(response, event)
                cursor = event["seq"]
            await response.write(b": replay-complete\n\n")
            heartbeat_at = time.monotonic()
            while True:
                await asyncio.sleep(POLL_SECONDS)
                if request.transport is None or request.transport.is_closing():
                    break
                for event in await runs.replay(run_id, cursor):
                    await write_sse_event(response, event)
                    cursor = event["seq"]
                if time.monotonic() - heartbeat_at >= HEARTBEAT_SECONDS:
                    await response.write(b": heartbeat\n\n")
                    heartbeat_at = time.monotonic()
        except (ConnectionResetError, asyncio.CancelledError):
            pass
        return response

    async def get_run_result(request: web.Request) -> web.Response:
        if not _is_authorized(request, token):
            raise web.HTTPUnauthorized()
        if runs is None:
            raise web.HTTPServiceUnavailable()
        result = await runs.result(request.match_info["run_id"])
        if result is None:
            raise web.HTTPNotFound()
        return web.json_response(result, headers={"Cache-Control": "no-store"})

    async def cancel_run(request: web.Request) -> web.Response:
        if not _is_authorized(request, token):
            raise web.HTTPUnauthorized()
        if runs is None:
            raise web.HTTPServiceUnavailable()
        try:
            event, created = await runs.cancel_run(request.match_info["run_id"])
        except KeyError as exc:
            raise web.HTTPNotFound() from exc
        return web.json_response(
            event,
            status=202 if created else 200,
            headers={"Cache-Control": "no-store"},
        )

    async def cleanup(_: web.Application) -> None:
        if runs is not None:
            await runs.close()

    app.router.add_get("/health", health)
    app.router.add_post("/rpc", rpc)
    app.router.add_post("/v1/runs", start_run)
    app.router.add_get("/v1/runs/{run_id}/events", stream_events)
    app.router.add_get("/v1/runs/{run_id}/result", get_run_result)
    app.router.add_post("/v1/runs/{run_id}/cancel", cancel_run)
    app.on_cleanup.append(cleanup)
    return app


async def write_sse_event(response: web.StreamResponse, event: dict[str, Any]) -> None:
    data = json.dumps(event, ensure_ascii=False, separators=(",", ":"))
    frame = f"id: {event['seq']}\nevent: {event['event_type']}\ndata: {data}\n\n"
    await response.write(frame.encode("utf-8"))


def _loopback_socket(host: str, port: int) -> socket.socket:
    family = socket.AF_INET6 if ":" in host else socket.AF_INET
    sock = socket.socket(family, socket.SOCK_STREAM)
    try:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.bind((host, port))
        sock.listen(128)
        sock.setblocking(False)
        return sock
    except BaseException:
        sock.close()
        raise


async def serve(
    handler: RpcHandler,
    *,
    host: str,
    port: int,
    token: str,
    runs: RunService | None = None,
    ready: Callable[[dict[str, Any]], None] = print,
    stop: Awaitable[object] | None = None,
) -> None:
    validate_loopback_host(host)
    if runs is not None:
        await runs.recover_incomplete()
    app = create_app(handler, token, runs)
    runner = web.AppRunner(app, access_log=None)
    await runner.setup()
    sock = _loopback_socket(host, port)
    site = web.SockSite(runner, sock)
    try:
        await site.start()
        bound_port = int(sock.getsockname()[1])
        ready({"protocol": CONTROL_PROTOCOL, "status": "ready", "host": host, "port": bound_port})
        await (stop if stop is not None else asyncio.Event().wait())
    finally:
        await runner.cleanup()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="MicrocodeX Campaign sidecar")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--event-log", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    host = validate_loopback_host(args.host)
    token = load_auth_token()

    from campaign.core.events import SqliteEventLog
    from campaign.transport import JsonRpcAgentServer

    handler = JsonRpcAgentServer({}, auth_token=token)
    event_log = SqliteEventLog(args.event_log)
    capability_url = os.environ.get("MICROCODEX_CAPABILITY_URL", "")
    capability_token = os.environ.get("MICROCODEX_CAPABILITY_TOKEN", "")
    workflow = None
    if capability_url and capability_token:
        from .workflow import AdvisoryStoryWorkflow, RustCapabilityClient

        workflow = AdvisoryStoryWorkflow(
            event_log, RustCapabilityClient(capability_url, capability_token)
        )
    runs = RunService(event_log, workflow)

    def write_ready(payload: dict[str, Any]) -> None:
        print(json.dumps(payload, separators=(",", ":")), flush=True)

    try:
        asyncio.run(
            serve(
                handler,
                host=host,
                port=args.port,
                token=token,
                runs=runs,
                ready=write_ready,
            )
        )
    except KeyboardInterrupt:
        return 0
    finally:
        event_log.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
