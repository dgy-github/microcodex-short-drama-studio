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

Both are defined over *pairs*. Pass one `--pair-dir` per measured pair (or none
for the historical stage-0 default); the headline figures are computed over the
whole set, with per-judge observation rates alongside so the granularity
(1/pairs) is never mistaken for a confidence interval. A judge counts as
detecting a pair only when **every** primary judge did — one unresponsive judge
vetoes, which is the manifest's all-judges definition.

Usage:
    python eval/tools/compute_evaluator_metrics.py \
        --pair-dir eval/adversarial/stage0/motive-explicit-narrow \
        --pair-dir eval/adversarial/stage1/hook-fake ...
"""

from __future__ import annotations

import argparse
import json
import statistics
from pathlib import Path
from typing import Any

from probe_metrics import krippendorff_alpha_interval

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PAIR = ROOT / "eval" / "adversarial" / "stage0" / "motive-explicit-narrow"
MANIFEST = ROOT / "eval" / "manifests" / "eval-v0.1.0.json"


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def result_files(pair_dir: Path) -> list[Path]:
    return sorted(
        p
        for p in pair_dir.glob("judge-*.result.json")
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


def pair_identifier(pair_dir: Path) -> str:
    try:
        return pair_dir.relative_to(ROOT).as_posix()
    except ValueError:
        return pair_dir.resolve().as_posix()


def measure_pair(pair_dir: Path, dimension_ids: list[str]) -> dict[str, Any]:
    """All primary judges' observations for one pair, reruns excluded."""
    pair = load(pair_dir / "pair.json")
    negative = load(pair_dir / "negative.story-package.json")
    defect_spans = set(pair["seeded_defects"][0]["spans"])

    per_judge = []
    for path in result_files(pair_dir):
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
    # Counting it as one would weight that judge 2:1 in the headline figure.
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

    unseeded_dialogue = sorted(
        {
            f"story-package/{scene['node_id']}/{line['node_id']}"
            for scene in negative["scenes"]
            for line in scene["lines"]
            if line.get("kind") == "dialogue"
        }
        - defect_spans
    )
    fingerprints = sorted(
        {
            load(pair_dir / entry["source"]).get("summary", {}).get("input_fingerprint")
            for entry in primary
        }
        - {None}
    )
    return {
        "pair_dir": pair_identifier(pair_dir),
        "result_paths": {
            entry["source"]: pair_dir / entry["source"] for entry in per_judge
        },
        "pair_id": pair["pair_id"],
        "primary": primary,
        "reruns_excluded_from_headline": [j["source"] for j in reruns],
        "detected": bool(primary) and all(j["pair_detected"] for j in primary),
        "localised": bool(primary) and any(j["pair_localised"] for j in primary),
        "constructively_guaranteed_localisation": not unseeded_dialogue,
        "input_fingerprints": fingerprints,
        "all_inputs_fingerprinted": len(fingerprints) <= 1
        and all(
            load(pair_dir / entry["source"]).get("summary", {}).get("input_fingerprint")
            == (fingerprints[0] if fingerprints else None)
            for entry in primary
        ),
    }


