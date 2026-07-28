"""Compute the manifest's evaluator_metrics from judge samples already on disk.

The probe emits 28 fields; the manifest names 7 metrics; only 3 line up. This
closes the gap for the two that need no new API calls, using the manifest's own
definitions rather than the probe's ad-hoc field names.

`seeded_defect_detection` — share of seeded pairs where the degraded artifact
scores below its own unmodified baseline. It measures within-pair direction,
not quality ranking: it says nothing about whether the baseline was any good.

`defect_localisation` — share of degraded artifacts whose cited spans overlap
the seeded defect spans. Reported beside the detection rate on purpose: a judge
that gets the direction right but cannot say where the defect is has responded
to a diffuse signal (length, tone, fluency) rather than the planted defect, and
its dimension-level scores must not be used to attribute failure.

Both are defined over *pairs*, so with a single stage-0 pair each can only be
0.0 or 1.0. The per-observation breakdown is reported alongside so the headline
figure is not mistaken for an estimate.

Usage:
    python eval/tools/compute_evaluator_metrics.py
"""

from __future__ import annotations

import argparse
import json
import statistics
from pathlib import Path
from typing import Any

ROOT = Path(__file__).parents[2]
PAIR_DIR = ROOT / "eval" / "adversarial" / "stage0" / "motive-explicit"
MANIFEST = ROOT / "eval" / "manifests" / "eval-v0.1.0.json"
OUT = PAIR_DIR / "evaluator-metrics.json"


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def result_files() -> list[Path]:
    return sorted(
        p
        for p in PAIR_DIR.glob("judge-*.result.json")
        if ".invalid-span." not in p.name
    )


def observations(
    samples: list[dict[str, Any]],
    baseline_label: str,
    negative_label: str,
    dimension_ids: list[str],
    defect_spans: set[str],
) -> list[dict[str, Any]]:
    """One observation per sample: did the negative land below its baseline,
    and did the citation land on the seeded spans."""
    rows = []
    for sample in samples:
        baseline = statistics.mean(
            sample[baseline_label][d]["score"] for d in dimension_ids
        )
        negative = statistics.mean(
            sample[negative_label][d]["score"] for d in dimension_ids
        )
        cited: set[str] = set()
        for block in sample[negative_label].values():
            if isinstance(block, dict):
                cited.update(block.get("spans", []))
        rows.append(
            {
                "baseline_mean": round(baseline, 4),
                "negative_mean": round(negative, 4),
                "negative_lower": negative < baseline,
                "cited_spans": len(cited),
                "on_defect": len(cited & defect_spans),
                "localised": bool(cited & defect_spans),
            }
        )
    return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    manifest = load(MANIFEST)
    metrics = manifest["evaluator_metrics"]
    dimension_ids = [
        d for pillar in manifest["pillars"].values() for d in pillar["dimensions"]
    ]
    pair = load(PAIR_DIR / "pair.json")
    negative = load(PAIR_DIR / "negative.story-package.json")
    defect_spans = set(pair["seeded_defects"][0]["spans"])

    per_judge = []
    for path in result_files():
        data = load(path)
        summary = data.get("summary", {})
        rows = observations(
            data["forward"], "A", "B", dimension_ids, defect_spans
        ) + observations(data["reverse"], "B", "A", dimension_ids, defect_spans)
        per_judge.append(
            {
                "source": path.name,
                "judge_model": summary.get("judge_model"),
                "route_provider": summary.get("route_provider"),
                "observations": len(rows),
                "negative_lower_rate": sum(r["negative_lower"] for r in rows)
                / len(rows),
                "localised_rate": sum(r["localised"] for r in rows) / len(rows),
                "pair_detected": all(r["negative_lower"] for r in rows),
                "pair_localised": any(r["localised"] for r in rows),
                "detail": rows,
            }
        )

    # A stability rerun is the same judge measured twice, not a third judge.
    # Counting it as one would weight GLM 2:1 in the headline figure.
    primary = []
    reruns = []
    seen: set[str] = set()
    for entry in per_judge:
        if "rerun" in entry["source"]:
            reruns.append(entry)
        elif entry["judge_model"] in seen:
            reruns.append(entry)
        else:
            seen.add(entry["judge_model"])
            primary.append(entry)

    pairs_total = 1
    detected = int(all(j["pair_detected"] for j in primary))
    localised = int(all(j["pair_localised"] for j in primary))

    # Localisation is only informative if some part of the artifact was left
    # unseeded. When every dialogue node carries a seeded defect, any citation
    # of any line is a hit by construction.
    unseeded_dialogue = sorted(
        {
            f"story-package/{scene['node_id']}/{line['node_id']}"
            for scene in negative["scenes"]
            for line in scene["lines"]
            if line.get("kind") == "dialogue"
        }
        - defect_spans
    )

    report = {
        "schema": "evaluator-metrics/v1",
        "manifest": manifest["eval_version"],
        "pair_ids": [pair["pair_id"]],
        "pairs_total": pairs_total,
        "resolution_warning": (
            "Both metrics are defined over pairs. With one pair the only "
            "attainable values are 0.0 and 1.0; this is not an estimate and "
            "has no confidence interval. Read the per-judge observation rates "
            "for texture."
        ),
        "seeded_defect_detection": {
            "value": detected / pairs_total,
            "target": metrics["seeded_defect_detection"]["target"],
            "meets_target": detected / pairs_total
            >= metrics["seeded_defect_detection"]["target"],
        },
        "defect_localisation": {
            "value": localised / pairs_total,
            "target": metrics["defect_localisation"]["target"],
            "meets_target": localised / pairs_total
            >= metrics["defect_localisation"]["target"],
            "constructively_guaranteed": not unseeded_dialogue,
            "caveat": (
                "Every dialogue node in this negative is seeded, so any cited "
                "line is a hit by construction and this figure reflects no "
                "localisation skill. It becomes meaningful only once the "
                "degradation is narrowed and unseeded dialogue exists."
            )
            if not unseeded_dialogue
            else None,
        },
        "judges_counted": [j["judge_model"] for j in primary],
        "reruns_excluded_from_headline": [j["source"] for j in reruns],
        "per_judge": per_judge,
        "not_computable_here": {
            "inter_model_agreement": "needs an agreement statistic over a shared rater set; scores exist but the estimator is not implemented",
            "spot_check_agreement": "needs the internal human spot check, which has never been run",
        },
    }

    if args.check:
        if not OUT.exists():
            print("MISSING evaluator-metrics.json")
            return 1
        print("OK evaluator-metrics.json present")
        return 0

    OUT.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps({k: v for k, v in report.items() if k != "per_judge"},
                     ensure_ascii=False, indent=2))
    for judge in per_judge:
        print(
            f"  {judge['judge_model']:16} n={judge['observations']} "
            f"negative_lower={judge['negative_lower_rate']:.2f} "
            f"localised={judge['localised_rate']:.2f}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
