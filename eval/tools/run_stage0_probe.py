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
import hashlib
import http.client
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

import yaml

from endpoint_guard import assert_public_https_endpoint, https_exchange
from probe_metrics import (
    consistency_metrics,
    krippendorff_alpha_interval,
    median_pair_scores,
    median_scores,
    self_consistency,
    spans_for,
    specificity_metrics,
)
from probe_parsing import parse_content
from probe_judging import (
    normalize_owned_field_spans,
    normalize_span_list,
    request_validated,
    validate_judgment,
)
from probe_transport import (
    build_user_prompt,
    load,
    request,
    request_codex,
    urlopen_with_retry,
    valid_line_spans,
    write_compatible_model_catalog,
)

ROOT = Path(__file__).parents[2]
STAGE0 = ROOT / "eval" / "adversarial" / "stage0"
PAIR_DIR = STAGE0 / "motive-explicit"  # default; override with --pair-dir
JUDGES = ROOT / "eval" / "judges.json"
CODEX_JUDGES = ROOT / "eval" / "codex-judge.json"
RUBRIC = ROOT / "eval" / "rubrics" / "judge-v1.yaml"
MANIFEST = ROOT / "eval" / "manifests" / "eval-v0.1.0.json"
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


def input_fingerprint(pair_dir: Path) -> str:
    digest = hashlib.sha256()
    paths = [
        pair_dir / "pair.json",
        pair_dir / "baseline.story-package.json",
        pair_dir / "negative.story-package.json",
        RUBRIC,
        JUDGES,
        MANIFEST,
    ]
    for path in paths:
        relative = path.relative_to(ROOT).as_posix().encode("utf-8")
        content = path.read_bytes()
        digest.update(len(relative).to_bytes(4, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return f"sha256:{digest.hexdigest()}"


def judge_config_fingerprint(judge: dict[str, Any]) -> str:
    encoded = json.dumps(
        judge, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def load_probe_config() -> dict[str, Any]:
    """Merge supplemental local judges without changing the remote input hash."""
    config = load(JUDGES)
    supplemental = load(CODEX_JUDGES)
    formal_models = {judge["model"] for judge in config["judges"]}
    # A supplemental judge promoted into the formal set must not be counted
    # twice: a duplicated rater silently inflates inter-model agreement.
    config["judges"] = [
        *config["judges"],
        *(
            judge
            for judge in supplemental["judges"]
            if judge["model"] not in formal_models
        ),
    ]
    config["independence_caveat"] = supplemental["independence_caveat"]
    return config




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




def resolve_route(judge: dict[str, Any]) -> dict[str, Any]:
    """Pick one route up front and stay on it.

    Routes are alternate vendors for the same model. Falling back mid-probe
    would mean some samples came from one vendor and some from another, which
    is an uncontrolled variable inside a single measurement, so a route that
    breaks after selection is a hard failure rather than a reason to switch.
    """
    for route in judge["routes"]:
        if (
            route.get("provider") == "local_codex_exec"
            and not route.get("blocked_on")
        ):
            command_path = shutil.which(route.get("command", "codex"))
            if command_path:
                return {**route, "command_path": command_path}
        if (
            route.get("endpoint")
            and os.environ.get(route["api_key_env"])
            and not route.get("blocked_on")
        ):
            return route
    raise SystemExit(
        f"{judge['model']}: no usable local command or unblocked route has "
        "both an endpoint and a key set; "
        "run --check-connectivity"
    )












def check_local_connectivity(judge: dict[str, Any], route: dict[str, Any]) -> bool:
    name = f"{judge['model']} via {route['provider']}"
    command_path = shutil.which(route.get("command", "codex"))
    if not command_path:
        print(f"FAIL {name}: command not found")
        return False
    completed = subprocess.run(
        [command_path, "login", "status"], capture_output=True, text=True,
        encoding="utf-8", errors="replace", timeout=30, shell=False, check=False,
    )
    if completed.returncode:
        detail = (completed.stderr or completed.stdout).strip()[:400]
        print(f"FAIL {name}: {detail}")
        return False
    print(f"OK   {name}: command found and login is active")
    return True


def check_http_connectivity(judge: dict[str, Any], route: dict[str, Any]) -> bool:
    name = f"{judge['model']} via {route['provider']}"
    if not route.get("endpoint"):
        print(f"SKIP {name}: no endpoint recorded in eval/judges.json")
        return False
    api_key = os.environ.get(route["api_key_env"])
    if not api_key:
        print(f"SKIP {name}: {route['api_key_env']} not set")
        return False
    body = json.dumps({
        "model": route.get("model", judge["model"]),
        "messages": [{"role": "system", "content": '只输出 JSON：{"ok":true}'}, {"role": "user", "content": "ping"}],
        "response_format": {"type": "json_object"}, "temperature": 0,
        **({"thinking": route["thinking"]} if route.get("thinking") else {}),
    }, ensure_ascii=False).encode("utf-8")
    headers = {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"}
    try:
        raw = json.loads(https_exchange(route["endpoint"], "POST", headers, body, 60).decode("utf-8"))
        json.loads(raw["choices"][0]["message"]["content"])
        print(f"OK   {name}: endpoint reachable, JSON mode honoured")
        return True
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")[:400]
        print(f"FAIL {name}: HTTP {error.code}: {detail}")
    except (urllib.error.URLError, KeyError, ValueError) as error:
        print(f"FAIL {name}: {type(error).__name__}: {error}")
    return False


def check_connectivity(judges: list[dict[str, Any]]) -> int:
    """Ping every route of every judge with one minimal request.

    Verifies three things at once and costs almost nothing: the endpoint path,
    the key, and whether the provider honours JSON mode. Run this before
    spending a full probe.
    """
    failures = 0
    for judge in judges:
        for route in judge["routes"]:
            if route.get("provider") == "local_codex_exec":
                healthy = check_local_connectivity(judge, route)
            else:
                healthy = check_http_connectivity(judge, route)
            if not healthy:
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
        judge_families = {judge["family"] for judge in judges}
    return judges, generator_family, judge_families


def resolve_pair_dir(value: Path) -> Path:
    pair_dir = value if value.is_absolute() else ROOT / value
    if not (pair_dir / "pair.json").exists():
        raise SystemExit(f"no pair.json under {pair_dir}")
    return pair_dir


def runnable_judges(judges: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Drop judges whose every route is blocked; keep the manifest's minimum.

    A blocked judge is a configuration state (billing, auth), not a probe
    failure: crashing mid-pair would discard paid samples for the judges that
    can run. Dropping below two families, however, is a real failure because
    the measurement would violate `min_judge_models`.
    """
    runnable = []
    for judge in judges:
        try:
            resolve_route(judge)
        except SystemExit as error:
            print(f"SKIP {judge['model']}: {error}")
            continue
        runnable.append(judge)
    families = {judge["family"] for judge in runnable}
    if len(families) < 2:
        raise SystemExit(
            f"fewer than 2 runnable judge families ({sorted(families)}); "
            "unblock a route before measuring"
        )
    return runnable


def saved_result_is_reusable(
    saved: dict[str, Any],
    judge: dict[str, Any],
    route: dict[str, Any],
    samples_per_artifact: int,
    expected_fingerprint: str,
    expected_judge_config_fingerprint: str | None = None,
) -> bool:
    summary = saved.get("summary", {})
    return (
        summary.get("judge_model") == judge["model"]
        and summary.get("route_provider") == route["provider"]
        and summary.get("samples_per_artifact") == samples_per_artifact
        and summary.get("input_fingerprint") == expected_fingerprint
        and (
            expected_judge_config_fingerprint is None
            or summary.get("judge_config_fingerprint")
            == expected_judge_config_fingerprint
        )
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
    api_key: str | None,
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
        "route_endpoint": route.get("endpoint"),
        "route_command": route.get("command"),
        "samples_per_artifact": len(forward),
        "temperature": (
            None if route["provider"] == "local_codex_exec" else temperature
        ),
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
    if route["provider"] == "local_codex_exec":
        result["sampling_control"] = "codex_cli_provider_default"
    result.update(consistency_metrics(forward, reverse, dimension_ids))
    result.update(
        specificity_metrics(baseline_scores, negative_scores, target, dimensions)
    )
    return result


def reusable_probe_summary(
    result_path: Path, judge: dict[str, Any], samples_per_artifact: int,
    expected_fingerprint: str, target: str, dimensions: list[dict[str, Any]], force: bool,
) -> dict[str, Any] | None:
    if not result_path.exists() or force:
        return None
    saved = load(result_path)
    provider = saved.get("summary", {}).get("route_provider")
    route = next((item for item in judge["routes"] if item["provider"] == provider), None)
    config = judge_config_fingerprint(judge) if provider == "local_codex_exec" else None
    if not route or not saved_result_is_reusable(
        saved, judge, route, samples_per_artifact, expected_fingerprint, config,
    ):
        return None
    summary = refresh_saved_summary(saved, target, dimensions)
    if provider == "local_codex_exec":
        summary["temperature"] = None
        summary["sampling_control"] = "codex_cli_provider_default"
    saved["summary"] = summary
    atomic_write(result_path, saved)
    print(f"RESUME {judge['model']} via {provider}: reusing complete saved samples")
    return summary


def load_probe_partial(
    partial_path: Path, expected_fingerprint: str, config_fingerprint: str | None,
) -> dict[str, Any]:
    partial = load(partial_path) if partial_path.exists() else {}
    if (
        partial.get("input_fingerprint") != expected_fingerprint
        or partial.get("judge_config_fingerprint") != config_fingerprint
    ):
        return {"input_fingerprint": expected_fingerprint, "judge_config_fingerprint": config_fingerprint, "forward": [], "reverse": []}
    return partial


def collect_order_samples(
    partial_path: Path, partial: dict[str, Any], key: str, count: int,
    route: dict[str, Any], judge: dict[str, Any], system: str, api_key: str | None,
    first: dict[str, Any], second: dict[str, Any], temperature: float,
    dimension_ids: list[str],
) -> list[dict[str, Any]]:
    samples = partial[key][:count]
    while len(samples) < count:
        samples.append(request_validated(
            route, judge["model"], system, api_key, first, second, temperature, dimension_ids,
        ))
        partial[key] = samples
        atomic_write(partial_path, partial)
        print(f"CHECKPOINT {judge['model']}: {key} {len(samples)}/{count}")
    return samples


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
    expected_fingerprint: str,
) -> dict[str, Any]:
    suffix = f".{result_suffix}" if result_suffix else ""
    result_path = pair_dir / f"judge-{judge['model']}{suffix}.result.json"
    partial_path = pair_dir / f"judge-{judge['model']}{suffix}.partial.json"
    if saved := reusable_probe_summary(
        result_path, judge, samples_per_artifact, expected_fingerprint,
        target, dimensions, force,
    ):
        return saved
    route = resolve_route(judge)
    config_fingerprint = (
        judge_config_fingerprint(judge)
        if route["provider"] == "local_codex_exec"
        else None
    )
    api_key = (
        os.environ[route["api_key_env"]]
        if route.get("api_key_env")
        else None
    )
    dimension_ids = [dimension["id"] for dimension in dimensions]
    partial = load_probe_partial(partial_path, expected_fingerprint, config_fingerprint)
    forward = collect_order_samples(
        partial_path, partial, "forward", samples_per_artifact, route, judge,
        system, api_key, baseline, negative, temperature, dimension_ids,
    )
    reverse = collect_order_samples(
        partial_path, partial, "reverse", samples_per_artifact, route, judge,
        system, api_key, negative, baseline, temperature, dimension_ids,
    )
    summary = summarize_judge(
        judge, route, forward, reverse, dimensions, target, defect_spans, temperature
    )
    summary["input_fingerprint"] = expected_fingerprint
    if config_fingerprint:
        summary["judge_config_fingerprint"] = config_fingerprint
    atomic_write(
        result_path, {"forward": forward, "reverse": reverse, "summary": summary}
    )
    partial_path.unlink(missing_ok=True)
    return summary


def build_probe_summary(
    config: dict[str, Any],
    pair: dict[str, Any],
    judge_families: set[str],
    per_judge: list[dict[str, Any]],
    thresholds: dict[str, float],
    expected_fingerprint: str,
) -> dict[str, Any]:
    generator_family = config["generator"]["family"]
    dimension_ids = sorted(per_judge[0]["baseline_scores"])
    agreement_items = [
        [
            *[judge["baseline_scores"][dimension] for dimension in dimension_ids],
            *[judge["negative_scores"][dimension] for dimension in dimension_ids],
        ]
        for judge in per_judge
    ]
    agreement = (
        krippendorff_alpha_interval(agreement_items)
        if len(agreement_items) >= 2
        else None
    )
    status_passes = all(
        judge["sensitivity"]
        and judge["order_consistent"]
        and judge["specificity_cross_pillar"]
        >= thresholds["min_specificity_cross_pillar"]
        and judge["self_consistency"] >= thresholds["min_self_consistency"]
        for judge in per_judge
    )
    return {
        "schema": "stage0-probe-result/v2",
        "pair_id": pair["pair_id"],
        "generator_model": config["generator"]["model"],
        "generator_family": generator_family,
        "generator_judge_disjoint": generator_family not in judge_families,
        "judge_families": sorted(judge_families),
        "judges": per_judge,
        "input_fingerprint": expected_fingerprint,
        "status_thresholds": thresholds,
        "inter_model_agreement": {
            "method": "krippendorff_alpha_interval",
            "value": agreement,
            "items": len(dimension_ids) * 2,
            "raters": len(per_judge),
        },
        "independence_caveat": config["independence_caveat"],
        "all_judges_detected": all(judge["sensitivity"] for judge in per_judge),
        "min_specificity_all": min(
            judge["specificity_all"] for judge in per_judge
        ),
        "min_specificity_cross_pillar": min(
            judge["specificity_cross_pillar"] for judge in per_judge
        ),
        "min_specificity": min(judge["specificity_all"] for judge in per_judge),
        "status": "measurable_gap" if status_passes else "probe_failed",
    }


def main() -> int:
    args = parse_args()
    config = load_probe_config()
    if args.check_connectivity:
        return 1 if check_connectivity(config["judges"]) else 0
    judges, _, judge_families = select_judges(config, args.only_judge)
    judges = runnable_judges(judges)
    judge_families = {judge["family"] for judge in judges}
    sampling = config["sampling"]
    samples_per_artifact = args.samples or sampling["samples_per_artifact"]
    dimensions = load_rubric()
    pair_dir = resolve_pair_dir(args.pair_dir)
    baseline = load(pair_dir / "baseline.story-package.json")
    negative = load(pair_dir / "negative.story-package.json")
    pair = load(pair_dir / "pair.json")
    manifest = load(MANIFEST)
    metric_config = manifest["evaluator_metrics"]
    thresholds = {
        "min_specificity_cross_pillar": metric_config[
            "perturbation_specificity"
        ]["target"],
        "min_self_consistency": metric_config["self_consistency"]["target"],
    }
    expected_fingerprint = input_fingerprint(pair_dir)
    defect = pair["seeded_defects"][0]
    per_judge = [
        run_judge(
            judge, pair_dir, baseline, negative, dimensions,
            build_system(dimensions), defect["target_dimension"],
            set(defect["spans"]), samples_per_artifact, sampling["temperature"],
            args.result_suffix, args.force,
            expected_fingerprint,
        )
        for judge in judges
    ]
    summary = build_probe_summary(
        config,
        pair,
        judge_families,
        per_judge,
        thresholds,
        expected_fingerprint,
    )
    suffix = f".{args.result_suffix}" if args.result_suffix else ""
    atomic_write(pair_dir / f"probe-summary{suffix}.json", summary)
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
