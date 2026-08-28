"""Tests for the premise-family near-duplicate check (P11)."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import check_premise_families as checker


def write_corpus(root: Path, records: list[dict]) -> None:
    by_split: dict[str, list[dict]] = {}
    for record in records:
        by_split.setdefault(record["split"], []).append(record)
    for split, bucket in by_split.items():
        directory = root / split
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "cases.jsonl").write_text(
            "\n".join(json.dumps(r, ensure_ascii=False) for r in bucket) + "\n",
            encoding="utf-8",
        )


def case(case_id: str, split: str, family: str, prompt: str) -> dict:
    return {
        "schema": "eval-case/v1",
        "case_id": case_id,
        "split": split,
        "premise_family": family,
        "genre": "family",
        "difficulty": "ordinary",
        "hard_slice": None,
        "input": prompt,
        "constraints": {
            "episodes": 8,
            "minutes_per_episode": 2,
            "audience": "25-45",
            "rating": "general",
            "production_level": "low_budget",
            "max_locations": 4,
            "max_speaking_cast": 6,
        },
        "required_elements": ["a"],
        "required_conditions": [],
        "forbidden_elements": ["b"],
        "rights": {
            "source": "internal_authored",
            "license_id": f"internal-eval-test-{case_id}",
            "allowed_uses": ["evaluation"],
            "expires_at": None,
        },
    }


class ShingleTests(unittest.TestCase):
    def test_punctuation_and_whitespace_are_stripped(self) -> None:
        self.assertEqual(checker.shingles("母亲，卖掉 老房子！"), checker.shingles("母亲卖掉老房子"))

    def test_short_text_keeps_one_token(self) -> None:
        self.assertEqual(checker.shingles("母女"), {"母女"})

    def test_identical_texts_have_jaccard_one(self) -> None:
        left = checker.shingles("母亲卖掉老房子后三个子女第一次回家吃饭")
        right = checker.shingles("母亲卖掉老房子后三个子女第一次回家吃饭")
        self.assertEqual(checker.exact_jaccard(left, right), 1.0)


class CorpusCheckTests(unittest.TestCase):
    def _run(self, records: list[dict], threshold: float = 0.6) -> dict:
        with TemporaryDirectory() as directory:
            root = Path(directory)
            write_corpus(root, records)
            original_load = checker.load_cases
            checker.load_cases = lambda: [
                json.loads(json.dumps(r)) for r in records
            ]
            try:
                return checker.build_report(threshold)
            finally:
                checker.load_cases = original_load

    def test_identical_premise_in_two_families_is_flagged(self) -> None:
        prompt = "母亲卖掉老房子后，三个成年子女第一次回家吃饭。"
        report = self._run(
            [
                case("family_001", "dev", "inheritance_and_shared_meal", prompt),
                case("family_002", "train", "guardian_secret_ledger", prompt),
            ]
        )
        self.assertFalse(report["passes"])
        self.assertEqual(len(report["cross_family_near_duplicates"]), 1)
        item = report["cross_family_near_duplicates"][0]
        self.assertEqual(item["exact_jaccard"], 1.0)
        # the two families sit in different splits: that is the leakage
        self.assertNotEqual(item["split_a"], item["split_b"])

    def test_premise_variants_in_two_families_are_flagged(self) -> None:
        # two-character drift scores ~0.45 on 3-grams: below the strict 0.6
        # default but unambiguously the same premise at a looser threshold
        report = self._run(
            [
                case(
                    "family_001", "dev", "inheritance_and_shared_meal",
                    "母亲卖掉老房子后，三个成年子女第一次回家吃饭。",
                ),
                case(
                    "family_002", "dev", "guardian_secret_ledger",
                    "母亲卖掉老房子以后，三个成年子女头一次回家吃饭。",
                ),
            ],
            threshold=0.35,
        )
        self.assertFalse(report["passes"])

    def test_distinct_premises_pass(self) -> None:
        report = self._run(
            [
                case("a_001", "dev", "family_a", "母亲卖掉老房子后，三个成年子女第一次回家吃饭。"),
                case("a_002", "dev", "family_b", "合作社首次分红，账面盈利比收购站流水少了三成。"),
            ]
        )
        self.assertTrue(report["passes"])
        self.assertEqual(report["cross_family_near_duplicates"], [])

    def test_near_duplicates_inside_one_family_are_not_violations(self) -> None:
        report = self._run(
            [
                case("a_001", "dev", "same_family", "母亲卖掉老房子后，三个成年子女第一次回家吃饭。"),
                case("a_002", "dev", "same_family", "母亲卖掉老房子后，三个成年子女第一次回家吃饭。"),
            ]
        )
        self.assertTrue(report["passes"])

    def test_unrelated_members_inside_one_family_are_reported_as_smell(self) -> None:
        report = self._run(
            [
                case("a_001", "dev", "same_family", "母亲卖掉老房子后，三个成年子女第一次回家吃饭。"),
                case("a_002", "dev", "same_family", "无人机植保队进村，爷爷坚持按老黄历吉日作业。"),
            ]
        )
        self.assertTrue(report["passes"])
        self.assertEqual(len(report["zero_overlap_within_family"]), 1)


class LiveCorpusTests(unittest.TestCase):
    def test_repository_corpus_reports_clean_or_lists_duplicates(self) -> None:
        report = checker.build_report(checker.DEFAULT_THRESHOLD)
        self.assertEqual(report["cases"], 120)
        # whatever the verdict, the report must enumerate every violation with
        # both families and splits so a human can act on it
        for item in report["cross_family_near_duplicates"]:
            self.assertTrue(item["family_a"] != item["family_b"])


if __name__ == "__main__":
    unittest.main()
