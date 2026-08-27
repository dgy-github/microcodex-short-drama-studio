"""Agreement between blind internal human scores and judge scores (REQ-322).

Joins the desktop evaluation root (`assignments/` + `human-scores/`, written by
`submit_blind_review`) with pointwise judge results (`eval/scores/<run_id>/`)
on `artifact_id`, then computes `spot_check_agreement`:

- nominal Krippendorff alpha over binned dimension scores with every judge and
  every human reviewer as one rater (reuses `probe_metrics`, no competing
  implementation);
- per-dimension mean differences (judge - human) so systematic judge drift on
  one dimension is visible, not just the headline alpha.

Internal reviewers are not ground truth (`internal_spot_check.is_ground_truth`
is false in the manifest); this metric detects judge failure modes.

Artifacts scored on only one side are reported as unmatched instead of being
silently dropped.

Usage:
    python eval/tools/compute_spot_check_agreement.py \
        --runs eval/scores/run-a [--evaluation-root C:\\...] [--report out.json]
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
from pathlib import Path
from typing import Any

from probe_metrics import krippendorff_alpha_nominal
from run_stage0_probe import MANIFEST, atomic_write, load

import yaml

ROOT = Path(__file__).parents[2]
SCORES = ROOT / "eval" / "scores"
RUBRIC = ROOT / "eval" / "rubrics" / "judge-v1.yaml"

# Scores are 1-5 integers; medians can land on .5. Binning keeps the nominal
# alpha interpretable: .5 medians round toward the neighbouring integer half
# the time is NOT acceptable (nondeterministic), so round-half-even via round().
BINS = {1: "1", 2: "2", 3: "3", 4: "4", 5: "5"}


def default_evaluation_root() -> Path:
    override = os.environ.get("MICROCODEX_EVALUATION_ROOT")
    if override:
        return Path(override)
    local_app_data = os.environ.get("LOCALAPPDATA")
    if not local_app_data:
        raise SystemExit(
            "neither MICROCODEX_EVALUATION_ROOT nor LOCALAPPDATA is set; "
            "pass --evaluation-root explicitly"
        )
    return (
        Path(local_app_data)
        / "MicrocodeX"
        / "ShortDramaStudio"
        / "evaluation"
    )


def load_judge_observations(
    run_dirs: list[Path],
) -> dict[str, dict[str, Any]]:
    """artifact_id -> {"judge_model", "case_id", "median_scores"}; later runs win."""
    observations: dict[str, dict[str, Any]] = {}
    for run_dir in run_dirs:
        for result_path in sorted(run_dir.glob("judge-*.result.json")):
            saved = load(result_path)
            summary = saved["summary"]
            if len(saved["samples"]) != summary["samples_per_artifact"]:
                raise SystemExit(
                    f"{result_path.name}: incomplete sample set; rerun score_artifacts"
                )
            observations[saved["artifact_id"]] = {
                "judge_model": summary["judge_model"],
                "case_id": saved["case_id"],
                "median_scores": summary["median_scores"],
            }
    return observations


def load_human_scores(
    evaluation_root: Path,
) -> dict[str, dict[str, Any]]:
    """artifact_id -> list of per-rater dimension score maps."""
    scores_dir = evaluation_root / "human-scores"
    by_artifact: dict[str, dict[str, Any]] = {}
    if not scores_dir.is_dir():
        raise SystemExit(f"no human-scores directory under {evaluation_root}")
    records = sorted(scores_dir.glob("*.json"))
    if not records:
        raise SystemExit(f"no human score records under {scores_dir}")
    for record_path in records:
        record = load(record_path)
        if record.get("rater", {}).get("rater_type") != "internal_spot_check":
            continue
        artifact_id = record["artifact_id"]
        dimensions = {
            entry["dimension_id"]: entry["score"]
            for entry in record["dimensions"]
        }
        by_artifact.setdefault(
            artifact_id,
            {"case_id": record["case_id"], "raters": []},
        )["raters"].append(
            {
                "rater_id": record["rater"]["rater_id"],
                "scores": dimensions,
            }
        )
    if not by_artifact:
        raise SystemExit("no internal_spot_check records found in human-scores")
    return by_artifact


def bin_score(value: float) -> str:
    binned = round(value)
    if binned not in BINS:
        raise ValueError(f"unbinable score: {value}")
    return BINS[binned]


def dimension_ids_from_rubric() -> list[str]:
    document = yaml.safe_load(RUBRIC.read_text(encoding="utf-8"))
    return [dimension["id"] for dimension in document["dimensions"]]


def build_agreement(
    evaluation_root: Path, run_dirs: list[Path]
) -> dict[str, Any]:
    dimension_ids = dimension_ids_from_rubric()
    judges = load_judge_observations(run_dirs)
    humans = load_human_scores(evaluation_root)

    joined = sorted(set(judges) & set(humans))
    human_only = sorted(set(humans) - set(judges))
    judge_only = sorted(set(judges) - set(humans))

    # Nominal alpha needs every rater to score the same complete item set,
    # which only holds inside one artifact (all raters scored its ten
    # dimensions). Alpha is therefore computed per artifact as a complete
    # block and aggregated; cross-artifact pooling would have holes.
    raters: list[dict[str, Any]] = []
    per_dimension: dict[str, dict[str, float]] = {}
    per_artifact: list[dict[str, Any]] = []
    alphas: list[float] = []
    for artifact_id in joined:
        judge = judges[artifact_id]
        block: list[list[str]] = []
        block.append(
            [
                bin_score(judge["median_scores"][dimension])
                for dimension in dimension_ids
            ]
        )
        raters.append(
            {
                "role": "judge",
                "name": judge["judge_model"],
                "artifact_id": artifact_id,
            }
        )
        for human in humans[artifact_id]["raters"]:
            block.append(
                [
                    bin_score(human["scores"][dimension])
                    for dimension in dimension_ids
                ]
            )
            raters.append(
                {
                    "role": "internal_spot_check",
                    "name": human["rater_id"],
                    "artifact_id": artifact_id,
                }
            )
            for dimension in dimension_ids:
                difference = (
                    judge["median_scores"][dimension] - human["scores"][dimension]
                )
                per_dimension.setdefault(dimension, {})[
                    f"{judge['judge_model']}-{human['rater_id']}"
                ] = difference
        alpha = krippendorff_alpha_nominal(block)
        alphas.append(alpha)
        per_artifact.append(
            {
                "artifact_id": artifact_id,
                "case_id": judge["case_id"],
                "raters_in_block": len(block),
                "alpha": alpha,
            }
        )

    mean_differences = {
        dimension: round(statistics.mean(pairs.values()), 3)
        for dimension, pairs in per_dimension.items()
    }
    meaningful = [alpha for alpha in alphas if alpha == alpha]
    return {
        "schema": "spot-check-agreement/v1",
        "evaluation_root": str(evaluation_root),
        "source_runs": [str(path) for path in run_dirs],
        "joined_artifacts": len(joined),
        "raters": raters,
        "spot_check_agreement": {
            "method": "krippendorff_alpha_nominal_per_artifact",
            "mean_alpha": (
                round(statistics.mean(meaningful), 4) if meaningful else None
            ),
            "min_alpha": round(min(meaningful), 4) if meaningful else None,
            "items_per_artifact": len(dimension_ids),
            "per_artifact": per_artifact,
        },
        "per_dimension_mean_difference_judge_minus_human": mean_differences,
        "unmatched": {
            "human_only": [
                {
                    "artifact_id": artifact_id,
                    "case_id": humans[artifact_id]["case_id"],
                }
                for artifact_id in human_only
            ],
            "judge_only": [
                {
                    "artifact_id": artifact_id,
                    "case_id": judges[artifact_id]["case_id"],
                }
                for artifact_id in judge_only
            ],
        },
        "manifest_note": load(MANIFEST)["internal_spot_check"],
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--runs", action="append", required=True,
        help="eval/scores run directory with judge-*.result.json; repeatable",
    )
    parser.add_argument("--evaluation-root", type=Path, default=None)
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
    evaluation_root = args.evaluation_root or default_evaluation_root()
    if not evaluation_root.is_dir():
        raise SystemExit(f"no such evaluation root: {evaluation_root}")
    agreement = build_agreement(evaluation_root, run_dirs)
    if not agreement["joined_artifacts"]:
        print(
            "WARNING: no artifact was scored by both a judge and a human; "
            f"human-only {len(agreement['unmatched']['human_only'])}, "
            f"judge-only {len(agreement['unmatched']['judge_only'])}"
        )
    report_path = args.report or (
        SCORES / f"spot-check-agreement-{agreement['joined_artifacts']}joined.json"
    )
    atomic_write(report_path, agreement)
    print(
        json.dumps(
            {
                "joined_artifacts": agreement["joined_artifacts"],
                "spot_check_agreement": agreement["spot_check_agreement"],
                "per_dimension_mean_difference_judge_minus_human": (
                    agreement["per_dimension_mean_difference_judge_minus_human"]
                ),
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    print(f"report: {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
