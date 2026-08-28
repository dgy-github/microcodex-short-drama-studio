"""Tests for the spot-check sampling plan."""

from __future__ import annotations

import unittest

import plan_spot_check as planner


class SamplingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.records = [
            {"case_id": f"family_{index:03d}", "genre": "family", "split": "dev"}
            for index in range(1, 11)
        ] + [
            {"case_id": f"comedy_{index:03d}", "genre": "comedy", "split": "train"}
            for index in range(1, 4)
        ]

    def test_rate_takes_at_least_one_per_genre(self) -> None:
        sampled = planner.sample_cases(self.records, rate=0.2)
        genres = {record["genre"] for record in sampled}
        self.assertEqual(genres, {"family", "comedy"})
        family_ids = [r["case_id"] for r in sampled if r["genre"] == "family"]
        self.assertEqual(len(family_ids), 2)  # ceil(0.2 * 10)
        self.assertEqual(family_ids, ["family_001", "family_002"])  # deterministic

    def test_sampling_is_deterministic(self) -> None:
        first = planner.sample_cases(self.records, rate=0.2)
        second = planner.sample_cases(self.records, rate=0.2)
        self.assertEqual(first, second)

    def test_live_plan_covers_every_genre(self) -> None:
        plan = planner.build_plan("reviewer_01")
        records = planner.load_cases()
        sampled_genres = {
            record["genre"]
            for record in records
            if record["case_id"] in plan["cases"]
        }
        all_genres = {record["genre"] for record in records}
        self.assertEqual(sampled_genres, all_genres)

    def test_live_plan_includes_every_adversarial_pair(self) -> None:
        plan = planner.build_plan("reviewer_01")
        expected = {
            pair["pair_id"]
            for pair in planner.adversarial_baselines(1.0)
        }
        self.assertEqual(
            {pair["pair_id"] for pair in plan["adversarial_pairs"]}, expected
        )
        self.assertGreaterEqual(len(expected), 7)  # stage-0 narrow + six stage-1


if __name__ == "__main__":
    unittest.main()
