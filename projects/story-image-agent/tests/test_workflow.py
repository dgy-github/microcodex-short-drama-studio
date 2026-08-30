import json
import sys
import unittest
from pathlib import Path

import jsonschema

sys.path.insert(0, str(Path(__file__).parents[1]))

from story_image_agent import ImagePromptWorkflow


class ImageWorkflowTests(unittest.TestCase):
    def test_revision_and_regeneration_are_append_only(self):
        workflow = ImagePromptWorkflow("image-project-1")
        first = workflow.create_prompt({"location": "厨房", "action": "递出钥匙"}, ["scene:ep-1:sc-2"])
        second = workflow.revise_prompt(first.revision_id, "厨房夜景，母女隔桌对视，钥匙位于画面前景。")
        old_request = workflow.build_generation_request(first.revision_id)
        new_request = workflow.build_generation_request(second.revision_id)

        self.assertEqual(len(workflow.revisions), 2)
        self.assertEqual(len(workflow.requests), 2)
        self.assertNotEqual(old_request["request_id"], new_request["request_id"])
        self.assertEqual(second.parent_revision_id, first.revision_id)
        self.assertEqual(old_request["prompt_revision_id"], first.revision_id)
        self.assertEqual(new_request["prompt_revision_id"], second.revision_id)
        schema_path = Path(__file__).parents[3] / "contracts/media-agent/image-generation-request-v1.json"
        jsonschema.Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8"))).validate(new_request)

    def test_invalid_revision_and_source_fail_closed(self):
        workflow = ImagePromptWorkflow("image-project-1")
        with self.assertRaises(ValueError):
            workflow.create_prompt({}, [])
        with self.assertRaises(KeyError):
            workflow.build_generation_request("missing")

    def test_repository_receives_revision_before_request_and_failure_is_not_hidden(self):
        class RecordingRepository:
            def __init__(self):
                self.records = []
                self.fail_request = False

            def append_prompt_revision(self, revision):
                self.records.append(("revision", revision))

            def append_generation_request(self, request):
                if self.fail_request:
                    raise RuntimeError("durable store unavailable")
                self.records.append(("request", request))

        repository = RecordingRepository()
        workflow = ImagePromptWorkflow("image-project-1", repository)
        revision = workflow.create_prompt({"location": "厨房"}, ["scene:1"])
        workflow.build_generation_request(revision.revision_id)
        self.assertEqual([kind for kind, _ in repository.records], ["revision", "request"])
        repository.fail_request = True
        with self.assertRaisesRegex(RuntimeError, "durable store unavailable"):
            workflow.build_generation_request(revision.revision_id)
        self.assertEqual(len(workflow.requests), 1)
