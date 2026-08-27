"""Tests for spot-check agreement between humans and judges (REQ-322)."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from compute_spot_check_agreement import (
    bin_score,
    build_agreement,
    load_human_scores,
    load_judge_observations,
)
from run_stage0_probe import load_rubric

DIMENSIONS = load_rubric()
DIMENSION_IDS = [dimension["id"] for dimension in DIMENSIONS]


def judge_result(
    run_dir: Path, judge: str, case_id: str, artifact_id: str, medians: dict
) -> None:
    (run_dir / f"judge-{judge}.{case_id}.result.json").write_text(
        json.dumps(
            {
                "schema": "pointwise-score-result/v1",
                "case_id": case_id,
                "artifact_id": artifact_id,
                "samples": [{}, {}, {}],
                "summary": {
                    "judge_model": judge,
                    "route_provider": "fake_http",
                    "samples_per_artifact": 3,
                    "input_fingerprint": "sha256:x",
                    "median_scores": medians,
                },
            },
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )


def human_record(
    evaluation_root: Path, assignment_id: str, case_id: str, artifact_id: str,
    rater_id: str, scores: dict,
) -> None:
    record = {
        "schema": "eval-score-record/v1",
        "record_id": f"human_{assignment_id}",
        "run_id": assignment_id,
        "case_id": case_id,
        "artifact_id": artifact_id,
        "rubric_version": "judge-v1",
        "rater": {
            "rater_id": rater_id,
            "rater_type": "internal_spot_check",
            "model_id": None,
            "rater_blinded": True,
            "blind_assignment_id": assignment_id,
        },
        "admission": {"passed": True, "failed_gates": []},
        "dimensions": [
            {
                "dimension_id": dimension,
                "score": scores[dimension],
                "reason": "r",
                "span_refs": ["story-package/logline-1"],
                "valid": True,
            }
            for dimension in DIMENSION_IDS
        ],
    }
    scores_dir = evaluation_root / "human-scores"
    scores_dir.mkdir(parents=True, exist_ok=True)
    (scores_dir / f"{assignment_id}.json").write_text(
        json.dumps(record, ensure_ascii=False), encoding="utf-8"
    )


def flat_scores(value: int) -> dict:
    return {dimension: value for dimension in DIMENSION_IDS}


class BinTests(unittest.TestCase):
    def test_integers_and_halves_bin_deterministically(self) -> None:
        self.assertEqual(bin_score(4.0), "4")
        self.assertEqual(bin_score(3.5), "4")  # banker's rounding, deterministic
        self.assertEqual(bin_score(2.5), "2")

    def test_out_of_range_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            bin_score(6.0)


class AgreementTests(unittest.TestCase):
    def _setup(self, directory: str) -> tuple[Path, Path]:
        run_dir = Path(directory) / "run-a"
        evaluation_root = Path(directory) / "evaluation"
        run_dir.mkdir()
        return run_dir, evaluation_root

    def test_identical_scores_agree_perfectly(self) -> None:
        with TemporaryDirectory() as directory:
            run_dir, evaluation_root = self._setup(directory)
            judge_result(
                run_dir, "judge-x", "case_000", "artifact-a", flat_scores(4)
            )
            human_record(
                evaluation_root, "asg-1", "case_000", "artifact-a",
                "reviewer_01", flat_scores(4),
            )
            agreement = build_agreement(evaluation_root, [run_dir])
            self.assertEqual(agreement["joined_artifacts"], 1)
            block = agreement["spot_check_agreement"]["per_artifact"][0]
            self.assertEqual(block["raters_in_block"], 2)
            self.assertEqual(block["alpha"], 1.0)
            self.assertEqual(
                agreement["per_dimension_mean_difference_judge_minus_human"],
                {dimension: 0.0 for dimension in DIMENSION_IDS},
            )

    def test_systematic_offset_shows_in_mean_difference(self) -> None:
        with TemporaryDirectory() as directory:
            run_dir, evaluation_root = self._setup(directory)
            judge_result(
                run_dir, "judge-x", "case_000", "artifact-a", flat_scores(4)
            )
            human_record(
                evaluation_root, "asg-1", "case_000", "artifact-a",
                "reviewer_01", flat_scores(3),
            )
            agreement = build_agreement(evaluation_root, [run_dir])
            mean_diff = (
                agreement["per_dimension_mean_difference_judge_minus_human"]
            )
            self.assertTrue(all(value == 1.0 for value in mean_diff.values()))
            self.assertLess(
                agreement["spot_check_agreement"]["per_artifact"][0]["alpha"], 1.0
            )

    def test_one_sided_artifacts_are_reported_not_dropped(self) -> None:
        with TemporaryDirectory() as directory:
            run_dir, evaluation_root = self._setup(directory)
            judge_result(
                run_dir, "judge-x", "case_000", "artifact-a", flat_scores(4)
            )
            judge_result(
                run_dir, "judge-x", "case_002", "artifact-c", flat_scores(4)
            )
            human_record(
                evaluation_root, "asg-1", "case_000", "artifact-a",
                "reviewer_01", flat_scores(4),
            )
            human_record(
                evaluation_root, "asg-2", "case_001", "artifact-b",
                "reviewer_01", flat_scores(4),
            )
            agreement = build_agreement(evaluation_root, [run_dir])
            self.assertEqual(agreement["joined_artifacts"], 1)
            self.assertEqual(
                [item["artifact_id"] for item in agreement["unmatched"]["human_only"]],
                ["artifact-b"],
            )
            self.assertEqual(
                [item["artifact_id"] for item in agreement["unmatched"]["judge_only"]],
                ["artifact-c"],
            )

    def test_multiple_human_reviewers_join_the_same_artifact(self) -> None:
        with TemporaryDirectory() as directory:
            run_dir, evaluation_root = self._setup(directory)
            judge_result(
                run_dir, "judge-x", "case_000", "artifact-a", flat_scores(4)
            )
            human_record(
                evaluation_root, "asg-1", "case_000", "artifact-a",
                "reviewer_01", flat_scores(4),
            )
            human_record(
                evaluation_root, "asg-2", "case_000", "artifact-a",
                "reviewer_02", flat_scores(5),
            )
            humans = load_human_scores(evaluation_root)
            self.assertEqual(len(humans["artifact-a"]["raters"]), 2)
            observations = load_judge_observations([run_dir])
            self.assertIn("artifact-a", observations)


if __name__ == "__main__":
    unittest.main()
