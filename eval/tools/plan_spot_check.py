"""Produce the concrete internal spot-check sampling plan (P3a/P1).

The manifest's `internal_spot_check` block sets the rates (default 20% of
cases, 100% of adversarial pairs) but nothing turns them into a list. The
desktop EvaluationCenter takes case selections by hand, so the reviewer needs
the exact case ids before opening the app. This tool computes them
deterministically: per genre, take `ceil(rate * n)` cases by sorted case_id,
plus every adversarial pair's baseline artifact, and writes a plan the
reviewer follows one id at a time.

Deterministic on purpose: two runs of this tool produce the same plan, so the
sample actually reviewed can be audited against the plan that was filed.

Usage:
    python eval/tools/plan_spot_check.py
    python eval/tools/plan_spot_check.py --rater reviewer_01
"""

from __future__ import annotations

import argparse
import json
import math
from collections import defaultdict
from datetime import date
from pathlib import Path
from typing import Any

from run_stage0_probe import MANIFEST, ROOT, load

CASES = ROOT / "eval" / "cases"
ADVERSARIAL = ROOT / "eval" / "adversarial"
PLAN = ROOT / "eval" / "scores" / "spot-check-plan.json"
SPLITS = ("dev", "train", "validation", "holdout", "challenge")


def load_cases() -> list[dict[str, Any]]:
    records = []
    for split in SPLITS:
        path = CASES / split / "cases.jsonl"
        if path.exists():
            records.extend(
                json.loads(line)
                for line in path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            )
    return records


def sample_cases(
    records: list[dict[str, Any]], rate: float
) -> list[dict[str, Any]]:
    by_genre: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in records:
        by_genre[record["genre"]].append(record)
    sampled = []
    for genre in sorted(by_genre):
        members = sorted(by_genre[genre], key=lambda r: r["case_id"])
        take = max(1, math.ceil(rate * len(members)))
        sampled.extend(members[:take])
    return sorted(sampled, key=lambda r: r["case_id"])


def adversarial_baselines(rate: float) -> list[dict[str, str]]:
    pairs = []
    for pair_path in sorted(ADVERSARIAL.rglob("pair.json")):
        pair = load(pair_path)
        if rate < 1.0 and pair.get("status") not in {"candidate", "accepted_hard"}:
            continue
        pairs.append(
            {
                "pair_id": pair["pair_id"],
                "case_id": pair["case_id"],
                "problem_code": pair["seeded_defects"][0]["problem_code"],
            }
        )
    return pairs


def build_plan(rater: str) -> dict[str, Any]:
    manifest = load(MANIFEST)
    config = manifest["internal_spot_check"]
    records = load_cases()
    sampled = sample_cases(records, config["case_sample_rate"])
    adversarial = adversarial_baselines(config.get("adversarial_sample_rate", 1.0))
    return {
        "schema": "spot-check-plan/v1",
        "rater_id": rater,
        "planned_on": date.today().isoformat(),
        "manifest_rates": {
            "case_sample_rate": config["case_sample_rate"],
            "adversarial_sample_rate": config.get("adversarial_sample_rate", 1.0),
        },
        "cases": [record["case_id"] for record in sampled],
        "case_count": len(sampled),
        "adversarial_pairs": adversarial,
        "instructions": (
            "Desktop EvaluationCenter -> offline-v0.1.0 dataset -> manual blind "
            "review -> select exactly these case ids, then score every artifact "
            "on all ten dimensions. Afterwards run "
            "compute_spot_check_agreement.py --runs eval/scores/<latest>."
        ),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rater", default="reviewer_01")
    parser.add_argument("--plan", type=Path, default=PLAN)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    plan = build_plan(args.rater)
    args.plan.parent.mkdir(parents=True, exist_ok=True)
    args.plan.write_text(
        json.dumps(plan, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        f"plan for {args.rater}: {plan['case_count']} cases + "
        f"{len(plan['adversarial_pairs'])} adversarial pairs"
    )
    print("  cases:", ", ".join(plan["cases"]))
    for pair in plan["adversarial_pairs"]:
        print(f"  pair {pair['pair_id']} ({pair['problem_code']}, {pair['case_id']})")
    print(f"plan: {args.plan}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
