import json
import unittest

from compute_evaluator_metrics import OUT, PAIR_DIR, load, observations, result_files


class ResultDiscoveryTests(unittest.TestCase):
    def test_invalid_span_files_are_excluded(self) -> None:
        self.assertFalse(
            [p for p in result_files() if ".invalid-span." in p.name]
        )


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


class ReportTests(unittest.TestCase):
    def setUp(self) -> None:
        self.report = load(OUT)

    def test_stability_rerun_is_not_counted_as_a_third_judge(self) -> None:
        counted = self.report["judges_counted"]
        self.assertEqual(len(counted), len(set(counted)))
        self.assertEqual(self.report["reruns_excluded_from_headline"], [])

    def test_narrow_pair_makes_localisation_informative(self) -> None:
        block = self.report["defect_localisation"]
        self.assertFalse(block["constructively_guaranteed"])
        self.assertIsNone(block["caveat"])

    def test_single_pair_resolution_is_disclosed(self) -> None:
        self.assertEqual(self.report["pairs_total"], 1)
        self.assertIn("0.0 and 1.0", self.report["resolution_warning"])

    def test_uncomputable_metrics_are_named_with_reasons(self) -> None:
        missing = self.report["not_computable_here"]
        self.assertNotIn("inter_model_agreement", missing)
        self.assertIn("spot_check_agreement", missing)

    def test_inter_model_agreement_is_reported(self) -> None:
        agreement = self.report["inter_model_agreement"]
        self.assertEqual(agreement["method"], "krippendorff_alpha_interval")
        self.assertEqual(agreement["raters"], 2)


if __name__ == "__main__":
    unittest.main()
