"""HTTP and provider request transport for the stage-0 probe."""
from __future__ import annotations
import http.client
import json
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any
from endpoint_guard import https_exchange
from probe_parsing import parse_content

ROOT = Path(__file__).parents[2]
CASE_ID = "comedy_002"

def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected object")
    return value

def valid_line_spans(artifact: dict[str, Any]) -> set[str]:
    """Return every addressable story-package node.

    The historical name is retained for callers, but STORY_EVAL_V1 permits
    evidence on any real node_path, not only scene lines.
    """
    spans = {
        f"story-package/{artifact['logline']['node_id']}",
        f"story-package/{artifact['promise']['node_id']}",
    }
    for collection in ("characters", "beats", "episodes", "scenes"):
        for node in artifact[collection]:
            parent = f"story-package/{node['node_id']}"
            spans.add(parent)
            if collection == "episodes":
                spans.add(f"{parent}/{node['end_hook']['node_id']}")
            if collection == "scenes":
                spans.update(
                    f"{parent}/{line['node_id']}" for line in node["lines"]
                )
    for collection in ("facts", "relationships", "timeline", "setups"):
        spans.update(
            f"story-package/{node['node_id']}"
            for node in artifact["continuity_ledger"][collection]
        )
    return spans

def build_user_prompt(
    first: dict[str, Any],
    second: dict[str, Any],
    validation_error: str | None,
) -> str:
    return json.dumps(
        {
            "case_id": CASE_ID,
            "artifact_A": first,
            "artifact_B": second,
            "valid_span_refs_A": sorted(valid_line_spans(first)),
            "valid_span_refs_B": sorted(valid_line_spans(second)),
            "instruction": "spans 只能从对应 artifact 的 valid_span_refs 中逐字选择",
            "retry_instruction": (
                f"上次输出未通过校验：{validation_error}。请修正后完整重答。"
                if validation_error
                else None
            ),
        },
        ensure_ascii=False,
        separators=(",", ":"),
    )

def write_compatible_model_catalog(workdir: Path) -> Path | None:
    """Adapt a newer desktop cache for an older standalone CLI, without
    modifying the operator's shared cache.

    Codex CLI 0.120 requires `supports_reasoning_summaries`, while the current
    desktop cache omits it. The judge does not request reasoning summaries, so
    the conservative capability value is false.
    """
    source = Path.home() / ".codex" / "models_cache.json"
    if not source.exists():
        return None
    catalog = load(source)
    models = catalog.get("models")
    if not isinstance(models, list):
        return None
    changed = False
    for model in models:
        if (
            isinstance(model, dict)
            and "supports_reasoning_summaries" not in model
        ):
            model["supports_reasoning_summaries"] = False
            changed = True
        if isinstance(model, dict) and isinstance(
            model.get("supported_reasoning_levels"), list
        ):
            compatible_levels = [
                level
                for level in model["supported_reasoning_levels"]
                if isinstance(level, dict)
                and level.get("effort")
                in {"none", "minimal", "low", "medium", "high", "xhigh"}
            ]
            if compatible_levels != model["supported_reasoning_levels"]:
                model["supported_reasoning_levels"] = compatible_levels
                changed = True
    if not changed:
        return None
    destination = workdir / "models-catalog.compat.json"
    destination.write_text(
        json.dumps(catalog, ensure_ascii=False, separators=(",", ":")),
        encoding="utf-8",
    )
    return destination

