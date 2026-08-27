"""Tests for freeze enforcement (REQ-324)."""

from __future__ import annotations

import json
import shutil
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from check_eval_freeze import check, sha256_of

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "eval" / "manifests" / "eval-v0.1.0.json"
RUBRIC = ROOT / "eval" / "rubrics" / "judge-v1.yaml"


def fixture_root(directory: str) -> Path:
    root = Path(directory)
    (root / "eval" / "manifests").mkdir(parents=True)
    (root / "eval" / "rubrics").mkdir(parents=True)
    shutil.copy(MANIFEST, root / "eval" / "manifests" / "eval-v0.1.0.json")
    shutil.copy(RUBRIC, root / "eval" / "rubrics" / "judge-v1.yaml")
    return root


def freeze_record(root: Path, **overrides) -> dict:
    record = {
        "schema": "eval-freeze-record/v1",
        "frozen_at": "2026-08-27T00:00:00+00:00",
        "eval_version": "eval-v0.1.0",
        "rubric_version": "judge-v1",
        "manifest_sha256": sha256_of(root / "eval" / "manifests" / "eval-v0.1.0.json"),
        "rubric_sha256": sha256_of(root / "eval" / "rubrics" / "judge-v1.yaml"),
        "evidence": {
            "pillar_review": "eval/scores/pillar-review-30records.json",
            "spot_check": "eval/scores/spot-check-agreement-6joined.json",
        },
        "note": "test",
    }
    record.update(overrides)
    return record


class FreezeTests(unittest.TestCase):
    def test_absent_record_passes(self) -> None:
        with TemporaryDirectory() as directory:
            root = fixture_root(directory)
            self.assertEqual(check(root), 0)

    def test_matching_pins_and_existing_evidence_pass(self) -> None:
        with TemporaryDirectory() as directory:
            root = fixture_root(directory)
            (root / "eval" / "scores").mkdir(parents=True)
            for name in ("pillar-review-30records.json", "spot-check-agreement-6joined.json"):
                (root / "eval" / "scores" / name).write_text("{}", encoding="utf-8")
            (root / "eval" / "manifests" / "FREEZE.json").write_text(
                json.dumps(freeze_record(root)), encoding="utf-8"
            )
            self.assertEqual(check(root), 0)

    def test_manifest_drift_after_freeze_fails(self) -> None:
        with TemporaryDirectory() as directory:
            root = fixture_root(directory)
            (root / "eval" / "scores").mkdir(parents=True)
            for name in ("pillar-review-30records.json", "spot-check-agreement-6joined.json"):
                (root / "eval" / "scores" / name).write_text("{}", encoding="utf-8")
            record = freeze_record(root)
            (root / "eval" / "manifests" / "FREEZE.json").write_text(
                json.dumps(record), encoding="utf-8"
            )
            manifest = root / "eval" / "manifests" / "eval-v0.1.0.json"
            manifest.write_text(
                manifest.read_text(encoding="utf-8") + "\n", encoding="utf-8"
            )
            self.assertNotEqual(check(root), 0)

    def test_missing_evidence_fails(self) -> None:
        with TemporaryDirectory() as directory:
            root = fixture_root(directory)
            (root / "eval" / "manifests" / "FREEZE.json").write_text(
                json.dumps(freeze_record(root)), encoding="utf-8"
            )
            self.assertNotEqual(check(root), 0)

    def test_incomplete_record_fails(self) -> None:
        with TemporaryDirectory() as directory:
            root = fixture_root(directory)
            (root / "eval" / "manifests" / "FREEZE.json").write_text(
                json.dumps({"frozen_at": "2026-08-27"}), encoding="utf-8"
            )
            self.assertNotEqual(check(root), 0)


if __name__ == "__main__":
    unittest.main()
