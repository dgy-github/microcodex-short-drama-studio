"""The per-task agent and the contract reviewer the runtime schedules."""

from __future__ import annotations

import asyncio
import json
from typing import Any

from campaign.core.models import AgentSpec, Task
from campaign.roles.base import Agent
from campaign.roles.reviewer import Reviewer

from .capability import Capability, PackageValidationFailed, ReviewRejected
from .context import WorkflowContext
from .graph import REVIEW_TASKS, TaskSpec
from .packaging import canonical_package, merge_usage, normalize_artifact
from .prompts import SYSTEM, build_prompt, episode_writer_prompt

EPISODE_WRITER_CONCURRENCY = 3


class ContractReviewer(Reviewer):
    async def handle(self, task: Task, artifact: dict[str, Any] | None = None) -> dict:
        output = artifact.get("output", "") if isinstance(artifact, dict) else ""
        try:
            parsed = json.loads(output)
            passed = isinstance(parsed, dict) and isinstance(parsed.get("schema"), str)
        except (json.JSONDecodeError, TypeError):
            passed = False
        return {
            "passed": passed,
            "score": 1.0 if passed else 0.0,
            "reasons": ["structured artifact present"] if passed else ["malformed artifact"],
            "task_id": task.id,
        }


class StoryAgent(Agent):
    def __init__(
        self, spec: AgentSpec, event_log: Any, task_spec: TaskSpec,
        context: WorkflowContext, capability: Capability
    ) -> None:
        super().__init__(spec, event_log)
        self._task_spec = task_spec
        self._context = context
        self._capability = capability

    async def _validate_package(self, package: dict[str, Any]) -> None:
        """Both t15 and t17 validate the package; both must classify alike.

        Wrapping only one call site left the other reporting a contract
        violation as a provider failure.
        """
        try:
            await self._capability.validate_package(
                package, int(self._context.job["format"]["episodes"])
            )
        except asyncio.TimeoutError:
            raise
        except Exception as exc:
            raise PackageValidationFailed(
                f"package rejected by validate_package: {exc}"
            ) from exc

    async def handle(self, task: Task) -> dict:
        spec = self._task_spec
        if spec.task_id != "t01":
            await self._context.emit(
                "task.queued", spec.agent_id, spec.task_id,
                {"task_name": spec.name, "depends_on": list(spec.depends_on)},
            )
        await self._context.emit(
            "task.started", spec.agent_id, spec.task_id, {"task_name": spec.name}
        )
        try:
            artifact, usage, model = await self._execute()
        except asyncio.TimeoutError as exc:
            self._context.record_failure(
                "capability_timeout", spec.task_id, "typed capability timed out"
            )
            raise RuntimeError("typed capability timed out") from exc
        except ReviewRejected as exc:
            # Not an infrastructure failure. The fail-closed reviewer refused
            # the package, which is the gate doing its job. Bucketing this with
            # provider errors tells the operator to retry when the correct
            # response is to fix the story.
            self._context.record_failure(
                "final_review_rejected", spec.task_id, str(exc)
            )
            raise
        except PackageValidationFailed as exc:
            self._context.record_failure(
                "artifact_validation_failed", spec.task_id, str(exc)
            )
            raise
        except Exception as exc:
            self._context.record_failure(
                "provider_or_task_failure", spec.task_id, f"{type(exc).__name__}: {exc}"
            )
            raise
        if artifact.get("schema") != spec.artifact_schema:
            raise RuntimeError(
                f"{spec.task_id} returned {artifact.get('schema')!r}, "
                f"expected {spec.artifact_schema!r}"
            )
        digest = await self._context.retain(spec, artifact, model, usage)
        if spec.task_id in REVIEW_TASKS:
            for finding in artifact.get("findings", []):
                await self._context.emit(
                    "review.finding", spec.agent_id, spec.task_id, finding
                )
            await self._context.emit(
                "review.completed",
                spec.agent_id,
                spec.task_id,
                {"status": artifact.get("status"), "content_sha256": digest},
            )
        await self._context.emit(
            "task.artifact.ready",
            spec.agent_id,
            spec.task_id,
            {"artifact_schema": spec.artifact_schema, "content_sha256": digest},
        )
        await self._context.emit(
            "task.completed",
            spec.agent_id,
            spec.task_id,
            {"content_sha256": digest, "usage": usage},
        )
        return {
            "task_id": spec.task_id,
            "output": json.dumps(artifact, ensure_ascii=False, separators=(",", ":")),
            "usage": usage,
        }

    async def _execute(self) -> tuple[dict[str, Any], dict[str, Any], str]:
        spec = self._task_spec
        if spec.task_id == "t02":
            sources = (
                self._context.genre_context.get("retrieval_sources", [])
                if self._context.genre_context
                else []
            )
            return (
                {
                    "schema": "retrieval-manifest/v1",
                    "policy": "authorized-only",
                    "sources": sources,
                    "note": (
                        "使用经 Rust 校验的类型包检索来源。"
                        if sources
                        else "本次 advisory 运行未使用外部检索材料。"
                    ),
                },
                {},
                "deterministic",
            )
        if spec.task_id == "t17":
            package = self._context.artifacts["t15"]
            final_review = self._context.artifacts["t16"]
            critical = [
                item
                for item in final_review.get("findings", [])
                if item.get("severity") == "critical"
            ]
            if final_review.get("status") != "pass" or critical:
                raise ReviewRejected(
                    f"final review status={final_review.get('status')!r}, "
                    f"critical findings={len(critical)}"
                )
            await self._validate_package(package)
            return package, {}, "deterministic"
        if spec.task_id == "t10":
            return await self._write_episodes()

        route = "review" if spec.task_id in REVIEW_TASKS else "generation"
        prompt = build_prompt(spec, self._context)
        artifact, usage, model = await self._capability.generate(route, SYSTEM, prompt)
        normalize_artifact(spec, artifact)
        if spec.task_id == "t15":
            artifact = canonical_package(
                artifact,
                self._context.job,
                self._context.artifacts["t10"].get("scenes"),
            )
            await self._validate_package(artifact)
        return artifact, usage, model

    async def _write_episodes(
        self,
    ) -> tuple[dict[str, Any], dict[str, Any], str]:
        episode_plan = self._context.artifacts["t09"].get("episodes")
        if not isinstance(episode_plan, list):
            raise RuntimeError("episode plan did not contain episodes")
        expected = int(self._context.job["format"]["episodes"])
        if len(episode_plan) != expected:
            raise RuntimeError("episode plan count does not match the story job")

        semaphore = asyncio.Semaphore(EPISODE_WRITER_CONCURRENCY)

        async def write_one(index: int, episode: Any) -> tuple[list[dict[str, Any]], dict[str, Any], str]:
            agent_id = f"episode-writer-{index:02}"
            await self._context.emit(
                "episode.started",
                agent_id,
                self._task_spec.task_id,
                {"episode_index": index, "episode_count": expected},
            )
            try:
                async with semaphore:
                    artifact, usage, model = await self._capability.generate(
                        "generation",
                        SYSTEM,
                        episode_writer_prompt(index, episode, self._context),
                    )
                scenes = artifact.get("scenes")
                if not isinstance(scenes, list) or not scenes:
                    raise RuntimeError(
                        f"episode writer {index} returned no scripted scenes"
                    )
                attributed = []
                for scene in scenes:
                    if not isinstance(scene, dict):
                        raise RuntimeError(
                            f"episode writer {index} returned an invalid scene"
                        )
                    attributed.append({**scene, "episode_index": index})
                await self._context.emit(
                    "episode.completed",
                    agent_id,
                    self._task_spec.task_id,
                    {
                        "episode_index": index,
                        "scene_count": len(attributed),
                        "usage": usage,
                    },
                )
                return attributed, usage, model
            except Exception as exc:
                await self._context.emit(
                    "episode.failed",
                    agent_id,
                    self._task_spec.task_id,
                    {"episode_index": index, "error": str(exc)},
                )
                raise

        # gather() hands the first exception to the caller but leaves the other
        # children running: on a provider failure the remaining episode writers
        # would keep spending flagship tokens on scenes nobody reads, and that
        # usage never reaches retain() to be counted against the budget. Cancel
        # the siblings and await them so the run stops paying the moment it has
        # decided to fail.
        writers = [
            asyncio.create_task(write_one(index, episode))
            for index, episode in enumerate(episode_plan, start=1)
        ]
        try:
            results = await asyncio.gather(*writers)
        except BaseException:
            for writer in writers:
                writer.cancel()
            await asyncio.gather(*writers, return_exceptions=True)
            raise
        scenes = [scene for child_scenes, _, _ in results for scene in child_scenes]
        usage = merge_usage(item[1] for item in results)
        models = sorted({item[2] for item in results})
        return (
            {
                "schema": "sample-scenes/v1",
                "mode": "parallel-episode-room",
                "episodes_completed": expected,
                "scenes": scenes,
            },
            usage,
            "+".join(models),
        )
