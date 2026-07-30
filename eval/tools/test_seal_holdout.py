import json
import tempfile
import unittest
from pathlib import Path

from seal_holdout import create_seal, verify_seal


class HoldoutSealTests(unittest.TestCase):
    def test_commitment_verifies_and_detects_private_change(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            private = Path(directory)
            case = private / "cases.jsonl"
            case.write_text(
                json.dumps({"case_id": "hidden_1"}) + "\n",
                encoding="utf-8",
            )
            seal = create_seal(private, "eval-v1.0.0")
            self.assertEqual(seal["allowed_uses"], ["evaluation"])
            self.assertTrue(verify_seal(private, seal))
            case.write_text(
                json.dumps({"case_id": "hidden_changed"}) + "\n",
                encoding="utf-8",
            )
            self.assertFalse(verify_seal(private, seal))

    def test_empty_private_set_cannot_be_sealed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(ValueError):
                create_seal(Path(directory), "eval-v1.0.0")


if __name__ == "__main__":
    unittest.main()
