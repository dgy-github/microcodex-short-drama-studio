import json
import http.client
import re
import subprocess
import unittest
import urllib.error
from io import BytesIO
from pathlib import Path
from unittest.mock import MagicMock, patch

from run_stage0_probe import (
    CODEX_JUDGES,
    JUDGES,
    PAIR_DIR,
    build_probe_summary,
    build_system,
    consistency_metrics,
    input_fingerprint,
    judge_config_fingerprint,
    krippendorff_alpha_interval,
    load,
    load_probe_config,
    load_rubric,
    median_scores,
    normalize_owned_field_spans,
    resolve_route,
    request_codex,
    saved_result_is_reusable,
    self_consistency,
    specificity_metrics,
    urlopen_with_retry,
    valid_line_spans,
    validate_judgment,
)


def sample(scores: dict[str, int], spans: list[str], preferred: str = "A") -> dict:
    block = {
        dimension: {"score": score, "reason": "r", "spans": spans}
        for dimension, score in scores.items()
    }
    return {"A": block, "B": block, "preferred": preferred}


class RubricTests(unittest.TestCase):
    def test_rubric_has_ten_dimensions(self) -> None:
        self.assertEqual(len(load_rubric()), 10)

    def test_system_prompt_names_every_dimension(self) -> None:
        dimensions = load_rubric()
        system = build_system(dimensions)
        for dimension in dimensions:
            self.assertIn(dimension["id"], system)


class JudgeConfigTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = load(JUDGES)

    def test_generator_family_is_disjoint_from_judges(self) -> None:
        families = {j["family"] for j in self.config["judges"]}
        self.assertNotIn(self.config["generator"]["family"], families)

    def test_at_least_two_judge_families(self) -> None:
        families = {j["family"] for j in self.config["judges"]}
        self.assertGreaterEqual(len(families), 2)

    def test_supplemental_codex_is_a_third_disjoint_family(self) -> None:
        config = load_probe_config()
        families = {judge["family"] for judge in config["judges"]}
        self.assertEqual(families, {"qwen", "zhipu", "openai"})
        self.assertNotIn(config["generator"]["family"], families)

    def test_no_secret_material_in_tracked_config(self) -> None:
        """eval/judges.json is tracked; it must name env vars, never hold values."""
        raw = JUDGES.read_text(encoding="utf-8")
        for judge in self.config["judges"]:
            self.assertNotIn("api_key", judge)
            for route in judge["routes"]:
                self.assertIn("api_key_env", route)
                self.assertNotIn("api_key", route)
        self.assertNotIn("Bearer", raw)
        # nothing shaped like a live credential
        self.assertIsNone(re.search(r"[0-9a-f]{32}\.", raw))
        self.assertIsNone(
            re.search(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-", raw)
        )

    def test_temperature_permits_variance(self) -> None:
        """At temperature 0 the median of three samples is the sample, and
        self_consistency is trivially 1.0."""
        self.assertGreater(self.config["sampling"]["temperature"], 0)
        self.assertGreaterEqual(self.config["sampling"]["samples_per_artifact"], 3)


class ValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.baseline = load(PAIR_DIR / "baseline.story-package.json")
        self.negative = load(PAIR_DIR / "negative.story-package.json")
        self.ids = [d["id"] for d in load_rubric()]
        self.span = sorted(valid_line_spans(self.baseline))[:1]

    def test_full_dimension_set_passes(self) -> None:
        value = sample({d: 3 for d in self.ids}, self.span)
        validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_missing_dimension_is_rejected(self) -> None:
        value = sample({d: 3 for d in self.ids[:-1]}, self.span)
        with self.assertRaisesRegex(ValueError, "missing dimensions"):
            validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_span_outside_the_artifact_is_rejected(self) -> None:
        value = sample({d: 3 for d in self.ids}, ["story-package/scene-9/dialogue-9"])
        with self.assertRaisesRegex(ValueError, "invalid spans"):
            validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_real_beat_span_is_accepted(self) -> None:
        value = sample({d: 3 for d in self.ids}, ["story-package/beats-1"])
        validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_owned_field_span_normalizes_to_its_node(self) -> None:
        value = sample(
            {d: 3 for d in self.ids},
            ["story-package/scene-2/dialogue-1.subtext"],
        )
        normalize_owned_field_spans(value, self.baseline, self.baseline)
        self.assertEqual(
            value["A"][self.ids[0]]["spans"],
            ["story-package/scene-2/dialogue-1"],
        )
        validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_collection_fields_expand_to_member_nodes(self) -> None:
        artifact = load(PAIR_DIR / "baseline.story-package.json")
        value = {"A": {}, "B": {}}
        for label, source in (("A", artifact), ("B", artifact)):
            value[label] = {
                "producibility": {
                    "score": 3,
                    "reason": "r",
                    "spans": ["story-package/scenes"],
                }
            }
            break
        normalize_owned_field_spans(value, artifact, artifact)
        self.assertEqual(
            sorted(value["A"]["producibility"]["spans"]),
            sorted(f"story-package/{scene['node_id']}" for scene in artifact["scenes"]),
        )

    def test_production_fields_expand_to_real_source_nodes(self) -> None:
        value = sample(
            {d: 3 for d in self.ids},
            [
                "story-package/production/locations",
                "story-package/production/speaking_cast",
            ],
        )
        normalize_owned_field_spans(value, self.baseline, self.baseline)
        spans = value["A"][self.ids[0]]["spans"]
        self.assertIn("story-package/scene-1", spans)
        self.assertIn("story-package/char-1", spans)
        validate_judgment(value, self.baseline, self.baseline, self.ids)

    def test_out_of_range_score_is_rejected(self) -> None:
        scores = {d: 3 for d in self.ids}
        scores[self.ids[0]] = 9
        value = sample(scores, self.span)
        with self.assertRaisesRegex(ValueError, "score must be 1-5"):
            validate_judgment(value, self.baseline, self.baseline, self.ids)


class AggregationTests(unittest.TestCase):
    def test_median_ignores_a_single_outlier(self) -> None:
        ids = ["a"]
        samples = [
            sample({"a": 5}, ["x"]),
            sample({"a": 5}, ["x"]),
            sample({"a": 1}, ["x"]),
        ]
        self.assertEqual(median_scores(samples, "A", ids), {"a": 5})

    def test_self_consistency_detects_disagreement(self) -> None:
        ids = ["a", "b"]
        samples = [
            sample({"a": 5, "b": 3}, ["x"]),
            sample({"a": 5, "b": 4}, ["x"]),
        ]
        self.assertEqual(self_consistency(samples, "A", ids), 0.5)

    def test_consistency_metrics_cover_both_orders_and_sides(self) -> None:
        ids = ["a"]
        forward = [sample({"a": 5}, ["x"]), sample({"a": 5}, ["x"])]
        reverse = [sample({"a": 5}, ["x"]), sample({"a": 4}, ["x"])]
        metrics = consistency_metrics(forward, reverse, ids)
        self.assertEqual(
            set(metrics["self_consistency_by_order"]),
            {"forward_A", "forward_B", "reverse_A", "reverse_B"},
        )
        self.assertEqual(metrics["self_consistency"], 0.5)

    def test_specificity_reports_all_and_cross_pillar_views(self) -> None:
        dimensions = [
            {"id": "target", "pillar": "character"},
            {"id": "same", "pillar": "character"},
            {"id": "cross_drop", "pillar": "structure"},
            {"id": "cross_keep", "pillar": "delivery"},
        ]
        metrics = specificity_metrics(
            {dimension["id"]: 5.0 for dimension in dimensions},
            {
                "target": 4.0,
                "same": 4.0,
                "cross_drop": 4.0,
                "cross_keep": 5.0,
            },
            "target",
            dimensions,
        )
        self.assertAlmostEqual(metrics["specificity_all"], 1 / 3)
        self.assertEqual(metrics["specificity_cross_pillar"], 0.5)
        self.assertEqual(metrics["specificity"], metrics["specificity_all"])
        self.assertEqual(
            metrics["collateral_dimensions_all"], ["cross_drop", "same"]
        )
        self.assertEqual(
            metrics["collateral_dimensions_cross_pillar"], ["cross_drop"]
        )

    def test_specificity_rejects_target_without_a_pillar(self) -> None:
        with self.assertRaisesRegex(ValueError, "target dimension has no pillar"):
            specificity_metrics({"a": 5.0}, {"a": 4.0}, "missing", [])

    def test_inter_model_agreement_is_one_for_identical_raters(self) -> None:
        self.assertEqual(
            krippendorff_alpha_interval([[1, 2, 3], [1, 2, 3]]),
            1.0,
        )

    def test_probe_status_requires_specificity_and_stability(self) -> None:
        judge = {
            "judge_model": "j",
            "baseline_scores": {"a": 5.0},
            "negative_scores": {"a": 4.0},
            "sensitivity": True,
            "order_consistent": True,
            "specificity_all": 1.0,
            "specificity_cross_pillar": 0.8,
            "self_consistency": 0.7,
        }
        summary = build_probe_summary(
            {
                "generator": {"model": "g", "family": "generator"},
                "independence_caveat": "c",
            },
            {"pair_id": "p"},
            {"judge"},
            [judge, {**judge, "judge_model": "j2"}],
            {
                "min_specificity_cross_pillar": 0.7,
                "min_self_consistency": 0.8,
            },
            "sha256:x",
        )
        self.assertEqual(summary["status"], "probe_failed")


class PairSelfContainmentTests(unittest.TestCase):
    def test_both_members_live_in_the_pair_directory(self) -> None:
        """F4: the positive used to exist only under the gitignored eval/runs/,
        which made the pair unrebuildable elsewhere."""
        for name in ("baseline.story-package.json", "negative.story-package.json"):
            self.assertTrue((PAIR_DIR / name).exists(), name)

    def test_probe_does_not_read_the_ignored_run_directory(self) -> None:
        source = Path(__file__).with_name("run_stage0_probe.py").read_text(
            encoding="utf-8"
        )
        self.assertNotIn('"runs"', source)

    def test_saved_primary_and_narrow_results_have_both_specificity_views(self) -> None:
        pair_dirs = [
            PAIR_DIR,
            PAIR_DIR.with_name("motive-explicit-narrow"),
        ]
        for pair_dir in pair_dirs:
            summary = load(pair_dir / "probe-summary.json")
            self.assertIn("min_specificity_all", summary)
            self.assertIn("min_specificity_cross_pillar", summary)
            self.assertEqual(summary["min_specificity"], summary["min_specificity_all"])
            for judge in summary["judges"]:
                self.assertIn("specificity_all", judge)
                self.assertIn("specificity_cross_pillar", judge)
                self.assertEqual(judge["specificity"], judge["specificity_all"])

    def test_pair_inputs_have_a_stable_fingerprint(self) -> None:
        value = input_fingerprint(PAIR_DIR)
        self.assertRegex(value, r"^sha256:[0-9a-f]{64}$")

    def test_saved_result_without_fingerprint_is_not_reusable(self) -> None:
        saved = {
            "summary": {
                "judge_model": "m",
                "route_provider": "p",
                "samples_per_artifact": 1,
            },
            "forward": [{}],
            "reverse": [{}],
        }
        self.assertFalse(
            saved_result_is_reusable(
                saved,
                {"model": "m"},
                {"provider": "p"},
                1,
                "sha256:expected",
            )
        )




class RouteTests(unittest.TestCase):
    """Routes are alternate vendors for ONE model, not extra judges."""

    def setUp(self) -> None:
        self.config = load(JUDGES)

    def test_alternate_vendor_does_not_duplicate_a_model(self) -> None:
        glm = next(j for j in self.config["judges"] if j["family"] == "zhipu")
        self.assertGreater(len(glm["routes"]), 1)
        models = [j["model"] for j in self.config["judges"]]
        # one judge per MODEL: a model behind two vendors is one opinion, and
        # counting it twice would inflate min_judge_models and agreement
        self.assertEqual(len(models), len(set(models)))
        # distinct models of one family may coexist (e.g. a paused flagship and
        # a running flash tier); they add raters, not family diversity
        self.assertGreaterEqual(len(set(models)), 2)

    def test_every_judge_uses_the_routes_shape(self) -> None:
        for judge in self.config["judges"]:
            self.assertIn("routes", judge)
            for route in judge["routes"]:
                self.assertIn("provider", route)
                self.assertIn("api_key_env", route)

    def test_resolve_route_requires_endpoint_and_key(self) -> None:
        judge = {
            "model": "m",
            "routes": [
                {"provider": "a", "endpoint": None, "api_key_env": "NOPE_A"},
                {"provider": "b", "endpoint": "https://x", "api_key_env": "NOPE_B"},
            ],
        }
        with self.assertRaises(SystemExit):
            resolve_route(judge)

    def test_resolve_route_skips_a_blocked_route(self) -> None:
        judge = {
            "model": "m",
            "routes": [
                {
                    "provider": "blocked",
                    "endpoint": "https://blocked.example",
                    "api_key_env": "PATH",
                    "blocked_on": "account_balance",
                },
                {
                    "provider": "ready",
                    "endpoint": "https://ready.example",
                    "api_key_env": "PATH",
                    "blocked_on": None,
                },
            ],
        }
        self.assertEqual(resolve_route(judge)["provider"], "ready")

    def test_local_codex_route_does_not_require_an_api_key(self) -> None:
        judge = load(CODEX_JUDGES)["judges"][0]
        route = resolve_route(judge)
        self.assertEqual(route["provider"], "local_codex_exec")
        self.assertNotIn("api_key_env", route)

    @patch("run_stage0_probe.subprocess.run")
    def test_codex_exec_is_isolated_read_only_and_structured(
        self, run: MagicMock
    ) -> None:
        observed: dict = {}

        def complete(command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
            observed["command"] = command
            observed.update(kwargs)
            output = Path(
                command[command.index("--output-last-message") + 1]
            )
            output.write_text(
                '{"A":{},"B":{},"preferred":"tie"}',
                encoding="utf-8",
            )
            return subprocess.CompletedProcess(
                command,
                0,
                stdout='{"type":"turn.completed","usage":{"input_tokens":1}}\n',
                stderr="",
            )

        run.side_effect = complete
        result = request_codex(
            {
                "provider": "local_codex_exec",
                "command_path": "codex.exe",
                "output_schema": "schemas/stage0-judge-output-v1.json",
                "request_timeout_seconds": 30,
            },
            "gpt-test",
            "system",
            load(PAIR_DIR / "baseline.story-package.json"),
            load(PAIR_DIR / "negative.story-package.json"),
            None,
        )
        command = observed["command"]
        self.assertIn("--ephemeral", command)
        self.assertEqual(command[command.index("--sandbox") + 1], "read-only")
        self.assertEqual(command[command.index("--model") + 1], "gpt-test")
        self.assertIn("--output-schema", command)
        self.assertFalse(observed["shell"])
        self.assertFalse(Path(observed["cwd"]).is_relative_to(Path.cwd()))
        self.assertEqual(result["_provider_usage"], {"input_tokens": 1})

    def test_codex_saved_result_requires_exact_config_fingerprint(self) -> None:
        judge = load(CODEX_JUDGES)["judges"][0]
        fingerprint = judge_config_fingerprint(judge)
        saved = {
            "summary": {
                "judge_model": judge["model"],
                "route_provider": "local_codex_exec",
                "samples_per_artifact": 1,
                "input_fingerprint": "sha256:input",
                "judge_config_fingerprint": fingerprint,
            },
            "forward": [{}],
            "reverse": [{}],
        }
        route = {"provider": "local_codex_exec"}
        self.assertTrue(
            saved_result_is_reusable(
                saved, judge, route, 1, "sha256:input", fingerprint
            )
        )
        self.assertFalse(
            saved_result_is_reusable(
                saved, judge, route, 1, "sha256:input", "sha256:changed"
            )
        )

    @patch("run_stage0_probe.time.sleep")
    @patch("run_stage0_probe.https_exchange")
    def test_transient_429_is_retried(
        self, exchange: MagicMock, sleep: MagicMock
    ) -> None:
        throttled = urllib.error.HTTPError(
            "https://api.example.com/v1",
            429,
            "rate limited",
            {"Retry-After": "1"},
            BytesIO(b"{}"),
        )
        exchange.side_effect = [throttled, b'{"ok": true}']
        request = urllib.request.Request("https://api.example.com/v1")
        self.assertEqual(
            urlopen_with_retry(request, timeout=1), b'{"ok": true}'
        )
        sleep.assert_called_once_with(1.0)

    @patch("run_stage0_probe.time.sleep")
    @patch("run_stage0_probe.https_exchange")
    def test_remote_disconnect_is_retried(
        self, exchange: MagicMock, sleep: MagicMock
    ) -> None:
        exchange.side_effect = [
            http.client.RemoteDisconnected("closed"),
            b'{"ok": true}',
        ]
        request = urllib.request.Request("https://api.example.com/v1")
        self.assertEqual(
            urlopen_with_retry(request, timeout=1), b'{"ok": true}'
        )
        sleep.assert_called_once_with(2)


if __name__ == "__main__":
    unittest.main()
    input_fingerprint,
    krippendorff_alpha_interval,
