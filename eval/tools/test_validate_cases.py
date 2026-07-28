import json
import tempfile
import unittest
from pathlib import Path

from validate_cases import (
    load_corpus,
    load_jsonl,
    validate_corpus,
    validate_records,
)

CASES = Path(__file__).parents[1] / "cases"


class ValidateRecordsTests(unittest.TestCase):
    def test_every_split_file_is_valid(self) -> None:
        for split in ("dev", "train", "validation", "challenge"):
            path = CASES / split / "cases.jsonl"
            with self.subTest(split=split):
                self.assertEqual(validate_records(load_jsonl(path), split), [])

    def test_duplicate_case_id_is_rejected(self) -> None:
        records = load_jsonl(CASES / "dev" / "cases.jsonl")
        records[1]["case_id"] = records[0]["case_id"]
        self.assertIn(
            "duplicate case_id", "\n".join(validate_records(records, "dev"))
        )

    def test_record_in_the_wrong_directory_is_rejected(self) -> None:
        records = load_jsonl(CASES / "dev" / "cases.jsonl")
        self.assertIn(
            "split must be train", "\n".join(validate_records(records, "train"))
        )

    def test_train_requires_skill_derivation_rights(self) -> None:
        """A train case licensed only for evaluation cannot legally reach
        nanocodex, which is what the train split exists for."""
        records = load_jsonl(CASES / "train" / "cases.jsonl")
        records[0]["rights"] = dict(records[0]["rights"])
        records[0]["rights"]["allowed_uses"] = ["evaluation"]
        self.assertIn(
            "allowed_uses must be", "\n".join(validate_records(records, "train"))
        )

    def test_invalid_json_reports_line_number(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cases.jsonl"
            path.write_text(json.dumps({"ok": True}) + "\n{", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, r":2: invalid JSON"):
                load_jsonl(path)


class ValidateCorpusTests(unittest.TestCase):
    def test_repository_corpus_is_valid(self) -> None:
        self.assertEqual(validate_corpus(load_corpus(CASES)), [])

    def test_premise_family_split_across_splits_is_rejected(self) -> None:
        """The check that per-file validation structurally cannot perform."""
        records = load_corpus(CASES)
        family = records[0]["premise_family"]
        twin = dict(records[0])
        twin["case_id"] = "family_999"
        twin["split"] = "holdout"
        twin["rights"] = dict(twin["rights"])
        twin["rights"]["license_id"] = "internal-eval-9999"
        twin["premise_family"] = family
        errors = "\n".join(validate_corpus(records + [twin]))
        self.assertIn(f"premise_family {family!r} leaks across", errors)

    def test_duplicate_licence_across_splits_is_rejected(self) -> None:
        records = load_corpus(CASES)
        clone = dict(records[-1])
        clone["case_id"] = "family_998"
        errors = "\n".join(validate_corpus(records + [clone]))
        self.assertIn("appears 2 times", errors)

    def test_genre_quota_shortfall_is_reported(self) -> None:
        records = [r for r in load_corpus(CASES) if r["genre"] != "comedy"]
        errors = "\n".join(validate_corpus(records))
        self.assertIn("comedy expected 3 cases, got 0", errors)


if __name__ == "__main__":
    unittest.main()
