import copy
import unittest

from professional_gate import evaluate_release


def seal(case_count: int) -> dict:
    return {
        "schema": "holdout-seal/v1",
        "seal_id": "seal_0123456789abcdef0123456789abcdef",
        "eval_version": "eval-v1.0.0",
        "case_count": case_count,
        "file_count": 1,
        "commitment_sha256": "a" * 64,
        "created_at": "2026-07-28T00:00:00Z",
        "allowed_uses": ["evaluation"],
        "status": "sealed",
    }


def evidence(preference: str = "candidate") -> dict:
    pairs = []
    for index in range(4):
        pairs.append(
            {
                "pair_id": f"pair_{index}",
                "case_id": f"case_{index}",
                "genre": "family" if index < 2 else "suspense",
                "expected_preference": "candidate",
                "admission_passed": True,
                "policy_passed": True,
                "reviews": [
                    {
                        "rater_id": "writer_1",
                        "credential": "working_screenwriter",
                        "blind": True,
                        "preference": preference,
                        "dimensions": {
                            "human_credibility": 4,
                            "originality": 4,
                            "causal_coherence": 4,
                        },
                    },
                    {
                        "rater_id": "editor_1",
                        "credential": "story_editor",
                        "blind": True,
                        "preference": preference,
                        "dimensions": {
                            "human_credibility": 4,
                            "originality": 4,
                            "causal_coherence": 4,
                        },
                    },
                    {
                        "rater_id": "viewer_1",
                        "credential": "target_viewer",
                        "blind": True,
                        "preference": preference,
                        "dimensions": {
                            "human_credibility": 4,
                            "originality": 4,
                            "causal_coherence": 4,
                        },
                    },
                ],
                "adjudication": None,
            }
        )
    return {
        "schema": "professional-release-evidence/v1",
        "evaluation_id": "evaluation_1",
        "eval_version": "eval-v1.0.0",
        "candidate_kind": "model",
        "candidate_id": "candidate_1",
        "incumbent_id": "incumbent_1",
        "holdout_seal": seal(4),
        "critical_dimensions": [
            "human_credibility",
            "originality",
            "causal_coherence",
        ],
        "pair_reviews": pairs,
        "critical_dimension_deltas": {
            "human_credibility": 0.1,
            "originality": 0.0,
            "causal_coherence": 0.0,
        },
        "genre_slice_deltas": {"family": 0.1, "suspense": 0.05},
        "critical_failure_delta": 0,
        "overlap_blocking_violations": 0,
        "mean_cost_within_budget": True,
        "p95_latency_within_budget": True,
        "quality_gain_cost_approved": False,
        "stochastic_samples": 3,
        "screenwriter_signoffs": ["writer_1"],
    }


class ProfessionalGateTests(unittest.TestCase):
    def test_complete_passing_human_evidence_can_promote(self) -> None:
        decision = evaluate_release(evidence())
        self.assertEqual(decision["decision"], "promote")
        self.assertTrue(decision["human_gate_satisfied"])
        self.assertEqual(decision["metrics"]["pair_accuracy"], 1.0)
        self.assertEqual(decision["metrics"]["professional_agreement"], 1.0)

    def test_missing_human_evidence_is_non_promotable(self) -> None:
        value = evidence()
        value["pair_reviews"] = []
        value["screenwriter_signoffs"] = []
        decision = evaluate_release(value)
        self.assertEqual(decision["decision"], "non_promotable")
        self.assertFalse(decision["human_gate_satisfied"])
        self.assertIn("professional_pair_reviews_missing", decision["reasons"])

    def test_complete_human_evidence_that_loses_is_rejected(self) -> None:
        decision = evaluate_release(evidence("incumbent"))
        self.assertEqual(decision["decision"], "reject")
        self.assertIn(
            "holdout_preference_lcb_not_above_half", decision["reasons"]
        )

    def test_critical_disagreement_requires_adjudication(self) -> None:
        value = evidence()
        value["pair_reviews"][0]["reviews"][0]["dimensions"][
            "human_credibility"
        ] = 1
        decision = evaluate_release(value)
        self.assertEqual(decision["decision"], "non_promotable")
        self.assertEqual(decision["metrics"]["adjudications_required"], 1)
        self.assertEqual(decision["metrics"]["adjudications_complete"], 0)


if __name__ == "__main__":
    unittest.main()
