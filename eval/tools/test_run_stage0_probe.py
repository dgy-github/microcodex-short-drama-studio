import json
import re
import unittest
from pathlib import Path

from run_stage0_probe import (
    JUDGES,
    PAIR_DIR,
    build_system,
    consistency_metrics,
    load,
    load_rubric,
    median_scores,
    normalize_owned_field_spans,
    resolve_route,
    self_consistency,
    specificity_metrics,
    valid_line_spans,
    validate_judgment,
)


def sample(scores: dict[str, int], spans: list[str], preferred: str = "A") -> dict:
    block = {
        dimension: {"score": score, "reason": "r", "spans": spans}
        for dimension, score in scores.items()
    }
    return {"A": block, "B": block, "preferred": preferred}


class RubricTests(unittest.TestCase):
    def test_rubric_has_ten_dimensions(self) -> None:
        self.assertEqual(len(load_rubric()), 10)

    def test_system_prompt_names_every_dimension(self) -> None:
        dimensions = load_rubric()
        system = build_system(dimensions)
        for dimension in dimensions:
            self.assertIn(dimension["id"], system)


class JudgeConfigTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = load(JUDGES)

    def test_generator_family_is_disjoint_from_judges(self) -> None:
        families = {j["family"] for j in self.config["judges"]}
        self.assertNotIn(self.config["generator"]["family"], families)

    def test_at_least_two_judge_families(self) -> None:
        families = {j["family"] for j in self.config["judges"]}
        self.assertGreaterEqual(len(families), 2)

    def test_no_secret_material_in_tracked_config(self) -> None:
        """eval/judges.json is tracked; it must name env vars, never hold values."""
        raw = JUDGES.read_text(encoding="utf-8")
        for judge in self.config["judges"]:
            self.assertNotIn("api_key", judge)
            for route in judge["routes"]:
                self.assertIn("api_key_env", route)
                self.assertNotIn("api_key", route)
        self.assertNotIn("Bearer", raw)
        # nothing shaped like a live credential
        self.assertIsNone(re.search(r"[0-9a-f]{32}\.", raw))
        self.assertIsNone(
            re.search(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-", raw)
        )

    def test_temperature_permits_variance(self) -> None:
        """At temperature 0 the median of three samples is the sample, and
        self_consistency is trivially 1.0."""
        self.assertGreater(self.config["sampling"]["temperature"], 0)
        self.assertGreaterEqual(self.config["sampling"]["samples_per_artifact"], 3)


class ValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.baseline = load(PAIR_DIR / "baseline.story-package.json")
        self.negative = load(PAIR_DIR / "negative.story-package.json")
        self.ids = [d["id"] for d in load_rubric()]
        self.span = sorted(valid_line_spans(self.baseline))[:1]

    def test_full_dimension_set_passes(self) -> None:
        value = sample({d: 3 for d in self.ids}, self.span)
        validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_missing_dimension_is_rejected(self) -> None:
        value = sample({d: 3 for d in self.ids[:-1]}, self.span)
        with self.assertRaisesRegex(ValueError, "missing dimensions"):
            validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_span_outside_the_artifact_is_rejected(self) -> None:
        value = sample({d: 3 for d in self.ids}, ["story-package/scene-9/dialogue-9"])
        with self.assertRaisesRegex(ValueError, "invalid spans"):
            validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_real_beat_span_is_accepted(self) -> None:
        value = sample({d: 3 for d in self.ids}, ["story-package/beats-1"])
        validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_owned_field_span_normalizes_to_its_node(self) -> None:
        value = sample(
            {d: 3 for d in self.ids},
            ["story-package/scene-2/dialogue-1.subtext"],
        )
        normalize_owned_field_spans(value, self.baseline, self.baseline)
        self.assertEqual(
            value["A"][self.ids[0]]["spans"],
            ["story-package/scene-2/dialogue-1"],
        )
        validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_production_fields_expand_to_real_source_nodes(self) -> None:
        value = sample(
            {d: 3 for d in self.ids},
            [
                "story-package/production/locations",
                "story-package/production/speaking_cast",
            ],
        )
        normalize_owned_field_spans(value, self.baseline, self.baseline)
        spans = value["A"][self.ids[0]]["spans"]
        self.assertIn("story-package/scene-1", spans)
        self.assertIn("story-package/char-1", spans)
        validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_out_of_range_score_is_rejected(self) -> None:
        scores = {d: 3 for d in self.ids}
        scores[self.ids[0]] = 9
        value = sample(scores, self.span)
        with self.assertRaisesRegex(ValueError, "score must be 1-5"):
            validate_judgment(value, self.baseline, self.baseline, self.ids)


class AggregationTests(unittest.TestCase):
    def test_median_ignores_a_single_outlier(self) -> None:
        ids = ["a"]
        samples = [
            sample({"a": 5}, ["x"]),
            sample({"a": 5}, ["x"]),
            sample({"a": 1}, ["x"]),
        ]
        self.assertEqual(median_scores(samples, "A", ids), {"a": 5})

    def test_self_consistency_detects_disagreement(self) -> None:
        ids = ["a", "b"]
        samples = [
            sample({"a": 5, "b": 3}, ["x"]),
            sample({"a": 5, "b": 4}, ["x"]),
        ]
        self.assertEqual(self_consistency(samples, "A", ids), 0.5)

    def test_consistency_metrics_cover_both_orders_and_sides(self) -> None:
        ids = ["a"]
        forward = [sample({"a": 5}, ["x"]), sample({"a": 5}, ["x"])]
        reverse = [sample({"a": 5}, ["x"]), sample({"a": 4}, ["x"])]
        metrics = consistency_metrics(forward, reverse, ids)
        self.assertEqual(
            set(metrics["self_consistency_by_order"]),
            {"forward_A", "forward_B", "reverse_A", "reverse_B"},
        )
        self.assertEqual(metrics["self_consistency"], 0.5)

    def test_specificity_reports_all_and_cross_pillar_views(self) -> None:
        dimensions = [
            {"id": "target", "pillar": "character"},
            {"id": "same", "pillar": "character"},
            {"id": "cross_drop", "pillar": "structure"},
            {"id": "cross_keep", "pillar": "delivery"},
        ]
        metrics = specificity_metrics(
            {dimension["id"]: 5.0 for dimension in dimensions},
            {
                "target": 4.0,
                "same": 4.0,
                "cross_drop": 4.0,
                "cross_keep": 5.0,
            },
            "target",
            dimensions,
        )
        self.assertAlmostEqual(metrics["specificity_all"], 1 / 3)
        self.assertEqual(metrics["specificity_cross_pillar"], 0.5)
        self.assertEqual(metrics["specificity"], metrics["specificity_all"])
        self.assertEqual(
            metrics["collateral_dimensions_all"], ["cross_drop", "same"]
        )
        self.assertEqual(
            metrics["collateral_dimensions_cross_pillar"], ["cross_drop"]
        )

    def test_specificity_rejects_target_without_a_pillar(self) -> None:
        with self.assertRaisesRegex(ValueError, "target dimension has no pillar"):
            specificity_metrics({"a": 5.0}, {"a": 4.0}, "missing", [])


class PairSelfContainmentTests(unittest.TestCase):
    def test_both_members_live_in_the_pair_directory(self) -> None:
        """F4: the positive used to exist only under the gitignored eval/runs/,
        which made the pair unrebuildable elsewhere."""
        for name in ("baseline.story-package.json", "negative.story-package.json"):
            self.assertTrue((PAIR_DIR / name).exists(), name)

    def test_probe_does_not_read_the_ignored_run_directory(self) -> None:
        source = Path(__file__).with_name("run_stage0_probe.py").read_text(
            encoding="utf-8"
        )
        self.assertNotIn('"runs"', source)

    def test_saved_primary_and_narrow_results_have_both_specificity_views(self) -> None:
        pair_dirs = [
            PAIR_DIR,
            PAIR_DIR.with_name("motive-explicit-narrow"),
        ]
        for pair_dir in pair_dirs:
            summary = load(pair_dir / "probe-summary.json")
            self.assertIn("min_specificity_all", summary)
            self.assertIn("min_specificity_cross_pillar", summary)
            self.assertEqual(summary["min_specificity"], summary["min_specificity_all"])
            for judge in summary["judges"]:
                self.assertIn("specificity_all", judge)
                self.assertIn("specificity_cross_pillar", judge)
                self.assertEqual(judge["specificity"], judge["specificity_all"])




class RouteTests(unittest.TestCase):
    """Routes are alternate vendors for ONE model, not extra judges."""

    def setUp(self) -> None:
        self.config = load(JUDGES)

    def test_alternate_vendor_does_not_add_a_judge_family(self) -> None:
        glm = next(j for j in self.config["judges"] if j["family"] == "zhipu")
        self.assertGreater(len(glm["routes"]), 1)
        families = [j["family"] for j in self.config["judges"]]
        self.assertEqual(len(families), len(set(families)))

    def test_every_judge_uses_the_routes_shape(self) -> None:
        for judge in self.config["judges"]:
            self.assertIn("routes", judge)
            for route in judge["routes"]:
                self.assertIn("provider", route)
                self.assertIn("api_key_env", route)

    def test_resolve_route_requires_endpoint_and_key(self) -> None:
        judge = {
            "model": "m",
            "routes": [
                {"provider": "a", "endpoint": None, "api_key_env": "NOPE_A"},
                {"provider": "b", "endpoint": "https://x", "api_key_env": "NOPE_B"},
            ],
        }
        with self.assertRaises(SystemExit):
            resolve_route(judge)

    def test_resolve_route_skips_a_blocked_route(self) -> None:
        judge = {
            "model": "m",
            "routes": [
                {
                    "provider": "blocked",
                    "endpoint": "https://blocked.example",
                    "api_key_env": "PATH",
                    "blocked_on": "account_balance",
                },
                {
                    "provider": "ready",
                    "endpoint": "https://ready.example",
                    "api_key_env": "PATH",
                    "blocked_on": None,
                },
            ],
        }
        self.assertEqual(resolve_route(judge)["provider"], "ready")


if __name__ == "__main__":
    unittest.main()
