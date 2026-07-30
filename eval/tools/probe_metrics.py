"""Pure metric calculations for stage-0 adversarial probe results."""

from __future__ import annotations

import statistics
from typing import Any


def median_scores(
    samples: list[dict[str, Any]], label: str, dimension_ids: list[str]
) -> dict[str, float]:
    return {
        dimension: statistics.median(
            sample[label][dimension]["score"] for sample in samples
        )
        for dimension in dimension_ids
    }


def spans_for(samples: list[dict[str, Any]], label: str) -> set[str]:
    cited: set[str] = set()
    for sample in samples:
        for block in sample[label].values():
            if isinstance(block, dict):
                cited.update(block.get("spans", []))
    return cited


def self_consistency(
    samples: list[dict[str, Any]], label: str, dimension_ids: list[str]
) -> float:
    """Return the share of dimensions where every sample agreed."""
    if len(samples) < 2:
        return float("nan")
    agreed = sum(
        1
        for dimension in dimension_ids
        if len({sample[label][dimension]["score"] for sample in samples}) == 1
    )
    return agreed / len(dimension_ids)


def consistency_metrics(
    forward: list[dict[str, Any]],
    reverse: list[dict[str, Any]],
    dimension_ids: list[str],
) -> dict[str, Any]:
    by_order = {
        "forward_A": self_consistency(forward, "A", dimension_ids),
        "forward_B": self_consistency(forward, "B", dimension_ids),
        "reverse_A": self_consistency(reverse, "A", dimension_ids),
        "reverse_B": self_consistency(reverse, "B", dimension_ids),
    }
    return {
        "self_consistency": statistics.mean(by_order.values()),
        "self_consistency_by_order": by_order,
        "self_consistency_forward": by_order["forward_A"],
    }


def specificity_metrics(
    baseline_scores: dict[str, float],
    negative_scores: dict[str, float],
    target: str,
    dimensions: list[dict[str, Any]],
) -> dict[str, Any]:
    """Report isolation across all dimensions and across other pillars only."""
    dimension_pillars = {
        dimension["id"]: dimension["pillar"] for dimension in dimensions
    }
    if target not in dimension_pillars:
        raise ValueError(f"target dimension has no pillar: {target}")
    missing = sorted(
        set(dimension_pillars) - set(baseline_scores)
        | set(dimension_pillars) - set(negative_scores)
    )
    if missing:
        raise ValueError(f"specificity scores missing dimensions: {missing}")

    target_pillar = dimension_pillars[target]
    all_others = [dimension for dimension in dimension_pillars if dimension != target]
    cross_pillar = [
        dimension
        for dimension in all_others
        if dimension_pillars[dimension] != target_pillar
    ]
    if not cross_pillar:
        raise ValueError(f"target pillar has no cross-pillar dimensions: {target_pillar}")
    dropped = {
        dimension
        for dimension in dimension_pillars
        if negative_scores[dimension] < baseline_scores[dimension]
    }
    collateral_all = sorted(dropped - {target})
    collateral_cross = sorted(set(cross_pillar) & dropped)
    specificity_all = sum(dimension not in dropped for dimension in all_others) / len(
        all_others
    )
    specificity_cross = sum(
        dimension not in dropped for dimension in cross_pillar
    ) / len(cross_pillar)
    return {
        "target_pillar": target_pillar,
        "specificity_all": specificity_all,
        "specificity_cross_pillar": specificity_cross,
        "collateral_dimensions_all": collateral_all,
        "collateral_dimensions_cross_pillar": collateral_cross,
        "specificity": specificity_all,
        "collateral_dimensions": collateral_all,
    }


def median_pair_scores(
    forward: list[dict[str, Any]],
    reverse: list[dict[str, Any]],
    dimension_ids: list[str],
) -> tuple[dict[str, float], dict[str, float]]:
    baseline = {
        dimension: statistics.median(
            [sample["A"][dimension]["score"] for sample in forward]
            + [sample["B"][dimension]["score"] for sample in reverse]
        )
        for dimension in dimension_ids
    }
    negative = {
        dimension: statistics.median(
            [sample["B"][dimension]["score"] for sample in forward]
            + [sample["A"][dimension]["score"] for sample in reverse]
        )
        for dimension in dimension_ids
    }
    return baseline, negative


def krippendorff_alpha_interval(raters: list[list[float]]) -> float:
    """Krippendorff's alpha using squared interval distance.

    Rows are raters and columns are shared items. Every item must have one
    finite score from every rater; stage-0 validation guarantees no missing
    dimension scores before this function is called.
    """
    if len(raters) < 2:
        raise ValueError("inter-model agreement needs at least two raters")
    item_count = len(raters[0])
    if item_count == 0 or any(len(rater) != item_count for rater in raters):
        raise ValueError("raters must score the same non-empty item set")
    values = [score for rater in raters for score in rater]
    if any(not isinstance(score, (int, float)) for score in values):
        raise ValueError("agreement scores must be numeric")

    observed_pairs = [
        (raters[left][item] - raters[right][item]) ** 2
        for item in range(item_count)
        for left in range(len(raters))
        for right in range(left + 1, len(raters))
    ]
    observed = statistics.mean(observed_pairs)
    expected_pairs = [
        (values[left] - values[right]) ** 2
        for left in range(len(values))
        for right in range(left + 1, len(values))
    ]
    expected = statistics.mean(expected_pairs)
    if expected == 0:
        return 1.0 if observed == 0 else float("nan")
    return 1.0 - observed / expected


def krippendorff_alpha_nominal(raters: list[list[str]]) -> float:
    """Krippendorff's alpha with nominal disagreement for shared assignments."""
    if len(raters) < 2:
        raise ValueError("professional agreement needs at least two raters")
    item_count = len(raters[0])
    if item_count == 0 or any(len(rater) != item_count for rater in raters):
        raise ValueError("raters must score the same non-empty item set")
    values = [value for rater in raters for value in rater]
    if any(not isinstance(value, str) or not value for value in values):
        raise ValueError("nominal agreement values must be non-empty strings")
    observed_pairs = [
        raters[left][item] != raters[right][item]
        for item in range(item_count)
        for left in range(len(raters))
        for right in range(left + 1, len(raters))
    ]
    observed = statistics.mean(observed_pairs)
    expected_pairs = [
        values[left] != values[right]
        for left in range(len(values))
        for right in range(left + 1, len(values))
    ]
    expected = statistics.mean(expected_pairs)
    if expected == 0:
        return 1.0 if observed == 0 else float("nan")
    return 1.0 - observed / expected
