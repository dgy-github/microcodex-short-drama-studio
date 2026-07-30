"""Evaluate the human-gated release rules without allowing LLM-only promotion."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
from collections import defaultdict
from pathlib import Path
from typing import Any

import jsonschema

from probe_metrics import krippendorff_alpha_nominal

ROOT = Path(__file__).resolve().parents[2]


def canonical_fingerprint(value: dict[str, Any]) -> str:
    encoded = json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def majority_candidate(pair: dict[str, Any]) -> bool:
    votes = [review["preference"] for review in pair["reviews"]]
    return votes.count("candidate") > votes.count("incumbent")


def stratified_bootstrap_lcb(
    pair_reviews: list[dict[str, Any]], samples: int = 4000
) -> float | None:
    if len(pair_reviews) < 2:
        return None
    strata: dict[str, list[float]] = defaultdict(list)
    for pair in pair_reviews:
        strata[pair["genre"]].append(1.0 if majority_candidate(pair) else 0.0)
    randomizer = random.Random(0)
    estimates = []
    for _ in range(samples):
        drawn = [
            randomizer.choice(values)
            for values in strata.values()
            for _ in range(len(values))
        ]
        estimates.append(sum(drawn) / len(drawn))
    estimates.sort()
    return estimates[int(0.025 * (len(estimates) - 1))]


def panel_evidence(
    evidence: dict[str, Any],
) -> tuple[bool, float | None, int, int, list[str]]:
    pairs = evidence["pair_reviews"]
    reasons: list[str] = []
    required_adjudications = 0
    complete_adjudications = 0
    if not pairs:
        return False, None, 0, 0, ["professional_pair_reviews_missing"]
    expected_raters: list[str] | None = None
    preference_by_rater: dict[str, list[str]] = defaultdict(list)
    for pair in pairs:
        reviews = pair["reviews"]
        rater_ids = [review["rater_id"] for review in reviews]
        credentials = [review["credential"] for review in reviews]
        if (
            len(reviews) != 3
            or len(set(rater_ids)) != 3
            or not all(review["blind"] for review in reviews)
            or sum(
                credential in {"working_screenwriter", "story_editor"}
                for credential in credentials
            )
            < 2
            or "target_viewer" not in credentials
        ):
            reasons.append(f"invalid_blind_panel:{pair['pair_id']}")
        ordered = sorted(rater_ids)
        if expected_raters is None:
            expected_raters = ordered
        elif ordered != expected_raters:
            reasons.append("professional_rater_set_not_shared")
        for review in reviews:
            preference_by_rater[review["rater_id"]].append(review["preference"])
        if not pair["admission_passed"] or not pair["policy_passed"]:
            reasons.append(f"admission_or_policy_failed:{pair['pair_id']}")
        requires = any(
            max(review["dimensions"].get(dimension, 0) for review in reviews)
            - min(review["dimensions"].get(dimension, 0) for review in reviews)
            >= 2
            for dimension in evidence["critical_dimensions"]
        )
        if requires:
            required_adjudications += 1
            if pair["adjudication"] and pair["adjudication"].get("resolved") is True:
                complete_adjudications += 1
            else:
                reasons.append(f"adjudication_missing:{pair['pair_id']}")
    agreement = None
    if expected_raters and not any(
        reason == "professional_rater_set_not_shared" for reason in reasons
    ):
        agreement = krippendorff_alpha_nominal(
            [preference_by_rater[rater] for rater in expected_raters]
        )
        if not math.isfinite(agreement):
            reasons.append("professional_agreement_not_finite")
    human_ready = (
        not reasons
        and evidence["holdout_seal"]["case_count"]
        == len({pair["case_id"] for pair in pairs})
        and bool(evidence["screenwriter_signoffs"])
    )
    if evidence["holdout_seal"]["case_count"] != len(
        {pair["case_id"] for pair in pairs}
    ):
        reasons.append("holdout_case_count_mismatch")
        human_ready = False
    if not evidence["screenwriter_signoffs"]:
        reasons.append("screenwriter_signoff_missing")
        human_ready = False
    return (
        human_ready,
        agreement,
        required_adjudications,
        complete_adjudications,
        reasons,
    )


def evaluate_release(evidence: dict[str, Any]) -> dict[str, Any]:
    schema = json.loads(
        (ROOT / "schemas/professional-release-evidence-v1.json").read_text("utf-8")
    )
    schema["properties"]["holdout_seal"] = json.loads(
        (ROOT / "schemas/holdout-seal-v1.json").read_text("utf-8")
    )
    jsonschema.Draft202012Validator(schema).validate(evidence)
    (
        human_ready,
        agreement,
        adjudications_required,
        adjudications_complete,
        reasons,
    ) = panel_evidence(evidence)
    pairs = evidence["pair_reviews"]
    accuracy = (
        sum(majority_candidate(pair) for pair in pairs) / len(pairs)
        if pairs
        else None
    )
    preference_lcb = stratified_bootstrap_lcb(pairs)
    if human_ready:
        gates = [
            (
                all(value >= -0.10 for value in evidence["critical_dimension_deltas"].values()),
                "critical_dimension_regression",
            ),
            (
                all(value >= -0.15 for value in evidence["genre_slice_deltas"].values()),
                "genre_slice_regression",
            ),
            (
                preference_lcb is not None and preference_lcb > 0.50,
                "holdout_preference_lcb_not_above_half",
            ),
            (
                evidence["critical_failure_delta"] <= 0,
                "critical_failures_increased",
            ),
            (
                evidence["overlap_blocking_violations"] == 0,
                "originality_overlap_blocked",
            ),
            (
                (
                    evidence["mean_cost_within_budget"]
                    and evidence["p95_latency_within_budget"]
                )
                or evidence["quality_gain_cost_approved"],
                "cost_or_latency_budget_failed",
            ),
            (evidence["stochastic_samples"] >= 3, "insufficient_stochastic_samples"),
            (
                agreement is not None and agreement >= 0.67,
                "professional_agreement_below_threshold",
            ),
            (
                adjudications_required == adjudications_complete,
                "adjudication_incomplete",
            ),
        ]
        reasons.extend(reason for passed, reason in gates if not passed)
        decision = "promote" if not reasons else "reject"
    else:
        decision = "non_promotable"
    result = {
        "schema": "promotion-decision/v1",
        "evaluation_id": evidence["evaluation_id"],
        "candidate_kind": evidence["candidate_kind"],
        "candidate_id": evidence["candidate_id"],
        "incumbent_id": evidence["incumbent_id"],
        "decision": decision,
        "human_gate_satisfied": human_ready,
        "metrics": {
            "pair_accuracy": accuracy,
            "pair_preference_lcb": preference_lcb,
            "professional_agreement": agreement,
            "adjudications_required": adjudications_required,
            "adjudications_complete": adjudications_complete,
        },
        "reasons": reasons or ["all_release_gates_passed"],
        "evidence_fingerprint": canonical_fingerprint(evidence),
    }
    output_schema = json.loads(
        (ROOT / "schemas/promotion-decision-v1.json").read_text("utf-8")
    )
    jsonschema.Draft202012Validator(output_schema).validate(result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError("decision output already exists")
    result = evaluate_release(json.loads(args.evidence.read_text("utf-8")))
    args.output.write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
