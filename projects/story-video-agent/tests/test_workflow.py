import json
import sys
import unittest
from pathlib import Path

import jsonschema

sys.path.insert(0, str(Path(__file__).parents[1]))

from story_video_agent import VideoPromptWorkflow


class VideoWorkflowTests(unittest.TestCase):
    def test_request_uses_immutable_image_and_story_spans(self):
        workflow = VideoPromptWorkflow("video-project-1")
        request = workflow.build_request(
            "artifact://sha256/" + "a" * 64,
            ["story-package/episodes/1/scenes/2"],
            {"motion": "横向跟拍", "action": "角色转身离开"},
        )
        self.assertEqual(request["schema"], "video-generation-request/v1")
        self.assertEqual(len(workflow.requests), 1)
        self.assertIn("横向跟拍", request["prompt"])
        schema_path = Path(__file__).parents[3] / "contracts/media-agent/video-generation-request-v1.json"
        jsonschema.Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8"))).validate(request)

    def test_untrusted_paths_and_missing_story_spans_fail_closed(self):
        workflow = VideoPromptWorkflow("video-project-1")
        with self.assertRaises(ValueError):
            workflow.build_request("C:/secret/image.png", ["scene:1"], {})
        with self.assertRaises(ValueError):
            workflow.build_request("artifact://sha256/" + "a" * 64, [], {})
