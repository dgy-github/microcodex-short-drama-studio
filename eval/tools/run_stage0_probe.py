"""Run the stage-0 pair past every configured judge, in both orders, N samples each.

What changed from the first version, and why:

- the baseline is read from the pair directory, not from `eval/runs/`, which
  `.gitignore` excludes; the pair must be reproducible on another machine;
- `generator_judge_disjoint` is computed from model families instead of being
  a hardcoded `True` that could never fail;
- every judge in `eval/judges.json` is used, satisfying the manifest's
  `min_judge_models`, instead of one model passed on the command line;
- each artifact is sampled `samples_per_artifact` times and aggregated by
  median, satisfying the parent contract's ban on selecting a single run;
- all ten rubric dimensions are scored, not one, so that sensitivity and
  specificity are computable. Specificity is the point: if planting a dialogue
  defect also moves producibility, the dimensions are not separable and the
  rubric is decorative.

Usage:
    python eval/tools/run_stage0_probe.py --check-connectivity
    python eval/tools/run_stage0_probe.py
"""

from __future__ import annotations

import argparse
import json
import os
import tempfile
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

import yaml

from probe_metrics import (
    consistency_metrics,
    median_pair_scores,
    median_scores,
    self_consistency,
    spans_for,
    specificity_metrics,
)

ROOT = Path(__file__).parents[2]
STAGE0 = ROOT / "eval" / "adversarial" / "stage0"
PAIR_DIR = STAGE0 / "motive-explicit"  # default; override with --pair-dir
JUDGES = ROOT / "eval" / "judges.json"
RUBRIC = ROOT / "eval" / "rubrics" / "judge-v1.yaml"
CASE_ID = "comedy_002"

SYSTEM_HEADER = """你是短剧评测员。你不知道哪个产物是基线，也不知道是否有人植入缺陷。
对 A、B 两个产物，按下列每个维度分别给 1-5 整数分。
每个维度都必须给出理由，并引用该产物中真实存在的 span_ref。
不要因为文本更长、措辞更清楚或情绪更强就提高分数。
理由不超过 200 字。"""

SYSTEM_FOOTER = """只输出 JSON，结构为：
{"A":{"<dimension_id>":{"score":1,"reason":"","spans":[]}, ...},
 "B":{"<dimension_id>":{"score":1,"reason":"","spans":[]}, ...},
 "preferred":"A|B|tie"}
两个产物都必须包含全部维度。"""


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected object")
    return value


def atomic_write(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("utf-8")
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as handle:
        temporary = Path(handle.name)
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)


def load_rubric() -> list[dict[str, Any]]:
    document = yaml.safe_load(RUBRIC.read_text(encoding="utf-8"))
    return document["dimensions"]


def build_system(dimensions: list[dict[str, Any]]) -> str:
    lines = [SYSTEM_HEADER, ""]
    for dimension in dimensions:
        anchors = dimension["anchors"]
        lines.append(
            f"- {dimension['id']}（{dimension['name']}）：{dimension['ask']}\n"
            f"  1分：{anchors[1]}\n  3分：{anchors[3]}\n  5分：{anchors[5]}"
        )
    lines.extend(["", SYSTEM_FOOTER])
    return "\n".join(lines)


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


def resolve_route(judge: dict[str, Any]) -> dict[str, Any]:
    """Pick one route up front and stay on it.

    Routes are alternate vendors for the same model. Falling back mid-probe
    would mean some samples came from one vendor and some from another, which
    is an uncontrolled variable inside a single measurement, so a route that
    breaks after selection is a hard failure rather than a reason to switch.
    """
    for route in judge["routes"]:
        if (
            route.get("endpoint")
            and os.environ.get(route["api_key_env"])
            and not route.get("blocked_on")
        ):
            return route
    raise SystemExit(
        f"{judge['model']}: no unblocked route has both an endpoint and a key set; "
        "run --check-connectivity"
    )