def pooled_agreement(
    pairs: list[dict[str, Any]], dimension_ids: list[str]
) -> dict[str, Any] | None:
    """Interval alpha over every judge that is primary in EVERY pair.

    Rows must be complete over a shared item set, so judges missing from any
    pair are excluded from the pooled figure and reported as partial.
    """
    if not pairs:
        return None
    complete = [entry["judge_model"] for entry in pairs[0]["primary"]]
    for measured in pairs[1:]:
        models = {entry["judge_model"] for entry in measured["primary"]}
        complete = [model for model in complete if model in models]
    if len(complete) < 2:
        return None
    rows = []
    for model in complete:
        row: list[float] = []
        for measured in pairs:
            entry = next(
                e for e in measured["primary"] if e["judge_model"] == model
            )
            summary = load(measured["result_paths"][entry["source"]])["summary"]
            row.extend(summary["baseline_scores"][d] for d in dimension_ids)
            row.extend(summary["negative_scores"][d] for d in dimension_ids)
        rows.append(row)
    return {
        "method": "krippendorff_alpha_interval",
        "value": krippendorff_alpha_interval(rows),
        "items": len(pairs) * len(dimension_ids) * 2,
        "raters": len(complete),
        "judges": complete,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument(
        "--pair-dir",
        action="append",
        type=Path,
        default=None,
        help="pair directory (repeatable); defaults to the stage-0 narrow pair",
    )
    parser.add_argument("--out", type=Path, default=None)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    pair_dirs = [
        path if path.is_absolute() else ROOT / path
        for path in (args.pair_dir or [DEFAULT_PAIR])
    ]
    for pair_dir in pair_dirs:
        if not (pair_dir / "pair.json").exists():
            raise SystemExit(f"no pair.json under {pair_dir}")

    manifest = load(MANIFEST)
    metrics = manifest["evaluator_metrics"]
    dimension_ids = [
        d for pillar in manifest["pillars"].values() for d in pillar["dimensions"]
    ]
    pairs = [measure_pair(pair_dir, dimension_ids) for pair_dir in pair_dirs]

    pairs_total = len(pairs)
    detected = sum(1 for p in pairs if p["detected"])
    localised = sum(1 for p in pairs if p["localised"])
    guaranteed = [
        p["pair_id"] for p in pairs if p["constructively_guaranteed_localisation"]
    ]
    out_dir = (
        ROOT / pairs[0]["pair_dir"]
        if pairs_total == 1
        else ROOT / "eval" / "adversarial"
    )
    out = args.out or (out_dir / "evaluator-metrics.json")

    report = {
        "schema": "evaluator-metrics/v1",
        "manifest": manifest["eval_version"],
        "pair_ids": [p["pair_id"] for p in pairs],
        "pairs_total": pairs_total,
        "resolution_note": (
            f"Both headline figures are defined over pairs; with {pairs_total} "
            f"pair(s) the granularity is 1/{pairs_total}. "
            + (
                "A single pair can only attain 0.0 or 1.0 — not an estimate."
                if pairs_total == 1
                else "Read the per-judge observation rates for texture."
            )
        ),
        "seeded_defect_detection": {
            "value": detected / pairs_total,
            "target": metrics["seeded_defect_detection"]["target"],
            "meets_target": detected / pairs_total
            >= metrics["seeded_defect_detection"]["target"],
            "detected_pairs": detected,
        },
        "defect_localisation": {
            "value": localised / pairs_total,
            "target": metrics["defect_localisation"]["target"],
            "meets_target": localised / pairs_total
            >= metrics["defect_localisation"]["target"],
            "constructively_guaranteed_pairs": guaranteed,
        },
        "judges_counted": sorted(
            {j["judge_model"] for p in pairs for j in p["primary"]}
        ),
        "inter_model_agreement": pooled_agreement(pairs, dimension_ids),
        "per_pair": [
            {k: v for k, v in p.items() if k != "primary"} | {
                "per_judge": [
                    {k: v for k, v in j.items() if k != "detail"}
                    for j in p["primary"]
                ]
            }
            for p in pairs
        ],
        "not_computable_here": {
            "spot_check_agreement": "needs the internal human spot check, which has never been run",
        },
    }
    report = json.loads(json.dumps(report, ensure_ascii=False, default=str))

    if args.check:
        if not out.exists():
            print("MISSING evaluator-metrics.json")
            return 1
        print("OK evaluator-metrics.json present")
        return 0

    out.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {k: v for k, v in report.items() if k != "per_pair"},
            ensure_ascii=False,
            indent=2,
        )
    )
    for p in pairs:
        for judge in p["primary"]:
            print(
                f"  {p['pair_id'][:44]:44} {judge['judge_model']:16} "
                f"n={judge['observations']} "
                f"negative_lower={judge['negative_lower_rate']:.2f} "
                f"localised={judge['localised_rate']:.2f}"
            )
    print(f"report: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
