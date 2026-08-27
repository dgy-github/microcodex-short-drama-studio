"""Tests for split assignment at the 120-case scale (REQ-326)."""

from __future__ import annotations

import unittest
from collections import Counter
from pathlib import Path

import split_cases

CASES = Path(__file__).parents[1] / "cases"


class SplitAssignmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.cases = split_cases.load_all()
        cls.assigned = split_cases.assign(cls.cases)

    def test_every_case_is_assigned_exactly_once(self) -> None:
        total = sum(len(bucket) for bucket in self.assigned.values())
        self.assertEqual(total, len(self.cases))
        self.assertEqual(total, 120)

    def test_split_sizes_follow_the_parent_ratio(self) -> None:
        sizes = {split: len(bucket) for split, bucket in self.assigned.items()}
        self.assertEqual(
            sizes, {"dev": 38, "train": 37, "validation": 30, "holdout": 0, "challenge": 15}
        )

    def test_holdout_stays_sealed(self) -> None:
        self.assertEqual(self.assigned["holdout"], [])

    def test_premise_families_never_cross_splits(self) -> None:
        self.assertEqual(split_cases.check_families(self.assigned), [])
        homes: dict[str, set[str]] = {}
        for split, bucket in self.assigned.items():
            for case in bucket:
                homes.setdefault(case["premise_family"], set()).add(split)
        for family, splits in homes.items():
            with self.subTest(family=family):
                self.assertEqual(len(splits), 1)

    def test_train_alone_carries_skill_derivation_rights(self) -> None:
        for split, bucket in self.assigned.items():
            for case in bucket:
                with self.subTest(split=split, case=case["case_id"]):
                    expected = set(split_cases.SPLIT_USES[split])
                    self.assertEqual(set(case["rights"]["allowed_uses"]), expected)

    def test_genre_quotas_hold_at_120(self) -> None:
        counts = Counter(case["genre"] for case in self.cases)
        self.assertEqual(
            counts,
            Counter(
                {
                    "family": 20,
                    "urban_romance": 16,
                    "revenge": 16,
                    "suspense": 16,
                    "workplace": 12,
                    "rural": 12,
                    "comedy": 12,
                    "historical": 8,
                    "cross_genre": 8,
                }
            ),
        )

    def test_files_match_the_assignment_table(self) -> None:
        # mirrors the --check comparison without invoking main(), which would
        # rewrite the corpus files instead of verifying them
        import json

        for split, bucket in self.assigned.items():
            path = CASES / split / "cases.jsonl"
            existing = [
                json.loads(line)
                for line in path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
            self.assertEqual(existing, bucket)


if __name__ == "__main__":
    unittest.main()
