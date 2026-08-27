"""Tests for the stage-1 masking probe pairs (REQ-325)."""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import jsonschema
from build_stage1_masking_pairs import (
    BASE_ARTIFACT,
    RECIPES,
    STAGE0,
    STAGE1,
    apply_recipe,
    assert_only_edits_changed,
    load,
    main,
)
from generate_baselines import PACKAGE_SCHEMA, load_cases, load_json, validate_package

ROOT = Path(__file__).parents[2]
PAIR_SCHEMA = ROOT / "schemas" / "eval-adversarial-pair-v1.json"
NARROW_PAIR = (
    ROOT / "eval" / "adversarial" / "stage0" / "motive-explicit-narrow" / "pair.json"
)
STAGE0_PROBLEM_CODES = {"MOTIVE_EXPLICIT"}
STAGE1_PROBLEM_CODES = {
    "HOOK_FAKE", "FALSE_PAYOFF", "EMOTION_UNEARNED",
    "VOICE_COLLAPSE", "PLOT_CONVENIENCE", "TROPE_STACK",
}


def comedy_case() -> dict:
    return next(
        case
        for case in load_cases(ROOT / "eval" / "cases" / "dev" / "cases.jsonl")
        if case["case_id"] == "comedy_002"
    )


class Stage1PairTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        main()  # regenerate the tracked artifacts; tests pin them to the script

    def test_every_negative_passes_admission_gates(self) -> None:
        case = comedy_case()
        schema = load_json(PACKAGE_SCHEMA)
        for recipe in RECIPES:
            with self.subTest(recipe=recipe["dir"]):
                negative = load(
                    STAGE1 / recipe["dir"] / "negative.story-package.json"
                )
                validate_package(negative, case, schema)

    def test_negative_differs_only_at_declared_edits(self) -> None:
        baseline = load(STAGE0 / "baseline.story-package.json")
        for recipe in RECIPES:
            with self.subTest(recipe=recipe["dir"]):
                negative, spans = apply_recipe(baseline, recipe)
                assert_only_edits_changed(recipe, baseline, negative)
                tracked = load(
                    STAGE1 / recipe["dir"] / "negative.story-package.json"
                )
                self.assertEqual(negative, tracked)
                pair = load(STAGE1 / recipe["dir"] / "pair.json")
                self.assertEqual(pair["seeded_defects"][0]["spans"], spans)

    def test_every_pair_is_schema_valid_and_load_bearing(self) -> None:
        schema = load_json(PAIR_SCHEMA)
        for recipe in RECIPES:
            with self.subTest(recipe=recipe["dir"]):
                pair = load(STAGE1 / recipe["dir"] / "pair.json")
                jsonschema.Draft202012Validator(schema).validate(pair)
                defect = pair["seeded_defects"][0]
                self.assertTrue(defect["load_bearing"])
                self.assertIn(defect["repair_cost"], {"scene_rewrite", "restructure"})
                self.assertEqual(pair["positive_artifact_id"], BASE_ARTIFACT)
                self.assertEqual(pair["admission_checks"], "pass")
                self.assertEqual(pair["rights"]["allowed_uses"], ["evaluation"])

    def test_shared_base_positive_is_identical_across_all_pairs(self) -> None:
        baseline = load(STAGE0 / "baseline.story-package.json")
        for recipe in RECIPES:
            with self.subTest(recipe=recipe["dir"]):
                self.assertEqual(
                    load(STAGE1 / recipe["dir"] / "baseline.story-package.json"),
                    baseline,
                )

    def test_char_delta_stays_bounded(self) -> None:
        for recipe in RECIPES:
            with self.subTest(recipe=recipe["dir"]):
                pair = load(STAGE1 / recipe["dir"] / "pair.json")
                self.assertLessEqual(
                    abs(pair["confound_controls"]["char_count_delta_ratio"]),
                    0.02,
                )

    def test_all_seven_masking_recipes_are_covered(self) -> None:
        covered = set(STAGE0_PROBLEM_CODES)
        for recipe in RECIPES:
            covered.add(recipe["problem_code"])
        self.assertEqual(
            covered,
            STAGE0_PROBLEM_CODES | STAGE1_PROBLEM_CODES,
        )
        narrow = load(NARROW_PAIR)
        self.assertEqual(
            narrow["seeded_defects"][0]["problem_code"], "MOTIVE_EXPLICIT"
        )
        self.assertEqual(narrow["positive_artifact_id"], BASE_ARTIFACT)

    def test_target_dimensions_span_multiple_pillars(self) -> None:
        targets = {recipe["target_dimension"] for recipe in RECIPES}
        # two causal-coherence recipes approach one dimension from different
        # masking paths; the probe's variable is the path, not the dimension
        self.assertEqual(len(targets), 5)
        # four rubric pillars should all be exercised across the seven paths
        # (narrow pair covers dialogue_subtext -> character_credibility)
        self.assertIn("causal_coherence", targets)
        self.assertIn("short_drama_pacing", targets)
        self.assertIn("emotional_progression", targets)
        self.assertIn("character_distinction", targets)
        self.assertIn("genre_fulfillment", targets)


if __name__ == "__main__":
    unittest.main()
