"""Dimension correlation matrix and pillar grouping review (REQ-321).

Consumes pointwise score results (`eval/scores/<run_id>/judge-*.result.json`)
and produces the `pillar_grouping_review` evidence the manifest has been
waiting on: per-judge and pooled Spearman correlation matrices over the ten
dimension medians, plus merge recommendations for dimension pairs that
correlate at or above the manifest's `merge_threshold` while living in
different pillars.

A high same-pillar correlation is expected (those dimensions are averaged
together), so only cross-pillar pairs produce recommendations. The tool may
well conclude `no_change`; that is a legitimate review outcome, not a failure.

Correlations are rank correlations (Spearman) because the scores are ordinal
1-5 medians. Aggregation arithmetic itself stays owned by `crates/story-eval`.

Usage:
    python eval/tools/compute_pillar_review.py --runs eval/scores/run-a [--runs ...] \
        [--report eval/scores/pillar-review-v1.json]
"""

from __future__ import annotations

import argparse
import json
import statistics
from pathlib import Path
from typing import Any

from run_stage0_probe import MANIFEST, RUBRIC, atomic_write, load

import yaml

ROOT = Path(__file__).parents[2]
SCORES = ROOT / "eval" / "scores"
MIN_CASES_PER_JUDGE = 10


def load_dimensions() -> list[dict[str, Any]]:
    document = yaml.safe_load(RUBRIC.read_text(encoding="utf-8"))
    return document["dimensions"]


def collect_observations(run_dirs: list[Path]) -> dict[tuple[str, str], dict[str, float]]:
    """Map (judge_model, case_id) -> dimension medians from result files.

    Later runs win: scoring the same judge-case again in a newer run
    supersedes the older observation instead of double-counting it.
    """
    observations: dict[tuple[str, str], dict[str, float]] = {}
    for run_dir in run_dirs:
        results = sorted(run_dir.glob("judge-*.result.json"))
        if not results:
            raise SystemExit(f"{run_dir}: no judge-*.result.json files")
        for result_path in results:
            saved = load(result_path)
            summary = saved["summary"]
            medians = summary["median_scores"]
            if len(saved["samples"]) != summary["samples_per_artifact"]:
                raise SystemExit(
                    f"{result_path.name}: incomplete sample set "
                    f"({len(saved['samples'])}/{summary['samples_per_artifact']})"
                )
            observations[(summary["judge_model"], saved["case_id"])] = dict(medians)
    return observations


def validate_completeness(
    observations: dict[tuple[str, str], dict[str, float]],
    dimension_ids: list[str],
) -> None:
    for (judge_model, case_id), medians in sorted(observations.items()):
        missing = [d for d in dimension_ids if d not in medians]
        if missing:
            raise SystemExit(
                f"{judge_model} {case_id}: missing dimension medians: {missing}"
            )


def rank_with_ties(values: list[float]) -> list[float]:
    ordered = sorted(range(len(values)), key=lambda index: values[index])
    ranks = [0.0] * len(values)
    start = 0
    while start < len(ordered):
        end = start
        while (
            end + 1 < len(ordered)
            and values[ordered[end + 1]] == values[ordered[start]]
        ):
            end += 1
        average_rank = statistics.mean(range(start + 1, end + 2))
        for position in range(start, end + 1):
            ranks[ordered[position]] = average_rank
        start = end + 1
    return ranks


def spearman(left: list[float], right: list[float]) -> float:
    if len(left) != len(right) or not left:
        raise ValueError("correlation vectors must be non-empty and equal length")
    left_ranks = rank_with_ties(left)
    right_ranks = rank_with_ties(right)
    mean_left = statistics.mean(left_ranks)
    mean_right = statistics.mean(right_ranks)
    numerator = sum(
        (a - mean_left) * (b - mean_right) for a, b in zip(left_ranks, right_ranks)
    )
    denominator = (
        sum((a - mean_left) ** 2 for a in left_ranks)
        * sum((b - mean_right) ** 2 for b in right_ranks)
    ) ** 0.5
    if denominator == 0:
        return float("nan")
    return numerator / denominator


def correlation_matrix(
    observations: dict[tuple[str, str], dict[str, float]],
    dimension_ids: list[str],
) -> dict[str, dict[str, float]]:
    matrix: dict[str, dict[str, float]] = {}
    for outer in dimension_ids:
        matrix[outer] = {}
        for inner in dimension_ids:
            if outer == inner:
                matrix[outer][inner] = 1.0
                continue
            left = [medians[outer] for medians in observations.values()]
            right = [medians[inner] for medians in observations.values()]
            matrix[outer][inner] = round(spearman(left, right), 4)
    return matrix


