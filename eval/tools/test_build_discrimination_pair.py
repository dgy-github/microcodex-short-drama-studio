import copy
import json
import unittest
from pathlib import Path

from build_discrimination_pair import ROOT, construct_pair


class DiscriminationPairTests(unittest.TestCase):
    def test_professional_candidate_pair_is_evaluation_only(self) -> None:
        source = ROOT / (
            "eval/baselines/baseline-deepseek-v4-pro-20260727/"
            "family_001.story-package.json"
        )
        positive = json.loads(source.read_text(encoding="utf-8"))
        negative = copy.deepcopy(positive)
        negative["logline"]["text"] = "家庭秘密被揭开。"
        pair = construct_pair(
            positive,
            negative,
            pair_id="pair_professional_1",
            case_id="family_001",
            author_id="professional_author_1",
            seeded_defects=[
                {
                    "problem_code": "HUMAN_GENERIC",
                    "spans": ["story-package/logline-1"],
                    "target_dimension": "human_credibility",
                    "load_bearing": True,
                    "repair_cost": "scene_rewrite",
                }
            ],
            masking_virtue=["hook_density"],
        )
        self.assertEqual(pair["pair_kind"], "discrimination")
        self.assertEqual(pair["status"], "candidate")
        self.assertEqual(pair["rights"]["allowed_uses"], ["evaluation"])
        self.assertNotEqual(pair["positive_artifact_id"], pair["negative_artifact_id"])

    def test_identical_members_are_rejected(self) -> None:
        source = ROOT / (
            "eval/baselines/baseline-deepseek-v4-pro-20260727/"
            "family_001.story-package.json"
        )
        package = json.loads(source.read_text(encoding="utf-8"))
        with self.assertRaises(ValueError):
            construct_pair(
                package,
                package,
                pair_id="pair_bad",
                case_id="family_001",
                author_id="professional_author_1",
                seeded_defects=[{"problem_code": "HUMAN_GENERIC"}],
                masking_virtue=[],
            )


if __name__ == "__main__":
    unittest.main()
