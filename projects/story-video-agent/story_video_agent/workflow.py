from __future__ import annotations

from typing import Any
from uuid import uuid4


class VideoPromptWorkflow:
    """Plan a video request without accessing files, providers or secrets."""

    def __init__(self, project_id: str) -> None:
        if not project_id.strip():
            raise ValueError("project_id must not be blank")
        self.project_id = project_id
        self._requests: list[dict[str, Any]] = []

    @property
    def requests(self) -> tuple[dict[str, Any], ...]:
        return tuple(self._requests)

    def build_request(
        self,
        image_artifact_ref: str,
        story_spans: list[str],
        shot: dict[str, Any],
    ) -> dict[str, Any]:
        self._validate_ref(image_artifact_ref)
        if not story_spans or any(not span.strip() for span in story_spans):
            raise ValueError("story_spans must contain at least one non-blank span")
        prompt = self._compose(shot)
        request = {
            "schema": "video-generation-request/v1",
            "project_id": self.project_id,
            "image_artifact_ref": image_artifact_ref,
            "story_spans": list(story_spans),
            "prompt": prompt,
            "request_id": f"vid_{uuid4().hex}",
        }
        self._requests.append(request)
        return dict(request)

    @staticmethod
    def _validate_ref(value: str) -> None:
        prefix = "artifact://sha256/"
        if not value.startswith(prefix) or len(value) != len(prefix) + 64:
            raise ValueError("image_artifact_ref must be a content-addressed artifact reference")
        if any(char not in "0123456789abcdef" for char in value[len(prefix):]):
            raise ValueError("image_artifact_ref digest must be lowercase hexadecimal")

    @staticmethod
    def _compose(shot: dict[str, Any]) -> str:
        motion = str(shot.get("motion", "缓慢推近"))
        duration = str(shot.get("duration_seconds", 5))
        action = str(shot.get("action", "人物完成一个有因果意义的动作"))
        return f"基于输入定帧；{motion}；时长 {duration} 秒；动作：{action}。"