def request_codex(
    route: dict[str, Any],
    model: str,
    system: str,
    first: dict[str, Any],
    second: dict[str, Any],
    validation_error: str | None,
    user_prompt: str | None = None,
) -> dict[str, Any]:
    schema_path = ROOT / route["output_schema"]
    prompt = (
        user_prompt
        if user_prompt is not None
        else f"{system}\n\n{build_user_prompt(first, second, validation_error)}"
    )
    with tempfile.TemporaryDirectory(prefix="story-judge-") as directory:
        workdir = Path(directory)
        output_path = workdir / "final.json"
        model_catalog = write_compatible_model_catalog(workdir)
        command = [
            route["command_path"],
            "exec",
            "--skip-git-repo-check",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--model",
            model,
            "--output-schema",
            str(schema_path),
            "--output-last-message",
            str(output_path),
            "--json",
            "--color",
            "never",
            "-",
        ]
        if model_catalog:
            command[2:2] = [
                "--config",
                f"model_catalog_json={json.dumps(str(model_catalog))}",
            ]
        completed = subprocess.run(
            command,
            input=prompt,
            cwd=workdir,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=int(route.get("request_timeout_seconds", 600)),
            shell=False,
            check=False,
        )
        if completed.returncode:
            detail = (completed.stderr or completed.stdout)[-1200:]
            raise RuntimeError(
                f"codex exec exited {completed.returncode}: {detail.strip()}"
            )
        result = json.loads(output_path.read_text(encoding="utf-8"))
        usage = None
        for line in completed.stdout.splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("type") == "turn.completed":
                usage = event.get("usage")
        result["_provider_usage"] = usage
        return result


def urlopen_with_retry(
    request: urllib.request.Request,
    timeout: int,
    attempts: int = 4,
) -> bytes:
    """Retry transient transport and throttling failures on one fixed route.

    Reads the body inside the loop too: a slow judge that sends headers and
    then stalls times out during read(), which is just as transient.
    """
    for attempt in range(1, attempts + 1):
        try:
            # Keep the historical test seam: callers that import the CLI
            # module may replace its exchange function with a deterministic
            # fake without changing the transport contract.
            exchange = https_exchange
            cli = sys.modules.get("run_stage0_probe")
            if cli is not None and hasattr(cli, "https_exchange"):
                exchange = cli.https_exchange
            return exchange(
                request.full_url,
                request.get_method(),
                {name: value for name, value in request.header_items()},
                request.data,
                timeout,
            )
        except urllib.error.HTTPError as error:
            retryable = error.code == 429 or 500 <= error.code < 600
            if not retryable or attempt == attempts:
                raise
            retry_after = error.headers.get("Retry-After")
            delay = min(float(retry_after), 60.0) if retry_after else 2 ** attempt
            error.read()
            print(f"RETRY HTTP {error.code}: waiting {delay:g}s", file=sys.stderr)
            time.sleep(delay)
        except (
            urllib.error.URLError,
            TimeoutError,
            http.client.RemoteDisconnected,
            ConnectionResetError,
            ConnectionAbortedError,
        ) as error:
            if attempt == attempts:
                raise
            delay = 2 ** attempt
            print(f"RETRY {type(error).__name__}: waiting {delay}s", file=sys.stderr)
            time.sleep(delay)
    raise AssertionError("unreachable")

def request(
    route: dict[str, Any],
    model: str,
    system: str,
    api_key: str | None,
    first: dict[str, Any],
    second: dict[str, Any],
    temperature: float,
    validation_error: str | None = None,
    user_prompt: str | None = None,
) -> dict[str, Any]:
    if route["provider"] == "local_codex_exec":
        return request_codex(
            route, model, system, first, second, validation_error,
            user_prompt=user_prompt,
        )
    if not api_key:
        raise RuntimeError(f"{route['provider']}: missing API key")
    prompt = (
        user_prompt
        if user_prompt is not None
        else build_user_prompt(first, second, validation_error)
    )
    request_body = {
        "model": route.get("model", model),
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
        ],
        "response_format": {"type": "json_object"},
        "temperature": temperature,
        "max_tokens": 8192,
    }
    if route.get("thinking"):
        request_body["thinking"] = route["thinking"]
    body = json.dumps(request_body, ensure_ascii=False).encode("utf-8")
    http = urllib.request.Request(
        route["endpoint"],
        data=body,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    response_body = urlopen_with_retry(
        http,
        timeout=int(route.get("request_timeout_seconds", 120)),
        attempts=int(route.get("transport_attempts", 2)),
    )
    try:
        raw = json.loads(response_body.decode("utf-8"))
    except json.JSONDecodeError as error:
        # a relay can cut the HTTP body mid-stream; as retryable as a timeout
        raise ValueError(f"truncated HTTP body from relay: {error}")
    result = parse_content(raw["choices"][0]["message"]["content"])
    result["_provider_usage"] = raw.get("usage")
    return result
