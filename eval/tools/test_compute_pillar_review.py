"""Tests for the pillar grouping review (REQ-321)."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from compute_pillar_review import (
    build_review,
    collect_observations,
    rank_with_ties,
    spearman,
)
from run_stage0_probe import load_rubric

DIMENSIONS = load_rubric()
DIMENSION_IDS = [dimension["id"] for dimension in DIMENSIONS]
PILLAR_OF = {dimension["id"]: dimension["pillar"] for dimension in DIMENSIONS}


def write_result(
    run_dir: Path, judge: str, case_id: str, medians: dict, samples: int = 3
) -> None:
    path = run_dir / f"judge-{judge}.{case_id}.result.json"
    path.write_text(
        json.dumps(
            {
                "schema": "pointwise-score-result/v1",
                "case_id": case_id,
                "artifact_id": f"artifact-{case_id}",
                "samples": [{}] * samples,
                "summary": {
                    "judge_model": judge,
                    "route_provider": "fake_http",
                    "samples_per_artifact": samples,
                    "input_fingerprint": "sha256:x",
                    "median_scores": medians,
                },
            },
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )


def synthetic_case_medians(
    case_index: int, correlated_pair: tuple[str, str] | None = None
) -> dict:
    """Deterministic scores; one optional perfectly-correlated pair.

    The default generator keeps every cross-pillar |Spearman| below ~0.51
    across ten cases (calibrated), so a `no_change` conclusion is meaningful.
    """
    medians = {}
    for position, dimension in enumerate(DIMENSION_IDS):
        if correlated_pair and dimension in correlated_pair:
            medians[dimension] = 1 + (case_index % 5)
            continue
        medians[dimension] = ((case_index * (position + 2)) % 13) % 5 + 1
    return medians


def synthetic_run(
    run_dir: Path,
    judges: list[str],
    cases: int,
    correlated_pair: tuple[str, str] | None = None,
) -> None:
    for judge in judges:
        for case_index in range(cases):
            write_result(
                run_dir,
                judge,
                f"case_{case_index:03d}",
                synthetic_case_medians(case_index, correlated_pair),
            )


class SpearmanTests(unittest.TestCase):
    def test_perfect_monotone_relationship_is_one(self) -> None:
        self.assertAlmostEqual(spearman([1, 2, 3, 4, 5], [10, 20, 30, 44, 50]), 1.0)

    def test_inverse_relationship_is_minus_one(self) -> None:
        self.assertAlmostEqual(spearman([1, 2, 3, 4, 5], [5, 4, 3, 2, 1]), -1.0)

    def test_ties_average_ranks(self) -> None:
        ranks = rank_with_ties([3, 1, 3, 2])
        self.assertEqual(ranks, [3.5, 1.0, 3.5, 2.0])

    def test_constant_vector_is_nan(self) -> None:
        self.assertTrue(spearman([2, 2, 2], [1, 2, 3]) != spearman([2, 2, 2], [1, 2, 3]))


class ReviewTests(unittest.TestCase):
    def test_correlated_cross_pillar_pair_recommends_merge(self) -> None:
        pair = ("human_credibility", "short_drama_pacing")
        self.assertNotEqual(PILLAR_OF[pair[0]], PILLAR_OF[pair[1]])
        with TemporaryDirectory() as directory:
            run_dir = Path(directory) / "run-a"
            run_dir.mkdir()
            synthetic_run(run_dir, ["judge-x", "judge-y"], 10, correlated_pair=pair)
            review = build_review([run_dir])
            self.assertEqual(review["conclusion"], "merge_recommended")
            recommendation = next(
                item
                for item in review["merge_recommendations"]
                if {item["dimension_a"], item["dimension_b"]} == set(pair)
            )
            self.assertEqual(len(recommendation["judges_supporting"]), 3)
            self.assertGreaterEqual(abs(recommendation["rho"]), 0.8)

    def test_uncorrelated_run_concludes_no_change(self) -> None:
        with TemporaryDirectory() as directory:
            run_dir = Path(directory) / "run-b"
            run_dir.mkdir()
            synthetic_run(run_dir, ["judge-x"], 10)
            review = build_review([run_dir])
            self.assertEqual(review["conclusion"], "no_change")

    def test_missing_dimension_is_rejected(self) -> None:
        with TemporaryDirectory() as directory:
            run_dir = Path(directory) / "run-c"
            run_dir.mkdir()
            medians = synthetic_case_medians(0)
            del medians[DIMENSION_IDS[0]]
            write_result(run_dir, "judge-x", "case_000", medians)
            with self.assertRaises(SystemExit):
                build_review([run_dir])

    def test_incomplete_sample_set_is_rejected(self) -> None:
        with TemporaryDirectory() as directory:
            run_dir = Path(directory) / "run-d"
            run_dir.mkdir()
            write_result(
                run_dir, "judge-x", "case_000",
                synthetic_case_medians(0), samples=3,
            )
            saved = json.loads(
                (run_dir / "judge-judge-x.case_000.result.json").read_text("utf-8")
            )
            saved["samples"] = saved["samples"][:2]
            (run_dir / "judge-judge-x.case_000.result.json").write_text(
                json.dumps(saved), encoding="utf-8"
            )
            with self.assertRaises(SystemExit):
                build_review([run_dir])

    def test_later_run_supersedes_same_judge_case(self) -> None:
        with TemporaryDirectory() as directory:
            first = Path(directory) / "run-e"
            second = Path(directory) / "run-f"
            first.mkdir()
            second.mkdir()
            write_result(first, "judge-x", "case_000", synthetic_case_medians(0))
            write_result(second, "judge-x", "case_000", synthetic_case_medians(1))
            observations = collect_observations([first, second])
            self.assertEqual(len(observations), 1)
            self.assertEqual(
                observations[("judge-x", "case_000")],
                synthetic_case_medians(1),
            )


if __name__ == "__main__":
    unittest.main()
