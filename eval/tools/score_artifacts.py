"""Pointwise judge scoring of tracked artifacts (REQ-320).

Produces `eval-score-record/v1` rows for archived baseline packages so the
"30 scored cases" evidence P3a is blocked on (10 archived packages x 3 judges)
can be collected under the same judge set, input fingerprinting and resume
guarantees as the stage-0 pairwise probe. Pointwise prompts share no context
with pairwise probes (`judging.pointwise_and_pairwise_share_context` is false
in the manifest): one artifact goes in, ten dimension scores come out.

Pillar aggregation and verdicts stay owned by `crates/story-eval`; this tool
records per-sample scores and per-dimension medians only, so no competing
aggregation implementation appears here.

Admission is inherited from the archive step: `generate_baselines.py` refuses
to emit a package that fails the admission gates, so every archived baseline
already passed. The records therefore carry `admission.passed = true` rather
than re-deriving gates.

Usage:
    python eval/tools/score_artifacts.py --check-connectivity
    python eval/tools/score_artifacts.py --run-id baseline-20260827
    python eval/tools/score_artifacts.py --run-id baseline-20260827 \\
        --only-judge gpt-5.4 --cases comedy_002 --dry-run
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import statistics
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import yaml

from run_stage0_probe import (
    JUDGES,
    MANIFEST,
    RUBRIC,
    atomic_write,
    check_connectivity,
    judge_config_fingerprint,
    load,
    load_probe_config,
    normalize_span_list,
    request,
    resolve_route,
    select_judges,
    valid_line_spans,
)

ROOT = Path(__file__).parents[2]
BASELINES = ROOT / "eval" / "baselines"
SCORES = ROOT / "eval" / "scores"

POINTWISE_SYSTEM_HEADER = """你是短剧评测员。对给定的一份产物，按下列每个维度分别给 1-5 整数分。
每个维度都必须给出理由，并引用该产物中真实存在的 span_ref。
不要因为文本更长、措辞更清楚或情绪更强就提高分数。
理由不超过 200 字。"""

POINTWISE_SYSTEM_FOOTER = """只输出 JSON，结构为：
{"<dimension_id>":{"score":1,"reason":"","spans":[]}, ...}
必须包含全部维度。"""


def load_rubric_document() -> dict[str, Any]:
    return yaml.safe_load(RUBRIC.read_text(encoding="utf-8"))


def build_pointwise_system(dimensions: list[dict[str, Any]]) -> str:
    lines = [POINTWISE_SYSTEM_HEADER, ""]
    for dimension in dimensions:
        anchors = dimension["anchors"]
        lines.append(
            f"- {dimension['id']}（{dimension['name']}）：{dimension['ask']}\n"
            f"  1分：{anchors[1]}\n  3分：{anchors[3]}\n  5分：{anchors[5]}"
        )
    lines.extend(["", POINTWISE_SYSTEM_FOOTER])
    return "\n".join(lines)


def build_pointwise_user_prompt(
    case_id: str,
    artifact: dict[str, Any],
    validation_error: str | None,
) -> str:
    return json.dumps(
        {
            "case_id": case_id,
            "artifact": artifact,
            "valid_span_refs": sorted(valid_line_spans(artifact)),
            "instruction": "spans 只能从产物的 valid_span_refs 中逐字选择",
            "retry_instruction": (
                f"上次输出未通过校验：{validation_error}。请修正后完整重答。"
                if validation_error
                else None
            ),
        },
        ensure_ascii=False,
        separators=(",", ":"),
    )


def validate_pointwise(
    value: dict[str, Any],
    artifact: dict[str, Any],
    dimension_ids: list[str],
) -> None:
    allowed = valid_line_spans(artifact)
    if not isinstance(value, dict):
        raise ValueError("judgment must be an object of dimensions")
    missing = [d for d in dimension_ids if d not in value]
    if missing:
        raise ValueError(f"missing dimensions: {missing}")
    for dimension in dimension_ids:
        entry = value[dimension]
        score = entry.get("score")
        if not isinstance(score, int) or not 1 <= score <= 5:
            raise ValueError(f"{dimension}.score must be 1-5")
        if not entry.get("reason") or not entry.get("spans"):
            raise ValueError(f"{dimension} requires a reason and spans")
        invalid = set(entry["spans"]) - allowed
        if invalid:
            raise ValueError(f"{dimension} invalid spans: {sorted(invalid)}")


def request_validated_pointwise(
    route: dict[str, Any],
    model: str,
    system: str,
    api_key: str | None,
    case_id: str,
    artifact: dict[str, Any],
    temperature: float,
    dimension_ids: list[str],
    retry_limit: int,
) -> dict[str, Any]:
    attempts = retry_limit + 1
    validation_error: str | None = None
    for attempt in range(1, attempts + 1):
        try:
            sample = request(
                route,
                model,
                system,
                api_key,
                None,
                None,
                temperature,
                user_prompt=(
                    f"{system}\n\n"
                    + build_pointwise_user_prompt(case_id, artifact, validation_error)
                ),
            )
        except (json.JSONDecodeError, KeyError, TypeError) as error:
            validation_error = f"{type(error).__name__}: {error}"
            if attempt == attempts:
                raise
            print(
                f"RETRY {model} via {route['provider']}: "
                f"unparseable judge output ({validation_error})"
            )
            continue
        allowed = valid_line_spans(artifact)
        for entry in sample.values():
            if isinstance(entry, dict) and isinstance(entry.get("spans"), list):
                entry["spans"] = normalize_span_list(entry["spans"], artifact, allowed)
        try:
            validate_pointwise(sample, artifact, dimension_ids)
            return sample
        except ValueError as error:
            validation_error = str(error)
            if attempt == attempts:
                raise
            print(
                f"RETRY {model} via {route['provider']}: "
                f"invalid judge output ({validation_error})"
            )
    raise AssertionError("unreachable")


def input_fingerprint_pointwise(package_path: Path) -> str:
    digest = hashlib.sha256()
    paths = [package_path, RUBRIC, JUDGES, MANIFEST]
    for path in paths:
        try:
            identifier = path.relative_to(ROOT).as_posix()
        except ValueError:
            # Outside-repo packages (tests, one-off artifacts) still get a
            # stable machine-absolute identifier.
            identifier = path.resolve().as_posix()
        relative = identifier.encode("utf-8")
        content = path.read_bytes()
        digest.update(len(relative).to_bytes(4, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return f"sha256:{digest.hexdigest()}"


def load_baseline_targets(
    only_cases: set[str] | None,
) -> list[dict[str, Any]]:
    """Collect archived baseline packages from every run under eval/baselines."""
    targets: list[dict[str, Any]] = []
    if not BASELINES.exists():
        raise SystemExit(f"no baseline archive under {BASELINES}")
    for index_path in sorted(BASELINES.glob("*/index.json")):
        index = load(index_path)
        for case in index["cases"]:
            if only_cases and case["case_id"] not in only_cases:
                continue
            targets.append(
                {
                    "case_id": case["case_id"],
                    "artifact_id": case["artifact_id"],
                    "package": index_path.parent / case["package"],
                    "content_hash": case.get("content_hash"),
                }
            )
    if not targets:
        raise SystemExit("no archived baseline packages matched the case filter")
    return targets


def saved_pointwise_is_reusable(
    saved: dict[str, Any],
    judge: dict[str, Any],
    route_provider: str,
    samples_per_artifact: int,
    expected_fingerprint: str,
    expected_config_fingerprint: str | None,
) -> bool:
    summary = saved.get("summary", {})
    return (
        summary.get("judge_model") == judge["model"]
        and summary.get("route_provider") == route_provider
        and summary.get("samples_per_artifact") == samples_per_artifact
        and summary.get("input_fingerprint") == expected_fingerprint
        and (
            expected_config_fingerprint is None
            or summary.get("judge_config_fingerprint")
            == expected_config_fingerprint
        )
        and len(saved.get("samples", [])) == samples_per_artifact
    )


def pointwise_score_record(
    run_id: str,
    case_id: str,
    artifact_id: str,
    content_hash: str | None,
    char_count: int,
    judge_model: str,
    sample: dict[str, Any],
    sample_index: int,
    rubric_version: str,
) -> dict[str, Any]:
    dimensions = [
        {
            "dimension_id": dimension_id,
            "score": entry["score"],
            "reason": entry["reason"],
            "span_refs": entry["spans"],
            "valid": True,
            "invalid_reason": None,
        }
        for dimension_id, entry in sorted(sample.items())
        if dimension_id != "_provider_usage"
    ]
    return {
        "schema": "eval-score-record/v1",
        "record_id": f"{run_id}:{judge_model}:{case_id}:{sample_index}",
        "run_id": run_id,
        "case_id": case_id,
        "artifact_id": artifact_id,
        "artifact_content_hash": content_hash,
        "artifact_char_count": char_count,
        "rubric_version": rubric_version,
        "rater": {
            "rater_id": judge_model,
            "rater_type": "llm_judge",
            "model_id": judge_model,
            "seed": None,
            "sample_index": sample_index,
            "credential": None,
            "blind_assignment_id": None,
            "rater_blinded": False,
        },
        "admission": {"passed": True, "failed_gates": []},
        "dimensions": dimensions,
        "occurred_at": datetime.now(timezone.utc).isoformat(),
    }


def score_case(
    judge: dict[str, Any],
    case: dict[str, Any],
    artifact: dict[str, Any],
    system: str,
    temperature: float,
    samples_per_artifact: int,
    retry_limit: int,
    dimension_ids: list[str],
    run_dir: Path,
    force: bool,
    expected_fingerprint: str,
) -> dict[str, Any] | None:
    result_path = run_dir / f"judge-{judge['model']}.{case['case_id']}.result.json"
    partial_path = run_dir / f"judge-{judge['model']}.{case['case_id']}.partial.json"
    if result_path.exists() and not force:
        saved = load(result_path)
        saved_provider = saved.get("summary", {}).get("route_provider")
        saved_route = next(
            (r for r in judge["routes"] if r["provider"] == saved_provider), None
        )
        saved_config_fingerprint = (
            judge_config_fingerprint(judge)
            if saved_provider == "local_codex_exec"
            else None
        )
        if saved_route and saved_pointwise_is_reusable(
            saved,
            judge,
            saved_provider,
            samples_per_artifact,
            expected_fingerprint,
            saved_config_fingerprint,
        ):
            print(
                f"RESUME {judge['model']} {case['case_id']} via {saved_provider}: "
                "reusing complete saved samples"
            )
            return saved["summary"]
        print(
            f"STALE {judge['model']} {case['case_id']}: saved fingerprint or sample "
            "count does not match; rescoring"
        )
    route = resolve_route(judge)
    config_fingerprint = (
        judge_config_fingerprint(judge) if route["provider"] == "local_codex_exec" else None
    )
    api_key = os.environ[route["api_key_env"]] if route.get("api_key_env") else None
    partial = (
        existing
        if partial_path.exists()
        and (existing := load(partial_path)).get("input_fingerprint") == expected_fingerprint
        and existing.get("judge_config_fingerprint") == config_fingerprint
        else {
            "input_fingerprint": expected_fingerprint,
            "judge_config_fingerprint": config_fingerprint,
            "samples": [],
        }
    )
    samples = partial["samples"][:samples_per_artifact]
    while len(samples) < samples_per_artifact:
        samples.append(
            request_validated_pointwise(
                route,
                judge["model"],
                system,
                api_key,
                case["case_id"],
                artifact,
                temperature,
                dimension_ids,
                retry_limit,
            )
        )
        partial["samples"] = samples
        atomic_write(partial_path, partial)
        print(
            f"CHECKPOINT {judge['model']} {case['case_id']}: "
            f"sample {len(samples)}/{samples_per_artifact}"
        )
    medians = {
        dimension: statistics.median(
            sample[dimension]["score"]
            for sample in samples
            if dimension != "_provider_usage"
        )
        for dimension in dimension_ids
    }
    summary = {
        "judge_model": judge["model"],
        "judge_family": judge["family"],
        "route_provider": route["provider"],
        "samples_per_artifact": samples_per_artifact,
        "temperature": (
            None if route["provider"] == "local_codex_exec" else temperature
        ),
        "input_fingerprint": expected_fingerprint,
        "median_scores": medians,
    }
    if config_fingerprint:
        summary["judge_config_fingerprint"] = config_fingerprint
        summary["sampling_control"] = "codex_cli_provider_default"
    atomic_write(
        result_path,
        {
            "schema": "pointwise-score-result/v1",
            "case_id": case["case_id"],
            "artifact_id": case["artifact_id"],
            "samples": samples,
            "summary": summary,
        },
    )
    partial_path.unlink(missing_ok=True)
    return summary


def rewrite_scores_jsonl(
    run_dir: Path,
    run_id: str,
    rubric_version: str,
    targets_by_case: dict[str, dict[str, Any]],
) -> int:
    """Rebuild scores.jsonl from the result files so it always mirrors them."""
    records = []
    for result_path in sorted(run_dir.glob("judge-*.result.json")):
        saved = load(result_path)
        judge_model = saved["summary"]["judge_model"]
        target = targets_by_case.get(saved["case_id"])
        if target is None:
            raise SystemExit(
                f"{result_path.name}: case {saved['case_id']} has no archived "
                "baseline; scores.jsonl cannot be rebuilt"
            )
        char_count = len(json.dumps(load(target["package"]), ensure_ascii=False))
        for sample_index, sample in enumerate(saved["samples"]):
            records.append(
                pointwise_score_record(
                    run_id,
                    saved["case_id"],
                    saved["artifact_id"],
                    target["content_hash"],
                    char_count,
                    judge_model,
                    sample,
                    sample_index,
                    rubric_version,
                )
            )
    lines = [
        json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        for record in records
    ]
    (run_dir / "scores.jsonl").write_text(
        "\n".join(lines) + ("\n" if lines else ""), encoding="utf-8"
    )
    return len(records)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-connectivity", action="store_true")
    parser.add_argument("--run-id", default=None, help="directory under eval/scores/")
    parser.add_argument("--only-judge", default=None)
    parser.add_argument("--cases", default=None, help="comma-separated case ids")
    parser.add_argument("--samples", type=int, default=None)
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    config = load_probe_config()
    if args.check_connectivity:
        return 1 if check_connectivity(config["judges"]) else 0
    if not args.run_id:
        raise SystemExit("--run-id is required unless --check-connectivity is used")
    judges, _, _ = select_judges(config, args.only_judge)
    sampling = config["sampling"]
    samples_per_artifact = args.samples or sampling["samples_per_artifact"]
    manifest = load(MANIFEST)
    retry_limit = manifest["judging"]["invalid_score_retry_limit"]
    rubric_document = load_rubric_document()
    dimensions = rubric_document["dimensions"]
    dimension_ids = [dimension["id"] for dimension in dimensions]
    system = build_pointwise_system(dimensions)
    only_cases = set(args.cases.split(",")) if args.cases else None
    targets = load_baseline_targets(only_cases)
    run_dir = SCORES / args.run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    if args.dry_run:
        calls = len(targets) * len(judges) * samples_per_artifact
        print(f"DRY-RUN {len(targets)} cases x {len(judges)} judges "
              f"x {samples_per_artifact} samples = {calls} judge calls")
        for case in targets:
            print(f"  {case['case_id']}: {input_fingerprint_pointwise(case['package'])}")
        return 0
    temperature = sampling["temperature"]
    targets_by_case = {target["case_id"]: target for target in targets}
    summaries = []
    for case in targets:
        artifact = load(case["package"])
        fingerprint = input_fingerprint_pointwise(case["package"])
        for judge in judges:
            summary = score_case(
                judge,
                case,
                artifact,
                system,
                temperature,
                samples_per_artifact,
                retry_limit,
                dimension_ids,
                run_dir,
                args.force,
                fingerprint,
            )
            if summary:
                summaries.append(summary)
    record_count = rewrite_scores_jsonl(
        run_dir, args.run_id, rubric_document["version"], targets_by_case
    )
    print(
        f"scored-run {args.run_id}: {len(summaries)} judge-case summaries, "
        f"{record_count} score records"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