def merge_recommendations(
    matrices: dict[str, dict[str, dict[str, float]]],
    dimensions: list[dict[str, Any]],
    threshold: float,
) -> list[dict[str, Any]]:
    pillar_of = {dimension["id"]: dimension["pillar"] for dimension in dimensions}
    dimension_ids = [dimension["id"] for dimension in dimensions]
    seen: set[frozenset[str]] = set()
    recommendations = []
    for judge_model, matrix in matrices.items():
        for left in dimension_ids:
            for right in dimension_ids:
                if left >= right:
                    continue
                rho = matrix[left][right]
                if abs(rho) < threshold or pillar_of[left] == pillar_of[right]:
                    continue
                key = frozenset((left, right))
                if key in seen:
                    for existing in recommendations:
                        if frozenset((existing["dimension_a"], existing["dimension_b"])) == key:
                            existing["judges_supporting"].append(judge_model)
                    continue
                seen.add(key)
                recommendations.append(
                    {
                        "dimension_a": left,
                        "pillar_a": pillar_of[left],
                        "dimension_b": right,
                        "pillar_b": pillar_of[right],
                        "rho": rho,
                        "judges_supporting": [judge_model],
                    }
                )
    return recommendations


def run_identifier(path: Path) -> str:
    try:
        return path.relative_to(ROOT).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def build_review(run_dirs: list[Path]) -> dict[str, Any]:
    manifest = load(MANIFEST)
    review_config = manifest["pillar_grouping_review"]
    threshold = review_config["merge_threshold"]
    dimensions = load_dimensions()
    dimension_ids = [dimension["id"] for dimension in dimensions]
    observations = collect_observations(run_dirs)
    validate_completeness(observations, dimension_ids)

    by_judge: dict[str, dict[str, dict[str, float]]] = {}
    cases_per_judge: dict[str, int] = {}
    for (judge_model, _) in observations:
        cases_per_judge[judge_model] = cases_per_judge.get(judge_model, 0) + 1
    for judge_model in cases_per_judge:
        judge_observations = {
            key: medians
            for key, medians in observations.items()
            if key[0] == judge_model
        }
        by_judge[judge_model] = correlation_matrix(judge_observations, dimension_ids)
    pooled = correlation_matrix(observations, dimension_ids)
    matrices = {**by_judge, "pooled": pooled}

    recommendations = merge_recommendations(matrices, dimensions, threshold)
    underfed = sorted(
        judge for judge, count in cases_per_judge.items()
        if count < MIN_CASES_PER_JUDGE
    )
    return {
        "schema": "pillar-review/v1",
        "source_runs": [run_identifier(path) for path in run_dirs],
        "merge_threshold": threshold,
        "cases_per_judge": cases_per_judge,
        "scored_records": len(observations),
        "matrices": matrices,
        "merge_recommendations": recommendations,
        "conclusion": "merge_recommended" if recommendations else "no_change",
        "caveats": (
            [
                f"judges with fewer than {MIN_CASES_PER_JUDGE} cases: {underfed}; "
                "their per-judge correlations are noisy"
            ]
            if underfed
            else []
        )
        + ["pooled matrix mixes rater scales; prefer per-judge views for decisions"],
        "note": review_config.get("note"),
    }


def markdown(review: dict[str, Any]) -> str:
    lines = [
        "# Pillar grouping review",
        "",
        f"- conclusion: **{review['conclusion']}**",
        f"- scored records: {review['scored_records']} "
        f"({review['cases_per_judge']})",
        f"- merge threshold: {review['merge_threshold']}",
    ]
    if review["merge_recommendations"]:
        lines.append("- cross-pillar merges at or above threshold:")
        for item in review["merge_recommendations"]:
            lines.append(
                f"  - {item['dimension_a']} ({item['pillar_a']}) ~ "
                f"{item['dimension_b']} ({item['pillar_b']}): "
                f"rho={item['rho']} via {', '.join(item['judges_supporting'])}"
            )
    for caveat in review["caveats"]:
        lines.append(f"- caveat: {caveat}")
    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--runs", action="append", required=True,
        help="eval/scores run directory; repeatable, later runs supersede",
    )
    parser.add_argument("--report", type=Path, default=None)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    run_dirs = [
        path if path.is_absolute() else ROOT / path for path in args.runs
    ]
    for run_dir in run_dirs:
        if not run_dir.is_dir():
            raise SystemExit(f"no such run directory: {run_dir}")
    review = build_review(run_dirs)
    report_path = args.report or (
        SCORES / f"pillar-review-{review['scored_records']}records.json"
    )
    atomic_write(report_path, review)
    print(markdown(review))
    print(f"report: {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
