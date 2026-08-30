from __future__ import annotations

from dataclasses import dataclass
from hashlib import sha256
from typing import Any
from uuid import uuid4

from .capability import MediaProjectRepository


@dataclass(frozen=True)
class PromptRevision:
    revision_id: str
    project_id: str
    prompt: str
    parent_revision_id: str | None
    source_spans: tuple[str, ...]


class ImagePromptWorkflow:
    """Build and revise prompts; provider execution remains a Rust capability."""

    def __init__(
        self, project_id: str, repository: MediaProjectRepository | None = None
    ) -> None:
        if not project_id.strip():
            raise ValueError("project_id must not be blank")
        self.project_id = project_id
        self._repository = repository
        self._revisions: list[PromptRevision] = []
        self._requests: list[dict[str, Any]] = []

    @property
    def revisions(self) -> tuple[PromptRevision, ...]:
        return tuple(self._revisions)

    @property
    def requests(self) -> tuple[dict[str, Any], ...]:
        return tuple(self._requests)

    def create_prompt(self, scene: dict[str, Any], source_spans: list[str]) -> PromptRevision:
        prompt = self._compose(scene)
        return self._append(prompt, source_spans, None)

    def revise_prompt(self, revision_id: str, prompt: str) -> PromptRevision:
        parent = self._find(revision_id)
        if not prompt.strip():
            raise ValueError("prompt must not be blank")
        return self._append(prompt.strip(), list(parent.source_spans), parent.revision_id)

    def build_generation_request(self, revision_id: str) -> dict[str, Any]:
        revision = self._find(revision_id)
        request = {
            "schema": "image-generation-request/v1",
            "project_id": self.project_id,
            "prompt_revision_id": revision.revision_id,
            "prompt": revision.prompt,
            "source_spans": list(revision.source_spans),
            "request_id": f"img_{uuid4().hex}",
        }
        self._requests.append(request)
        if self._repository is not None:
            try:
                self._repository.append_generation_request(request)
            except Exception:
                self._requests.pop()
                raise
        return dict(request)

    def _append(self, prompt: str, spans: list[str], parent: str | None) -> PromptRevision:
        if not spans or any(not span.strip() for span in spans):
            raise ValueError("source_spans must contain at least one non-blank span")
        revision = PromptRevision(
            f"prompt_{sha256(f'{self.project_id}:{len(self._revisions)}'.encode()).hexdigest()[:16]}",
            self.project_id, prompt, parent, tuple(spans),
        )
        if self._repository is not None:
            self._repository.append_prompt_revision(
                {
                    "schema": "image-prompt-revision/v1",
                    "project_id": revision.project_id,
                    "revision_id": revision.revision_id,
                    "parent_revision_id": revision.parent_revision_id,
                    "prompt": revision.prompt,
                    "source_spans": list(revision.source_spans),
                }
            )
        self._revisions.append(revision)
        return revision

    def _find(self, revision_id: str) -> PromptRevision:
        for revision in self._revisions:
            if revision.revision_id == revision_id:
                return revision
        raise KeyError(f"unknown prompt revision: {revision_id}")

    @staticmethod
    def _compose(scene: dict[str, Any]) -> str:
        location = str(scene.get("location", "具体室内空间"))
        action = str(scene.get("action", "人物作出关键选择"))
        mood = str(scene.get("mood", "克制而有张力"))
        return f"竖屏短剧画面；地点：{location}；动作：{action}；氛围：{mood}。"
