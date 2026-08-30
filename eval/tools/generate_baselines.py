"""Generate model-only story-package/v1 baselines for evaluation cases."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import tempfile
import urllib.error
import urllib.request

from endpoint_guard import https_exchange
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable

try:
    import jsonschema
except ImportError as error:  # pragma: no cover
    raise SystemExit(
        "generate_baselines.py requires jsonschema; install eval/tools/requirements.txt"
    ) from error


ROOT = Path(__file__).parents[2]
DEFAULT_CASES = ROOT / "eval" / "cases" / "dev" / "cases.jsonl"
PACKAGE_SCHEMA = ROOT / "schemas" / "story-package-v1.json"
ARTIFACT_SCHEMA = ROOT / "schemas" / "story-artifact-v1.json"

SYSTEM_PROMPT = """你是短剧故事策划。只输出一个 JSON 对象，不要 Markdown，不要解释。
输出必须严格符合给定的 story-package/v1 JSON Schema。
所有 node_id 在同级数组内从 1 连续编号；所有 span_ref 必须引用真实节点。
不得复述或模仿任何受保护作品，不要加入输入未要求的检索材料。
所有故事文本必须使用简体中文；JSON 字段名和枚举值保持 Schema 原文。
人物行为必须由压力、选择与后果推动；代表场景至少两场，台词应包含潜台词。
必须遵守集数、场地、说话角色、required_elements、required_conditions 和 forbidden_elements。
required_elements 中的每个字符串必须至少逐字出现在一个故事文本字段中，不能只写近义词。
这是未修改的基线，不要故意植入缺陷。"""


@dataclass(frozen=True)
class ProviderConfig:
    endpoint: str
    model: str
    api_key: str
    timeout_seconds: float
    temperature: float
    seed: int


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected a JSON object")
    return value


def load_cases(path: Path) -> list[dict[str, Any]]:
    cases = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        value = json.loads(line)
        if not isinstance(value, dict):
            raise ValueError(f"{path}:{line_number}: case must be an object")
        cases.append(value)
    return cases


def build_prompt(case: dict[str, Any], schema: dict[str, Any]) -> str:
    payload = {
        "case": {
            "case_id": case["case_id"],
            "input": case["input"],
            "genre": case["genre"],
            "constraints": case["constraints"],
            "required_elements": case["required_elements"],
            "required_conditions": case["required_conditions"],
            "forbidden_elements": case["forbidden_elements"],
        },
        "schema": schema,
    }
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"))


def extract_content(response: dict[str, Any]) -> str:
    if isinstance(response.get("output_text"), str):
        return response["output_text"]
    try:
        content = response["choices"][0]["message"]["content"]
    except (KeyError, IndexError, TypeError) as error:
        raise ValueError("provider response has no supported text field") from error
    if not isinstance(content, str):
        raise ValueError("provider response content must be text")
    return content


def request_model(config: ProviderConfig, prompt: str) -> dict[str, Any]:
    body = json.dumps(
        {
            "model": config.model,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": prompt},
            ],
            "response_format": {"type": "json_object"},
            "temperature": config.temperature,
            "seed": config.seed,
        },
        ensure_ascii=False,
    ).encode("utf-8")
    request = urllib.request.Request(
        config.endpoint,
        data=body,
        headers={
            "Authorization": f"Bearer {config.api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        result = json.loads(
            https_exchange(
                request.full_url,
                request.get_method(),
                {name: value for name, value in request.header_items()},
                request.data,
                config.timeout_seconds,
            ).decode("utf-8")
        )
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")[:1000]
        raise RuntimeError(f"provider HTTP {error.code}: {detail}") from error
    except urllib.error.URLError as error:
        raise RuntimeError(f"provider request failed: {error.reason}") from error
    if not isinstance(result, dict):
        raise ValueError("provider response must be a JSON object")
    return result


def canonical_bytes(value: dict[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("utf-8")


def atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as handle:
        temporary = Path(handle.name)
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    try:
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def prepare_package(
    raw_content: str, case: dict[str, Any], run_id: str, seed: int
) -> dict[str, Any]:
    package = json.loads(raw_content)
    if not isinstance(package, dict):
        raise ValueError("model output must be a JSON object")
    case_id = case["case_id"]
    package["schema"] = "story-package/v1"
    package["package_id"] = f"{run_id}-{case_id}-seed-{seed}"
    package["job_id"] = f"{run_id}-{case_id}"
    package["case_id"] = case_id
    package.pop("supersedes", None)
    package.pop("node_correspondence", None)
    package["provenance"] = [
        {
            "source_id": case_id,
            "license_id": case["rights"]["license_id"],
            "usage": "evaluation baseline prompt",
        }
    ]
    normalize_span_refs(package)
    normalize_node_ids(package)
    canonicalize_scene_line_ids(package)
    reconcile_scene_locations(package)
    for beat in package.get("beats", []):
        actor = beat.get("actor")
        if actor is not None and not re.fullmatch(
            r"story-package(?:/[a-z]+-[0-9]+)+", actor
        ):
            beat.pop("actor")
    return package


def normalize_span_refs(value: Any) -> Any:
    """Canonicalize common JSON-tree paths to story-package flat node paths."""
    containers = {
        "characters",
        "beats",
        "episodes",
        "scenes",
        "lines",
        "continuity_ledger",
        "facts",
        "relationships",
        "timeline",
        "setups",
    }
    if isinstance(value, str) and value.startswith("story-package/"):
        segments = value.split("/")
        return "/".join(
            segment
            for index, segment in enumerate(segments)
            if index == 0 or segment not in containers
        )
    if isinstance(value, list):
        for index, item in enumerate(value):
            value[index] = normalize_span_refs(item)
    elif isinstance(value, dict):
        for key, item in value.items():
            value[key] = normalize_span_refs(item)
    return value


def normalize_node_ids(value: Any) -> None:
    """Remove parent path prefixes mistakenly embedded in leaf node_id values."""
    if isinstance(value, list):
        for item in value:
            normalize_node_ids(item)
    elif isinstance(value, dict):
        node_id = value.get("node_id")
        if isinstance(node_id, str):
            leaf_match = re.search(r"([a-z]+-[0-9]+)$", node_id)
            if leaf_match:
                value["node_id"] = leaf_match.group(1)
        for item in value.values():
            normalize_node_ids(item)


def reconcile_scene_locations(package: dict[str, Any]) -> None:
    """Use the more specific scene spelling for an already-declared location."""
    declared = package.get("production", {}).get("locations", [])
    scene_locations = {
        scene.get("location")
        for scene in package.get("scenes", [])
        if isinstance(scene.get("location"), str)
    }
    story_without_production = {
        key: value for key, value in package.items() if key != "production"
    }
    story_text = json.dumps(story_without_production, ensure_ascii=False)
    for scene in package.get("scenes", []):
        location = scene.get("location")
        if not isinstance(location, str) or location in declared:
            continue
        matches = [
            index
            for index, candidate in enumerate(declared)
            if isinstance(candidate, str)
            and (candidate in location or location in candidate)
        ]
        if len(matches) == 1:
            declared[matches[0]] = location
            continue
        unused = [
            index
            for index, candidate in enumerate(declared)
            if candidate not in scene_locations and candidate not in story_text
        ]
        if unused:
            declared[unused[-1]] = location


def canonicalize_scene_line_ids(package: dict[str, Any]) -> None:
    """Assign schema-shaped IDs from line kind and preserve any references."""
    replacements: dict[str, str] = {}
    for scene in package.get("scenes", []):
        scene_id = scene.get("node_id")
        counters = {"action": 0, "dialogue": 0}
        for line in scene.get("lines", []):
            kind = line.get("kind")
            if kind not in counters:
                continue
            counters[kind] += 1
            old_id = line.get("node_id")
            new_id = f"{kind}-{counters[kind]}"
            if isinstance(old_id, str) and old_id != new_id:
                replacements[f"story-package/{scene_id}/{old_id}"] = (
                    f"story-package/{scene_id}/{new_id}"
                )
                line["node_id"] = new_id

    def replace(value: Any) -> Any:
        if isinstance(value, str):
            return replacements.get(value, value)
        if isinstance(value, list):
            for index, item in enumerate(value):
                value[index] = replace(item)
        elif isinstance(value, dict):
            for key, item in value.items():
                value[key] = replace(item)
        return value

    replace(package)


def collect_package_refs(package: dict[str, Any]) -> tuple[set[str], set[str]]:
    known_refs = {
        f"story-package/{package['logline']['node_id']}",
        f"story-package/{package['promise']['node_id']}",
    }
    for collection in ("characters", "beats", "episodes", "scenes"):
        for node in package[collection]:
            parent_ref = f"story-package/{node['node_id']}"
            known_refs.add(parent_ref)
            if collection == "episodes":
                known_refs.add(f"{parent_ref}/{node['end_hook']['node_id']}")
            if collection == "scenes":
                known_refs.update(f"{parent_ref}/{line['node_id']}" for line in node["lines"])

    for collection in ("facts", "relationships", "timeline", "setups"):
        known_refs.update(
            f"story-package/{node['node_id']}"
            for node in package["continuity_ledger"][collection]
        )
    referenced: set[str] = set()

    def collect(value: Any) -> None:
        if isinstance(value, str) and re.fullmatch(r"story-package(?:/[a-z]+-[0-9]+)+", value):
            referenced.add(value)
        elif isinstance(value, list):
            for item in value:
                collect(item)
        elif isinstance(value, dict):
            for item in value.values():
                collect(item)

    collect(package)
    return known_refs, referenced


def collect_natural_text(package: dict[str, Any]) -> str:
    keys = {"text", "name", "desire", "fear", "contradiction", "secret", "change", "pressure", "choice", "consequence", "opening_state", "conflict", "turn", "location", "statement", "relation", "when", "event", "tone"}
    values: list[str] = []

    def collect(value: Any) -> None:
        if isinstance(value, list):
            for item in value:
                collect(item)
        elif isinstance(value, dict):
            for key, item in value.items():
                if key in keys and isinstance(item, str):
                    values.append(item)
                else:
                    collect(item)

    collect(package)
    return "\n".join(values)


def validate_package(
    package: dict[str, Any], case: dict[str, Any], schema: dict[str, Any]
) -> None:
    jsonschema.Draft202012Validator(schema).validate(package)
    constraints = case["constraints"]
    if len(package["episodes"]) != constraints["episodes"]:
        raise ValueError(
            f"episode count {len(package['episodes'])} != {constraints['episodes']}"
        )
    if len(package["production"]["locations"]) > constraints["max_locations"]:
        raise ValueError("production.locations exceeds max_locations")
    if len(package["production"]["speaking_cast"]) > constraints["max_speaking_cast"]:
        raise ValueError("production.speaking_cast exceeds max_speaking_cast")

    known_refs, referenced = collect_package_refs(package)
    dangling = sorted(referenced - known_refs)
    if dangling:
        raise ValueError(f"dangling span_ref values: {', '.join(dangling)}")

    language_sample = collect_natural_text(package)
    cjk_count = len(re.findall(r"[\u4e00-\u9fff]", language_sample))
    latin_count = len(re.findall(r"[A-Za-z]", language_sample))
    if cjk_count == 0 or latin_count > cjk_count:
        raise ValueError(
            f"story language must be Chinese (cjk={cjk_count}, latin={latin_count})"
        )
    serialized = json.dumps(package, ensure_ascii=False)
    missing_elements = [
        element for element in case["required_elements"] if element not in serialized
    ]
    if missing_elements:
        raise ValueError(
            f"missing required_elements: {', '.join(sorted(missing_elements))}"
        )
    present_forbidden = [
        element for element in case["forbidden_elements"] if element in serialized
    ]
    if present_forbidden:
        raise ValueError(
            f"present forbidden_elements: {', '.join(sorted(present_forbidden))}"
        )

    declared_locations = set(package["production"]["locations"])
    scene_locations = {scene["location"] for scene in package["scenes"]}
    if not scene_locations <= declared_locations:
        raise ValueError("scene location is missing from production.locations")
    declared_cast = set(package["production"]["speaking_cast"])
    speakers = {
        line["speaker"]
        for scene in package["scenes"]
        for line in scene["lines"]
        if line["kind"] == "dialogue"
    }
    if not speakers <= declared_cast:
        raise ValueError("dialogue speaker is missing from production.speaking_cast")


def make_wrapper(
    package: dict[str, Any],
    package_path: Path,
    run_dir: Path,
    content_hash: str,
) -> dict[str, Any]:
    return {
        "schema": "story-artifact/v1",
        "artifact_id": package["package_id"],
        "artifact_type": "story-package",
        "content_ref": package_path.relative_to(run_dir).as_posix(),
        "content_hash": content_hash,
        "provenance": package["provenance"],
    }


def generate_one(
    case: dict[str, Any],
    run_id: str,
    run_dir: Path,
    package_schema: dict[str, Any],
    artifact_schema: dict[str, Any],
    config: ProviderConfig,
    requester: Callable[[ProviderConfig, str], dict[str, Any]] = request_model,
) -> tuple[Path, Path]:
    case_id = case["case_id"]
    package_path = run_dir / "artifacts" / f"{case_id}.story-package.json"
    wrapper_path = run_dir / "artifacts" / f"{case_id}.artifact.json"
    response_path = run_dir / "responses" / f"{case_id}.provider.json"
    if package_path.exists() or wrapper_path.exists():
        raise FileExistsError(f"{case_id}: artifact already exists; refusing to overwrite")

    if response_path.exists():
        response = load_json(response_path)
    else:
        response = requester(config, build_prompt(case, package_schema))
        atomic_write(response_path, canonical_bytes(response))
    package = prepare_package(extract_content(response), case, run_id, config.seed)
    validate_package(package, case, package_schema)
    package_bytes = canonical_bytes(package)
    content_hash = "sha256:" + hashlib.sha256(package_bytes).hexdigest()
    wrapper = make_wrapper(package, package_path, run_dir, content_hash)
    jsonschema.Draft202012Validator(artifact_schema).validate(wrapper)

    atomic_write(package_path, package_bytes)
    atomic_write(wrapper_path, canonical_bytes(wrapper))
    return package_path, wrapper_path


def select_cases(
    cases: list[dict[str, Any]], requested_ids: list[str]
) -> list[dict[str, Any]]:
    if not requested_ids:
        return cases
    by_id = {case["case_id"]: case for case in cases}
    missing = sorted(set(requested_ids) - by_id.keys())
    if missing:
        raise ValueError(f"unknown case ids: {', '.join(missing)}")
    return [by_id[case_id] for case_id in requested_ids]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--endpoint", help="OpenAI-compatible chat completions URL")
    parser.add_argument("--model", help="provider model identifier")
    parser.add_argument("--api-key-env", default="MODEL_API_KEY")
    parser.add_argument("--cases", type=Path, default=DEFAULT_CASES)
    parser.add_argument("--case-id", action="append", default=[])
    parser.add_argument("--run-id")
    parser.add_argument("--output-root", type=Path, default=ROOT / "eval" / "runs")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--temperature", type=float, default=0.7)
    parser.add_argument("--timeout-seconds", type=float, default=180.0)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    cases = select_cases(load_cases(args.cases), args.case_id)
    package_schema = load_json(PACKAGE_SCHEMA)
    artifact_schema = load_json(ARTIFACT_SCHEMA)

    if args.dry_run:
        for case in cases:
            print(f"{case['case_id']}\tprompt_chars={len(build_prompt(case, package_schema))}")
        return 0

    if not args.endpoint or not args.model or not args.run_id:
        raise SystemExit("--endpoint, --model, and --run-id are required")
    api_key = os.environ.get(args.api_key_env)
    if not api_key:
        raise SystemExit(f"missing API key environment variable: {args.api_key_env}")

    run_dir = args.output_root / args.run_id
    config_record = {
        "schema": "eval-generation-run/v1",
        "run_id": args.run_id,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "generator": {
            "kind": "model_only",
            "endpoint": args.endpoint,
            "model": args.model,
            "seed": args.seed,
            "temperature": args.temperature,
        },
        "case_ids": [case["case_id"] for case in cases],
        "story_package_schema": "story-package/v1",
    }
    config_path = run_dir / "config.json"
    if config_path.exists():
        existing = load_json(config_path)
        comparable_existing = dict(existing)
        comparable_existing.pop("created_at", None)
        comparable_existing.pop("case_ids", None)
        comparable_existing["generator"] = dict(comparable_existing["generator"])
        comparable_existing["generator"].pop("endpoint_history", None)
        comparable_new = dict(config_record)
        comparable_new.pop("created_at", None)
        comparable_new.pop("case_ids", None)
        if comparable_existing != comparable_new:
            raise SystemExit(f"{config_path}: conflicting run configuration")
        existing_ids = existing.get("case_ids", [])
        merged_ids = list(dict.fromkeys([*existing_ids, *config_record["case_ids"]]))
        if merged_ids != existing_ids:
            existing["case_ids"] = merged_ids
            atomic_write(config_path, canonical_bytes(existing))
    else:
        atomic_write(config_path, canonical_bytes(config_record))

    config = ProviderConfig(
        endpoint=args.endpoint,
        model=args.model,
        api_key=api_key,
        timeout_seconds=args.timeout_seconds,
        temperature=args.temperature,
        seed=args.seed,
    )
    for case in cases:
        package_path = run_dir / "artifacts" / f"{case['case_id']}.story-package.json"
        wrapper_path = run_dir / "artifacts" / f"{case['case_id']}.artifact.json"
        if package_path.exists() and wrapper_path.exists():
            print(f"SKIP {case['case_id']}: complete artifact pair exists")
            continue
        generated, _ = generate_one(
            case,
            args.run_id,
            run_dir,
            package_schema,
            artifact_schema,
            config,
        )
        print(f"OK {case['case_id']}: {generated}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
