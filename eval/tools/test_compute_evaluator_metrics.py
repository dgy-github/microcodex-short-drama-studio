"""Tests for the set-level evaluator metrics (multi-pair aggregation)."""

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from compute_evaluator_metrics import (
    DEFAULT_PAIR,
    load,
    measure_pair,
    observations,
    pooled_agreement,
    report_is_current,
    result_files,
)
from run_stage0_probe import load_rubric

DIMENSION_IDS = [d["id"] for d in load_rubric()]


class ResultDiscoveryTests(unittest.TestCase):
    def test_invalid_span_files_are_excluded(self) -> None:
        self.assertFalse(
            [p for p in result_files(DEFAULT_PAIR) if ".invalid-span." in p.name]
        )

    def test_report_check_rejects_missing_malformed_and_stale_json(self) -> None:
        with TemporaryDirectory() as directory:
            path = Path(directory) / "evaluator-metrics.json"
            expected = {"schema": "evaluator-metrics/v1", "value": 1}
            self.assertFalse(report_is_current(expected, path))
            path.write_text("not json", encoding="utf-8")
            self.assertFalse(report_is_current(expected, path))
            path.write_text(json.dumps({"schema": "evaluator-metrics/v1", "value": 0}), encoding="utf-8")
            self.assertFalse(report_is_current(expected, path))
            path.write_text(json.dumps(expected, indent=4), encoding="utf-8")
            self.assertTrue(report_is_current(expected, path))


class ObservationTests(unittest.TestCase):
    def test_direction_is_measured_within_the_pair(self) -> None:
        """seeded_defect_detection compares the negative to its OWN baseline,
        not to a quality bar."""
        ids = ["a", "b"]
        sample = {
            "A": {d: {"score": 5, "spans": ["s1"]} for d in ids},
            "B": {d: {"score": 2, "spans": ["s1"]} for d in ids},
        }
        rows = observations([sample], "A", "B", ids, {"s1"})
        self.assertTrue(rows[0]["negative_lower"])
        self.assertTrue(rows[0]["localised"])

    def test_citation_off_the_seeded_spans_is_not_localised(self) -> None:
        ids = ["a"]
        sample = {
            "A": {"a": {"score": 5, "spans": ["s1"]}},
            "B": {"a": {"score": 1, "spans": ["elsewhere"]}},
        }
        rows = observations([sample], "A", "B", ids, {"s1"})
        self.assertTrue(rows[0]["negative_lower"])
        self.assertFalse(rows[0]["localised"])


class NarrowPairReportTests(unittest.TestCase):
    """The historical stage-0 narrow pair, measured through the new API."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.measured = measure_pair(DEFAULT_PAIR, DIMENSION_IDS)

    def test_stability_rerun_is_not_counted_as_a_third_judge(self) -> None:
        models = [j["judge_model"] for j in self.measured["primary"]]
        self.assertEqual(len(models), len(set(models)))

    def test_narrow_pair_localisation_is_not_constructively_guaranteed(self) -> None:
        self.assertFalse(self.measured["constructively_guaranteed_localisation"])

    def test_every_result_carries_a_matching_fingerprint(self) -> None:
        self.assertTrue(self.measured["all_inputs_fingerprinted"])
        self.assertEqual(len(self.measured["input_fingerprints"]), 1)


class AggregationTests(unittest.TestCase):
    def _fixture(self, root: Path, name: str, detect: bool, judge: str) -> dict:
        pair_dir = root / name
        pair_dir.mkdir(parents=True)
        negative = load(DEFAULT_PAIR / "negative.story-package.json")
        (pair_dir / "negative.story-package.json").write_text(
            json.dumps(negative, ensure_ascii=False), encoding="utf-8"
        )
        defect_spans = load(DEFAULT_PAIR / "pair.json")["seeded_defects"][0]["spans"]
        (pair_dir / "pair.json").write_text(
            json.dumps(
                {"pair_id": f"{name}", "seeded_defects": [{"spans": defect_spans}]},
                ensure_ascii=False,
            ),
            encoding="utf-8",
        )
        span = defect_spans[0]
        base_score, negative_score = (4, 2) if detect else (4, 4)
        forward = []
        reverse = []
        for _ in range(3):
            forward.append(
                {
                    "A": {d: {"score": base_score, "spans": [span]} for d in DIMENSION_IDS},
                    "B": {d: {"score": negative_score, "spans": [span]} for d in DIMENSION_IDS},
                    "preferred": "A",
                }
            )
            # in the reverse order the labels swap sides, as the probe presents them
            reverse.append(
                {
                    "A": {d: {"score": negative_score, "spans": [span]} for d in DIMENSION_IDS},
                    "B": {d: {"score": base_score, "spans": [span]} for d in DIMENSION_IDS},
                    "preferred": "B",
                }
            )
        (pair_dir / f"judge-{judge}.result.json").write_text(
            json.dumps(
                {
                    "forward": forward,
                    "reverse": reverse,
                    "summary": {
                        "judge_model": judge,
                        "route_provider": "fixture",
                        "input_fingerprint": f"sha256:{name}",
                        "samples_per_artifact": 3,
                        "baseline_scores": {d: base_score for d in DIMENSION_IDS},
                        "negative_scores": {d: negative_score for d in DIMENSION_IDS},
                    },
                },
                ensure_ascii=False,
            ),
            encoding="utf-8",
        )
        return measure_pair(pair_dir, DIMENSION_IDS)

    def test_detection_requires_all_primary_judges(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            detected = self._fixture(root, "p-detected", True, "judge-x")
            self.assertTrue(detected["detected"])
            missed = self._fixture(root, "p-missed", False, "judge-x")
            self.assertFalse(missed["detected"])

    def test_pooled_agreement_needs_judges_in_every_pair(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            pair_one = self._fixture(root, "q1", True, "judge-x")
            pair_two = self._fixture(root, "q2", True, "judge-x")
            pooled = pooled_agreement([pair_one, pair_two], DIMENSION_IDS)
            self.assertIsNone(pooled)  # one judge alone cannot agree

    def test_pooled_agreement_pools_items_across_pairs(self) -> None:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            pairs = []
            for index in range(2):
                pair_dir = root / f"r{index}"
                self._fixture(root, f"r{index}", True, "judge-x")
                # clone the fixture with a second judge at identical scores
                first = load(pair_dir / "judge-judge-x.result.json")
                second = json.loads(json.dumps(first))
                second["summary"]["judge_model"] = "judge-y"
                (pair_dir / "judge-judge-y.result.json").write_text(
                    json.dumps(second, ensure_ascii=False), encoding="utf-8"
                )
                pairs.append(measure_pair(pair_dir, DIMENSION_IDS))
            pooled = pooled_agreement(pairs, DIMENSION_IDS)
            self.assertIsNotNone(pooled)
            self.assertEqual(pooled["raters"], 2)
            self.assertEqual(pooled["items"], len(pairs) * len(DIMENSION_IDS) * 2)
            self.assertEqual(pooled["value"], 1.0)


if __name__ == "__main__":
    unittest.main()
