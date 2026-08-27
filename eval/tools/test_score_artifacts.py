"""Tests for the pointwise judge scoring pipeline (REQ-320)."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from run_stage0_probe import load_rubric
from score_artifacts import (
    build_pointwise_system,
    build_pointwise_user_prompt,
    input_fingerprint_pointwise,
    load_baseline_targets,
    pointwise_score_record,
    rewrite_scores_jsonl,
    saved_pointwise_is_reusable,
    score_case,
    validate_pointwise,
)

ROOT = Path(__file__).parents[2]
BASELINE_PACKAGE = (
    ROOT / "eval" / "baselines" / "baseline-deepseek-v4-pro-20260727"
    / "comedy_002.story-package.json"
)
DIMENSIONS = load_rubric()
DIMENSION_IDS = [dimension["id"] for dimension in DIMENSIONS]


def pointwise_sample(scores: int | dict[str, int], spans: list[str]) -> dict:
    if isinstance(scores, int):
        scores = {dimension: scores for dimension in DIMENSION_IDS}
    return {
        dimension: {"score": score, "reason": "r", "spans": spans}
        for dimension, score in scores.items()
    }


def real_spans(artifact: dict) -> list[str]:
    from run_stage0_probe import valid_line_spans

    return sorted(valid_line_spans(artifact))[:2]


def fake_judge(route_provider: str = "fake_http") -> dict:
    return {
        "model": "qwen-test",
        "family": "qwen",
        "routes": [{"provider": route_provider, "endpoint": "https://x", "api_key_env": "X"}],
    }


def fake_case(tmp: Path, artifact: dict) -> dict:
    package = tmp / f"{artifact['case_id']}.story-package.json"
    package.write_text(json.dumps(artifact, ensure_ascii=False), encoding="utf-8")
    return {
        "case_id": artifact["case_id"],
        "artifact_id": f"artifact-{artifact['case_id']}",
        "package": package,
        "content_hash": "sha256:" + "0" * 64,
    }


class PromptTests(unittest.TestCase):
    def test_pointwise_system_names_every_dimension_and_no_pair(self) -> None:
        system = build_pointwise_system(DIMENSIONS)
        for dimension in DIMENSIONS:
            self.assertIn(dimension["id"], system)
        self.assertNotIn("preferred", system)
        self.assertNotIn("artifact_A", system)

    def test_user_prompt_carries_one_artifact_and_span_whitelist(self) -> None:
        artifact = json.loads(BASELINE_PACKAGE.read_text(encoding="utf-8"))
        prompt = build_pointwise_user_prompt("comedy_002", artifact, None)
        payload = json.loads(prompt)
        self.assertEqual(payload["case_id"], "comedy_002")
        self.assertIn("artifact", payload)
        self.assertNotIn("artifact_B", payload)
        self.assertTrue(payload["valid_span_refs"])


class ValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.artifact = json.loads(BASELINE_PACKAGE.read_text(encoding="utf-8"))

    def test_full_dimension_set_passes(self) -> None:
        spans = real_spans(self.artifact)
        validate_pointwise(pointwise_sample(4, spans), self.artifact, DIMENSION_IDS)

    def test_missing_dimension_is_rejected(self) -> None:
        sample = pointwise_sample(4, real_spans(self.artifact))
        del sample[DIMENSION_IDS[0]]
        with self.assertRaises(ValueError):
            validate_pointwise(sample, self.artifact, DIMENSION_IDS)

    def test_span_outside_the_artifact_is_rejected(self) -> None:
        sample = pointwise_sample(4, ["story-package/scene-99/dialogue-1"])
        with self.assertRaises(ValueError):
            validate_pointwise(sample, self.artifact, DIMENSION_IDS)

    def test_out_of_range_score_is_rejected(self) -> None:
        sample = pointwise_sample(4, real_spans(self.artifact))
        sample[DIMENSION_IDS[0]]["score"] = 6
        with self.assertRaises(ValueError):
            validate_pointwise(sample, self.artifact, DIMENSION_IDS)


class FingerprintTests(unittest.TestCase):
    def test_fingerprint_changes_with_the_artifact(self) -> None:
        with TemporaryDirectory() as directory:
            tmp = Path(directory)
            first = tmp / "a.json"
            second = tmp / "b.json"
            artifact = json.loads(BASELINE_PACKAGE.read_text(encoding="utf-8"))
            first.write_text(json.dumps(artifact, ensure_ascii=False), encoding="utf-8")
            second.write_text(
                json.dumps(artifact, ensure_ascii=False) + " ", encoding="utf-8"
            )
            self.assertNotEqual(
                input_fingerprint_pointwise(first),
                input_fingerprint_pointwise(second),
            )

    def test_saved_result_without_fingerprint_is_not_reusable(self) -> None:
        judge = fake_judge()
        saved = {"summary": {"judge_model": "qwen-test", "route_provider": "fake_http",
                             "samples_per_artifact": 2}, "samples": [{}, {}]}
        self.assertFalse(
            saved_pointwise_is_reusable(saved, judge, "fake_http", 2, "sha256:input", None)
        )
        saved["summary"]["input_fingerprint"] = "sha256:input"
        self.assertTrue(
            saved_pointwise_is_reusable(saved, judge, "fake_http", 2, "sha256:input", None)
        )
        self.assertFalse(
            saved_pointwise_is_reusable(saved, judge, "fake_http", 3, "sha256:input", None)
        )


class ScoreCaseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.artifact = json.loads(BASELINE_PACKAGE.read_text(encoding="utf-8"))
        self.spans = real_spans(self.artifact)

    def _run(self, tmp: Path, samples: list[dict], **overrides) -> dict:
        case = fake_case(tmp, self.artifact)
        judge = fake_judge()
        defaults = dict(
            system="s",
            temperature=0.7,
            samples_per_artifact=len(samples),
            retry_limit=1,
            dimension_ids=DIMENSION_IDS,
            run_dir=tmp,
            force=False,
            expected_fingerprint=input_fingerprint_pointwise(case["package"]),
        )
        defaults.update(overrides)
        calls: list[str] = []

        def fake_request(route, model, system, api_key, first, second,
                         temperature, validation_error=None, user_prompt=None):
            calls.append(user_prompt)
            return samples[len(calls) - 1] if len(calls) <= len(samples) else samples[-1]

        with patch.dict("os.environ", {"X": "test-key"}), patch(
            "score_artifacts.request", side_effect=fake_request
        ):
            summary = score_case(judge, case, self.artifact, **defaults)
        summary["_calls"] = len(calls)
        return summary

    def test_scoring_writes_summary_and_median(self) -> None:
        with TemporaryDirectory() as directory:
            tmp = Path(directory)
            samples = [
                pointwise_sample({d: 3 for d in DIMENSION_IDS}, self.spans),
                pointwise_sample({d: 5 for d in DIMENSION_IDS}, self.spans),
            ]
            summary = self._run(tmp, samples)
            self.assertEqual(summary["_calls"], 2)
            self.assertEqual(summary["median_scores"][DIMENSION_IDS[0]], 4)
            result = json.loads(
                (tmp / "judge-qwen-test.comedy_002.result.json").read_text("utf-8")
            )
            self.assertEqual(result["schema"], "pointwise-score-result/v1")
            self.assertEqual(len(result["samples"]), 2)
            self.assertFalse((tmp / "judge-qwen-test.comedy_002.partial.json").exists())

    def test_complete_saved_result_is_reused_without_calls(self) -> None:
        with TemporaryDirectory() as directory:
            tmp = Path(directory)
            samples = [pointwise_sample(4, self.spans), pointwise_sample(4, self.spans)]
            self._run(tmp, samples)
            summary = self._run(tmp, samples)
            self.assertEqual(summary["_calls"], 0)

    def test_changed_fingerprint_forces_rescoring(self) -> None:
        with TemporaryDirectory() as directory:
            tmp = Path(directory)
            samples = [pointwise_sample(4, self.spans), pointwise_sample(4, self.spans)]
            self._run(tmp, samples)
            summary = self._run(tmp, samples, expected_fingerprint="sha256:changed")
            self.assertEqual(summary["_calls"], 2)
            self.assertEqual(summary["input_fingerprint"], "sha256:changed")

    def test_partial_result_resumes_instead_of_restarting(self) -> None:
        with TemporaryDirectory() as directory:
            tmp = Path(directory)
            case = fake_case(tmp, self.artifact)
            judge = fake_judge()
            fingerprint = input_fingerprint_pointwise(case["package"])
            first = pointwise_sample(4, self.spans)
            from score_artifacts import atomic_write as write_partial

            write_partial(
                tmp / "judge-qwen-test.comedy_002.partial.json",
                {"input_fingerprint": fingerprint, "judge_config_fingerprint": None,
                 "samples": [first]},
            )
            second = pointwise_sample(5, self.spans)
            calls: list[int] = []

            def fake_request(*args, **kwargs):
                calls.append(1)
                return second

            with patch.dict("os.environ", {"X": "test-key"}), patch(
                "score_artifacts.request", side_effect=fake_request
            ):
                summary = score_case(
                    judge, case, self.artifact, system="s", temperature=0.7,
                    samples_per_artifact=2, retry_limit=1, dimension_ids=DIMENSION_IDS,
                    run_dir=tmp, force=False, expected_fingerprint=fingerprint,
                )
            self.assertEqual(len(calls), 1)
            self.assertEqual(summary["median_scores"][DIMENSION_IDS[0]], 4.5)

    def test_invalid_output_is_retried_then_accepted(self) -> None:
        with TemporaryDirectory() as directory:
            tmp = Path(directory)
            good = pointwise_sample(4, self.spans)
            bad = pointwise_sample(4, self.spans)
            del bad[DIMENSION_IDS[0]]
            case = fake_case(tmp, self.artifact)
            judge = fake_judge()
            responses = [bad, good]
            calls: list[str] = []

            def fake_request(*args, **kwargs):
                calls.append(kwargs.get("user_prompt") or args[8])
                return responses[len(calls) - 1]

            with patch.dict("os.environ", {"X": "test-key"}), patch(
                "score_artifacts.request", side_effect=fake_request
            ):
                summary = score_case(
                    judge, case, self.artifact, system="s", temperature=0.7,
                    samples_per_artifact=1, retry_limit=1, dimension_ids=DIMENSION_IDS,
                    run_dir=tmp, force=False,
                    expected_fingerprint=input_fingerprint_pointwise(case["package"]),
                )
            self.assertEqual(len(calls), 2)
            self.assertIn("未通过校验", calls[1])
            self.assertEqual(summary["median_scores"][DIMENSION_IDS[0]], 4)


class RecordTests(unittest.TestCase):
    def setUp(self) -> None:
        self.artifact = json.loads(BASELINE_PACKAGE.read_text(encoding="utf-8"))

    def test_record_is_eval_score_record_v1(self) -> None:
        sample = pointwise_sample(4, ["story-package/logline-1"])
        record = pointwise_score_record(
            "run-1", "comedy_002", "artifact-1", "sha256:" + "0" * 64,
            1234, "qwen-test", sample, 0, "judge-v1",
        )
        self.assertEqual(record["schema"], "eval-score-record/v1")
        self.assertEqual(record["rater"]["rater_type"], "llm_judge")
        self.assertEqual(record["rater"]["sample_index"], 0)
        self.assertEqual(record["admission"], {"passed": True, "failed_gates": []})
        self.assertEqual(
            sorted(d["dimension_id"] for d in record["dimensions"]),
            sorted(DIMENSION_IDS),
        )

    def test_rewrite_scores_jsonl_mirrors_result_files(self) -> None:
        with TemporaryDirectory() as directory:
            tmp = Path(directory)
            artifact = json.loads(BASELINE_PACKAGE.read_text(encoding="utf-8"))
            case = fake_case(tmp, artifact)
            sample = pointwise_sample(4, real_spans(self.artifact))
            from score_artifacts import atomic_write

            atomic_write(
                tmp / "judge-qwen-test.comedy_002.result.json",
                {
                    "schema": "pointwise-score-result/v1",
                    "case_id": "comedy_002",
                    "artifact_id": case["artifact_id"],
                    "samples": [sample, sample],
                    "summary": {"judge_model": "qwen-test"},
                },
            )
            count = rewrite_scores_jsonl(tmp, "run-1", "judge-v1",
                                         {"comedy_002": case})
            self.assertEqual(count, 2)
            lines = (tmp / "scores.jsonl").read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(lines), 2)
            record = json.loads(lines[0])
            self.assertEqual(record["schema"], "eval-score-record/v1")
            self.assertEqual(record["run_id"], "run-1")


class BaselineTargetTests(unittest.TestCase):
    def test_archived_baselines_are_discovered(self) -> None:
        targets = load_baseline_targets({"comedy_002"})
        self.assertEqual(len(targets), 1)
        self.assertEqual(targets[0]["case_id"], "comedy_002")
        self.assertTrue(targets[0]["package"].exists())
        self.assertTrue(targets[0]["content_hash"].startswith("sha256:"))

    def test_unknown_case_filter_fails_loudly(self) -> None:
        with self.assertRaises(SystemExit):
            load_baseline_targets({"no_such_case"})


if __name__ == "__main__":
    unittest.main()
