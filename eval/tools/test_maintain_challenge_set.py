import json
import unittest
from datetime import date

from maintain_challenge_set import ROOT, maintenance_plan


class ChallengeMaintenanceTests(unittest.TestCase):
    def test_quarterly_refresh_and_replacement_guard(self) -> None:
        policy = json.loads(
            (ROOT / "eval/adversarial/lifecycle-policy-v1.json").read_text("utf-8")
        )
        state = {
            "schema": "adversarial-maintenance-state/v1",
            "last_refresh": "2026-01-01",
            "pairs": [
                {
                    "pair_id": "pair_leaked",
                    "status": "accepted_hard",
                    "flags": ["holdout_leakage"],
                    "measurement_windows": [],
                    "replacement_pair_id": None,
                },
                {
                    "pair_id": "pair_saturated",
                    "status": "accepted_hard",
                    "flags": [],
                    "measurement_windows": [
                        {"detection_rate": 1.0, "localisation_rate": 1.0},
                        {"detection_rate": 0.99, "localisation_rate": 0.98},
                        {"detection_rate": 0.98, "localisation_rate": 0.95},
                    ],
                    "replacement_pair_id": "pair_fresh",
                },
            ],
            "production_failure_candidates": [
                {"case_id": f"failure_{index}", "rights_verified": True}
                for index in range(4)
            ],
        }
        plan = maintenance_plan(state, policy, date(2026, 7, 28))
        self.assertTrue(plan["refresh_due"])
        self.assertTrue(plan["refresh_ready"])
        self.assertEqual(plan["retire"][0]["pair_id"], "pair_saturated")
        self.assertEqual(
            plan["retirement_blocked"][0]["pair_id"], "pair_leaked"
        )


if __name__ == "__main__":
    unittest.main()
