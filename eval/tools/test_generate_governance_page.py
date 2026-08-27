"""Tests for the generated governance page (REQ-323)."""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import generate_governance_page as generator

ROOT = Path(__file__).parents[2]


class GenerationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.page = generator.render()

    def test_manifest_numbers_are_injected(self) -> None:
        # floors and pass threshold come from the manifest, not the template
        self.assertIn("var FLOOR=3.0,PASS=3.5;", self.page)
        self.assertIn("人物可信 25%", self.page)
        self.assertIn("原创可交付 25%", self.page)

    def test_status_section_reflects_repository_state(self) -> None:
        counts = generator.repository_status()["case_counts"]
        total = sum(counts.values())
        self.assertIn(f"{total} 个原创中文用例", self.page)
        for split, count in counts.items():
            display = generator.SPLIT_DISPLAY.get(split, split)
            self.assertIn(f"{display} {count}", self.page)
        # the stale hand-written claims must be gone
        self.assertNotIn("12 个原创中文种子用例", self.page)
        self.assertNotIn("九个测试", self.page)
        self.assertNotIn("对抗集因此为空", self.page)

    def test_freeze_state_is_reported(self) -> None:
        self.assertIn("评测契约未冻结", self.page)

    def test_generated_note_forbids_hand_edits(self) -> None:
        self.assertIn("请勿手工编辑", self.page)


class CheckModeTests(unittest.TestCase):
    def test_check_fails_on_a_hand_edited_page(self) -> None:
        with TemporaryDirectory() as directory:
            page = Path(directory) / "eval-governance.html"
            page.write_text(
                generator.render() + "<!-- hand edit -->", encoding="utf-8"
            )
            self.assertNotEqual(generator.check_page(page), 0)

    def test_check_passes_on_the_generated_page(self) -> None:
        with TemporaryDirectory() as directory:
            page = Path(directory) / "page.html"
            page.write_text(generator.render(), encoding="utf-8")
            self.assertEqual(generator.check_page(page), 0)


if __name__ == "__main__":
    unittest.main()
