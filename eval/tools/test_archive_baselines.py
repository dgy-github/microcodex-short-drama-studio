import hashlib
import json
import unittest
from pathlib import Path

from archive_baselines import ARCHIVE, canonical_bytes, load, sha256

PAIR_DIR = Path(__file__).parents[1] / "adversarial" / "stage0" / "motive-explicit"


def archives() -> list[Path]:
    return sorted(p for p in ARCHIVE.glob("*") if (p / "index.json").exists())


class ArchiveIntegrityTests(unittest.TestCase):
    def test_at_least_one_run_is_archived(self) -> None:
        """eval/runs/ is gitignored and generation is stochastic, so an
        unarchived baseline is unrecoverable once the machine changes."""
        self.assertTrue(archives(), "no baseline archive found")

    def test_every_archived_package_matches_its_recorded_hash(self) -> None:
        for archive in archives():
            index = load(archive / "index.json")
            for case in index["cases"]:
                with self.subTest(run=index["run_id"], case=case["case_id"]):
                    package = archive / case["package"]
                    self.assertTrue(package.exists())
                    self.assertEqual(sha256(package), case["content_hash"])

    def test_every_wrapper_agrees_with_its_package(self) -> None:
        for archive in archives():
            index = load(archive / "index.json")
            for case in index["cases"]:
                with self.subTest(case=case["case_id"]):
                    wrapper = load(archive / case["wrapper"])
                    body = canonical_bytes(archive / case["package"])
                    digest = "sha256:" + hashlib.sha256(body).hexdigest()
                    self.assertEqual(wrapper["content_hash"], digest)
                    self.assertEqual(wrapper["artifact_id"], case["artifact_id"])

    def test_index_records_generation_parameters(self) -> None:
        """Without model, seed and temperature the archive cannot say what it
        is a baseline of."""
        for archive in archives():
            index = load(archive / "index.json")
            generator = index["generator"]
            for field in ("model", "seed", "temperature"):
                self.assertIn(field, generator)

    def test_no_provider_responses_were_archived(self) -> None:
        """Raw responses carry usage and billing metadata and are telemetry,
        not evaluation input."""
        for archive in archives():
            self.assertEqual(list(archive.glob("*.provider.json")), [])

    def test_all_run_cases_are_covered(self) -> None:
        for archive in archives():
            index = load(archive / "index.json")
            names = {p.name for p in archive.glob("*.story-package.json")}
            self.assertEqual(len(names), len(index["cases"]))


class PairArchiveConsistencyTests(unittest.TestCase):
    def test_pair_baseline_matches_the_archived_baseline(self) -> None:
        """The stage-0 pair keeps its own copy for self-containment. If the two
        copies diverge, the probe and the archive describe different stories."""
        pair_copy = PAIR_DIR / "baseline.story-package.json"
        self.assertTrue(pair_copy.exists())
        digest = sha256(pair_copy)
        found = [
            case
            for archive in archives()
            for case in load(archive / "index.json")["cases"]
            if case["content_hash"] == digest
        ]
        self.assertTrue(
            found, "pair baseline does not match any archived baseline"
        )
        self.assertEqual(found[0]["case_id"], "comedy_002")


if __name__ == "__main__":
    unittest.main()