def request(
    route: dict[str, Any],
    model: str,
    system: str,
    api_key: str,
    first: dict[str, Any],
    second: dict[str, Any],
    temperature: float,
    validation_error: str | None = None,
) -> dict[str, Any]:
    prompt = json.dumps(
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
    with urllib.request.urlopen(http, timeout=300) as response:
        raw = json.loads(response.read().decode("utf-8"))
    result = json.loads(raw["choices"][0]["message"]["content"])
    result["_provider_usage"] = raw.get("usage")
    return result


def validate_judgment(
    value: dict[str, Any],
    first: dict[str, Any],
    second: dict[str, Any],
    dimension_ids: list[str],
) -> None:
    allowed = {"A": valid_line_spans(first), "B": valid_line_spans(second)}
    for label in ("A", "B"):
        block = value.get(label)
        if not isinstance(block, dict):
            raise ValueError(f"{label} must be an object of dimensions")
        missing = [d for d in dimension_ids if d not in block]
        if missing:
            raise ValueError(f"{label} missing dimensions: {missing}")
        for dimension in dimension_ids:
            entry = block[dimension]
            score = entry.get("score")
            if not isinstance(score, int) or not 1 <= score <= 5:
                raise ValueError(f"{label}.{dimension}.score must be 1-5")
            if not entry.get("reason") or not entry.get("spans"):
                raise ValueError(f"{label}.{dimension} requires a reason and spans")
            invalid = set(entry["spans"]) - allowed[label]
            if invalid:
                raise ValueError(
                    f"{label}.{dimension} invalid spans: {sorted(invalid)}"
                )
    if value.get("preferred") not in {"A", "B", "tie"}:
        raise ValueError("preferred must be A, B, or tie")


def normalize_owned_field_spans(
    value: dict[str, Any],
    first: dict[str, Any],
    second: dict[str, Any],
) -> None:
    """Map a cited field back to its unambiguous owning addressable node."""
    allowed = {"A": valid_line_spans(first), "B": valid_line_spans(second)}
    artifacts = {"A": first, "B": second}
    for label in ("A", "B"):
        for entry in value.get(label, {}).values():
            if not isinstance(entry, dict) or not isinstance(entry.get("spans"), list):
                continue
            normalized: list[Any] = []
            for span in entry["spans"]:
                if span in {
                    "story-package/production",
                    "story-package/production/locations",
                }:
                    normalized.extend(
                        f"story-package/{scene['node_id']}"
                        for scene in artifacts[label]["scenes"]
                    )
                    if span != "story-package/production":
                        continue
                if span in {
                    "story-package/production",
                    "story-package/production/speaking_cast",
                }:
                    normalized.extend(artifacts[label]["production"]["speaking_cast"])
                    continue
                candidate = span.split(".", 1)[0] if isinstance(span, str) else span
                while (
                    isinstance(candidate, str)
                    and candidate not in allowed[label]
                    and "/" in candidate.removeprefix("story-package/")
                ):
                    candidate = candidate.rsplit("/", 1)[0]
                normalized.append(
                    candidate if candidate in allowed[label] else span
                )
            entry["spans"] = list(dict.fromkeys(normalized))


def request_validated(
    route: dict[str, Any],
    model: str,
    system: str,
    api_key: str,
    first: dict[str, Any],
    second: dict[str, Any],
    temperature: float,
    dimension_ids: list[str],
    max_attempts: int = 3,
) -> dict[str, Any]:
    validation_error: str | None = None
    for attempt in range(1, max_attempts + 1):
        try:
            sample = request(
                route,
                model,
                system,
                api_key,
                first,
                second,
                temperature,
                validation_error,
            )
        except (json.JSONDecodeError, KeyError, TypeError) as error:
            validation_error = f"{type(error).__name__}: {error}"
            if attempt == max_attempts:
                raise
            print(
                f"RETRY {model} via {route['provider']}: "
                f"unparseable judge output ({validation_error})"
            )
            continue
        normalize_owned_field_spans(sample, first, second)
        try:
            validate_judgment(sample, first, second, dimension_ids)
            return sample
        except ValueError as error:
            validation_error = str(error)
            if attempt == max_attempts:
                raise
            print(
                f"RETRY {model} via {route['provider']}: "
                f"invalid judge output ({validation_error})"
            )
    raise AssertionError("unreachable")


def check_connectivity(judges: list[dict[str, Any]]) -> int:
    """Ping every route of every judge with one minimal request.

    Verifies three things at once and costs almost nothing: the endpoint path,
    the key, and whether the provider honours JSON mode. Run this before
    spending a full probe.
    """
    failures = 0
    for judge in judges:
        for route in judge["routes"]:
            name = f"{judge['model']} via {route['provider']}"
            if not route.get("endpoint"):
                print(f"SKIP {name}: no endpoint recorded in eval/judges.json")
                failures += 1
                continue
            api_key = os.environ.get(route["api_key_env"])
            if not api_key:
                print(f"SKIP {name}: {route['api_key_env']} not set")
                failures += 1
                continue
            request_body = {
                "model": route.get("model", judge["model"]),
                "messages": [
                    {"role": "system", "content": '只输出 JSON：{"ok":true}'},
                    {"role": "user", "content": "ping"},
                ],
                "response_format": {"type": "json_object"},
                "temperature": 0,
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
            try:
                with urllib.request.urlopen(http, timeout=60) as response:
                    raw = json.loads(response.read().decode("utf-8"))
                json.loads(raw["choices"][0]["message"]["content"])
                print(f"OK   {name}: endpoint reachable, JSON mode honoured")
            except urllib.error.HTTPError as error:
                # The provider's own error code lives in the body. Swallowing
                # it turns "quota exhausted" and "wrong model id" into the same
                # unactionable status line.
                detail = error.read().decode("utf-8", errors="replace")[:400]
                print(f"FAIL {name}: HTTP {error.code}: {detail}")
                failures += 1
            except (urllib.error.URLError, KeyError, ValueError) as error:
                print(f"FAIL {name}: {type(error).__name__}: {error}")
                failures += 1
    return failures


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-connectivity", action="store_true")
    parser.add_argument(
        "--pair-dir",
        type=Path,
        default=PAIR_DIR,
        help="pair directory to probe; defaults to the first stage-0 pair",
    )
    parser.add_argument("--samples", type=int, default=None)
    parser.add_argument("--only-judge", default=None)
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--result-suffix", default="")
    return parser.parse_args()


def select_judges(
    config: dict[str, Any], only_judge: str | None
) -> tuple[list[dict[str, Any]], str, set[str]]:
    judges = config["judges"]
    generator_family = config["generator"]["family"]
    judge_families = {judge["family"] for judge in judges}
    if generator_family in judge_families:
        raise SystemExit(
            f"generator family {generator_family!r} also appears in the judge set; "
            "the manifest requires them disjoint"
        )
    if len(judge_families) < 2:
        raise SystemExit(
            f"manifest requires at least 2 judge families, got {sorted(judge_families)}"
        )
    if only_judge:
        judges = [judge for judge in judges if judge["model"] == only_judge]
        if not judges:
            raise SystemExit(f"unknown judge model: {only_judge}")
    return judges, generator_family, judge_families


def resolve_pair_dir(value: Path) -> Path:
    pair_dir = value if value.is_absolute() else ROOT / value
    if not (pair_dir / "pair.json").exists():
        raise SystemExit(f"no pair.json under {pair_dir}")
    return pair_dir


def saved_result_is_reusable(
    saved: dict[str, Any],
    judge: dict[str, Any],
    route: dict[str, Any],
    samples_per_artifact: int,
) -> bool:
    summary = saved.get("summary", {})
    return (
        summary.get("judge_model") == judge["model"]
        and summary.get("route_provider") == route["provider"]
        and summary.get("samples_per_artifact") == samples_per_artifact
        and len(saved.get("forward", [])) == samples_per_artifact
        and len(saved.get("reverse", [])) == samples_per_artifact
    )


def refresh_saved_summary(
    saved: dict[str, Any],
    target: str,
    dimensions: list[dict[str, Any]],
) -> dict[str, Any]:
    summary = saved["summary"]
    dimension_ids = [dimension["id"] for dimension in dimensions]
    summary.update(consistency_metrics(saved["forward"], saved["reverse"], dimension_ids))
    summary.update(
        specificity_metrics(
            summary["baseline_scores"], summary["negative_scores"], target, dimensions
        )
    )
    return summary


def collect_samples(
    route: dict[str, Any],
    judge: dict[str, Any],
    system: str,
    api_key: str,
    first: dict[str, Any],
    second: dict[str, Any],
    temperature: float,
    dimension_ids: list[str],
    count: int,
) -> list[dict[str, Any]]:
    return [
        request_validated(
            route,
            judge["model"],
            system,
            api_key,
            first,
            second,
            temperature,
            dimension_ids,
        )
        for _ in range(count)
    ]


def summarize_judge(
    judge: dict[str, Any],
    route: dict[str, Any],
    forward: list[dict[str, Any]],
    reverse: list[dict[str, Any]],
    dimensions: list[dict[str, Any]],
    target: str,
    defect_spans: set[str],
    temperature: float,
) -> dict[str, Any]:
    dimension_ids = [dimension["id"] for dimension in dimensions]
    baseline_scores, negative_scores = median_pair_scores(
        forward, reverse, dimension_ids
    )
    cited = spans_for(forward, "B") | spans_for(reverse, "A")
    result = {
        "judge_model": judge["model"],
        "judge_family": judge["family"],
        "route_provider": route["provider"],
        "route_endpoint": route["endpoint"],
        "samples_per_artifact": len(forward),
        "temperature": temperature,
        "baseline_scores": baseline_scores,
        "negative_scores": negative_scores,
        "target_dimension": target,
        "sensitivity": negative_scores[target] < baseline_scores[target],
        "order_consistent": all(sample["preferred"] == "A" for sample in forward)
        and all(sample["preferred"] == "B" for sample in reverse),
        "seeded_defect_localized": bool(cited & defect_spans),
        "cited_span_precision": (
            len(cited & defect_spans) / len(cited) if cited else 0.0
        ),
    }
    result.update(consistency_metrics(forward, reverse, dimension_ids))
    result.update(
        specificity_metrics(baseline_scores, negative_scores, target, dimensions)
    )
    return result


def run_judge(
    judge: dict[str, Any],
    pair_dir: Path,
    baseline: dict[str, Any],
    negative: dict[str, Any],
    dimensions: list[dict[str, Any]],
    system: str,
    target: str,
    defect_spans: set[str],
    samples_per_artifact: int,
    temperature: float,
    result_suffix: str,
    force: bool,
) -> dict[str, Any]:
    route = resolve_route(judge)
    suffix = f".{result_suffix}" if result_suffix else ""
    result_path = pair_dir / f"judge-{judge['model']}{suffix}.result.json"
    if result_path.exists() and not force:
        saved = load(result_path)
        if saved_result_is_reusable(saved, judge, route, samples_per_artifact):
            summary = refresh_saved_summary(saved, target, dimensions)
            saved["summary"] = summary
            atomic_write(result_path, saved)
            print(
                f"RESUME {judge['model']} via {route['provider']}: "
                "reusing complete saved samples"
            )
            return summary
    api_key = os.environ[route["api_key_env"]]
    dimension_ids = [dimension["id"] for dimension in dimensions]
    forward = collect_samples(
        route, judge, system, api_key, baseline, negative,
        temperature, dimension_ids, samples_per_artifact,
    )
    reverse = collect_samples(
        route, judge, system, api_key, negative, baseline,
        temperature, dimension_ids, samples_per_artifact,
    )
    summary = summarize_judge(
        judge, route, forward, reverse, dimensions, target, defect_spans, temperature
    )
    atomic_write(
        result_path, {"forward": forward, "reverse": reverse, "summary": summary}
    )
    return summary


def build_probe_summary(
    config: dict[str, Any],
    pair: dict[str, Any],
    judge_families: set[str],
    per_judge: list[dict[str, Any]],
) -> dict[str, Any]:
    generator_family = config["generator"]["family"]
    return {
        "schema": "stage0-probe-result/v2",
        "pair_id": pair["pair_id"],
        "generator_model": config["generator"]["model"],
        "generator_family": generator_family,
        "generator_judge_disjoint": generator_family not in judge_families,
        "judge_families": sorted(judge_families),
        "judges": per_judge,
        "independence_caveat": config["independence_caveat"],
        "all_judges_detected": all(judge["sensitivity"] for judge in per_judge),
        "min_specificity_all": min(
            judge["specificity_all"] for judge in per_judge
        ),
        "min_specificity_cross_pillar": min(
            judge["specificity_cross_pillar"] for judge in per_judge
        ),
        "min_specificity": min(judge["specificity_all"] for judge in per_judge),
        "status": (
            "measurable_gap"
            if all(
                judge["sensitivity"] and judge["order_consistent"]
                for judge in per_judge
            )
            else "probe_failed"
        ),
    }


def main() -> int:
    args = parse_args()
    config = load(JUDGES)
    if args.check_connectivity:
        return 1 if check_connectivity(config["judges"]) else 0
    judges, _, judge_families = select_judges(config, args.only_judge)
    sampling = config["sampling"]
    samples_per_artifact = args.samples or sampling["samples_per_artifact"]
    dimensions = load_rubric()
    pair_dir = resolve_pair_dir(args.pair_dir)
    baseline = load(pair_dir / "baseline.story-package.json")
    negative = load(pair_dir / "negative.story-package.json")
    pair = load(pair_dir / "pair.json")
    defect = pair["seeded_defects"][0]
    per_judge = [
        run_judge(
            judge, pair_dir, baseline, negative, dimensions,
            build_system(dimensions), defect["target_dimension"],
            set(defect["spans"]), samples_per_artifact, sampling["temperature"],
            args.result_suffix, args.force,
        )
        for judge in judges
    ]
    summary = build_probe_summary(config, pair, judge_families, per_judge)
    suffix = f".{args.result_suffix}" if args.result_suffix else ""
    atomic_write(pair_dir / f"probe-summary{suffix}.json", summary)
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
