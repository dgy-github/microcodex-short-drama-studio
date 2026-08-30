import json
import tempfile
import unittest
from pathlib import Path

from generate_baselines import (
    ARTIFACT_SCHEMA,
    PACKAGE_SCHEMA,
    ProviderConfig,
    generate_one,
    load_cases,
    load_json,
    normalize_span_refs,
    normalize_node_ids,
    reconcile_scene_locations,
    canonicalize_scene_line_ids,
    select_cases,
)


CASES = Path(__file__).parents[1] / "cases" / "dev" / "cases.jsonl"


def valid_episodes(case: dict) -> list[dict]:
    episodes = []
    count = case["constraints"]["episodes"]
    for index in range(1, count + 1):
        episodes.append(
            {
                "node_id": f"episode-{index}",
                "index": index,
                "opening_state": "局面延续",
                "conflict": "角色必须作出选择",
                "turn": "选择产生新后果",
                "end_hook": {
                    "node_id": "hook-1",
                    "text": "后果进入下一集",
                    "kind": "consequence",
                    "consequence_in": (
                        f"story-package/episode-{index + 1}"
                        if index < count
                        else "none"
                    ),
                },
                "beats": ["story-package/beat-1"],
            }
        )
    return episodes


def valid_scenes() -> list[dict]:
    return [
        {
            "node_id": "scene-1", "episode_ref": "story-package/episode-1", "location": "室内",
            "lines": [{"node_id": "dialogue-1", "kind": "dialogue", "speaker": "story-package/character-1", "text": "这件事先别急。", "subtext": "害怕真相曝光"}],
        },
        {
            "node_id": "scene-2", "episode_ref": "story-package/episode-2", "location": "门外",
            "lines": [{"node_id": "action-1", "kind": "action", "text": "甲把文件放在门口。"}],
        },
    ]


def valid_package(case: dict) -> dict:
    return {
        "schema": "story-package/v1",
        "package_id": "model-value",
        "job_id": "model-value",
        "case_id": None,
        "logline": {
            "node_id": "logline-1",
            "text": "人物在压力中选择：" + "、".join(case["required_elements"]),
        },
        "promise": {
            "node_id": "promise-1",
            "genre": case["genre"],
            "audience": case["constraints"]["audience"],
            "tone": "克制",
        },
        "characters": [
            {
                "node_id": "character-1",
                "name": "甲",
                "desire": "守住承诺",
                "fear": "失去信任",
                "contradiction": "越想保护越不肯坦白",
                "secret": "曾隐瞒关键事实",
                "change": "学会承担后果",
            }
        ],
        "beats": [
            {
                "node_id": "beat-1",
                "pressure": "期限逼近",
                "choice": "公开事实",
                "consequence": "关系变化",
                "actor": "story-package/character-1",
                "caused_by": [],
            }
        ],
        "episodes": valid_episodes(case),
        "scenes": valid_scenes(),
        "continuity_ledger": {
            "facts": [],
            "relationships": [],
            "timeline": [],
            "setups": [],
        },
        "production": {
            "locations": ["室内", "门外"],
            "speaking_cast": ["story-package/character-1"],
        },
        "provenance": [],
    }


class GenerateBaselinesTests(unittest.TestCase):
    def setUp(self) -> None:
        self.case = load_cases(CASES)[0]
        self.package_schema = load_json(PACKAGE_SCHEMA)
        self.artifact_schema = load_json(ARTIFACT_SCHEMA)
        self.config = ProviderConfig(
            "https://unused.invalid", "test-model", "unused", 1, 0.7, 42
        )

    def test_generate_one_writes_content_and_wrapper(self) -> None:
        response = {
            "choices": [
                {"message": {"content": json.dumps(valid_package(self.case))}}
            ]
        }
        with tempfile.TemporaryDirectory() as directory:
            package_path, wrapper_path = generate_one(
                self.case,
                "baseline-test",
                Path(directory),
                self.package_schema,
                self.artifact_schema,
                self.config,
                requester=lambda _config, _prompt: response,
            )
            package = load_json(package_path)
            wrapper = load_json(wrapper_path)
            self.assertEqual(package["case_id"], self.case["case_id"])
            self.assertTrue(wrapper["content_hash"].startswith("sha256:"))
            self.assertEqual(
                wrapper["content_ref"],
                package_path.relative_to(Path(directory)).as_posix(),
            )

    def test_existing_artifact_is_not_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            run_dir = Path(directory)
            artifact = (
                run_dir
                / "artifacts"
                / f"{self.case['case_id']}.story-package.json"
            )
            artifact.parent.mkdir()
            artifact.write_text("existing", encoding="utf-8")
            with self.assertRaises(FileExistsError):
                generate_one(
                    self.case,
                    "baseline-test",
                    run_dir,
                    self.package_schema,
                    self.artifact_schema,
                    self.config,
                    requester=lambda _config, _prompt: {},
                )

    def test_unknown_case_selection_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "unknown case ids"):
            select_cases([self.case], ["missing_001"])

    def test_json_tree_span_is_canonicalized(self) -> None:
        value = {"actor": "story-package/characters/character-6"}
        self.assertEqual(
            normalize_span_refs(value)["actor"], "story-package/character-6"
        )

    def test_parent_prefix_is_removed_from_node_id(self) -> None:
        value = {"node_id": "scene-1/dialogue-2"}
        normalize_node_ids(value)
        self.assertEqual(value["node_id"], "dialogue-2")

        prefixed = {"node_id": "scene-1-action-1"}
        normalize_node_ids(prefixed)
        self.assertEqual(prefixed["node_id"], "action-1")

    def test_scene_location_refines_declared_location(self) -> None:
        value = {
            "production": {"locations": ["老房子"]},
            "scenes": [{"location": "老房子客厅"}],
        }
        reconcile_scene_locations(value)
        self.assertEqual(value["production"]["locations"], ["老房子客厅"])

    def test_scene_lines_are_numbered_by_kind(self) -> None:
        value = {
            "scenes": [
                {
                    "node_id": "scene-1",
                    "lines": [
                        {"node_id": "line-1-1", "kind": "dialogue"},
                        {"node_id": "line-1-2", "kind": "action"},
                    ],
                }
            ],
            "ref": "story-package/scene-1/line-1-1",
        }
        canonicalize_scene_line_ids(value)
        self.assertEqual(value["scenes"][0]["lines"][0]["node_id"], "dialogue-1")
        self.assertEqual(value["ref"], "story-package/scene-1/dialogue-1")


if __name__ == "__main__":
    unittest.main()
