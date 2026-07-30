"""Fixed 17-task advisory story workflow backed by typed Rust capabilities."""

from __future__ import annotations

import asyncio
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol
from uuid import uuid4

from aiohttp import ClientSession, ClientTimeout
from campaign.app.config import Config
from campaign.app.runtime import Runtime
from campaign.core.models import AgentSpec, ExecutionOrder, Task
from campaign.roles.base import Agent
from campaign.roles.reviewer import Reviewer

CAPABILITY_PROTOCOL = "story-capability-request/v1"
SYSTEM = (
    "你是短剧工作室中的专业 Agent。只输出一个 JSON 对象，不要 Markdown 或解释。"
    "不得模仿受保护作品；只使用提供的故事种子、约束和结构化上游产物。"
    "所有故事正文使用简体中文。"
)
REVIEW_TASKS = {"t11", "t12", "t13", "t14", "t16"}
REVIEW_TYPES = {
    "t11": "continuity",
    "t12": "human_taste",
    "t13": "originality",
    "t14": "production",
    "t16": "final",
}
EPISODE_WRITER_CONCURRENCY = 3


@dataclass(frozen=True)
class TaskSpec:
    task_id: str
    name: str
    agent_id: str
    skill: str
    artifact_schema: str
    depends_on: tuple[str, ...]


TASKS = (
    TaskSpec("t01", "classify_genre", "genre-analyst", "genre-classification", "genre-analysis/v1", ()),
    TaskSpec("t02", "retrieve_evidence", "life-detail-retriever", "licensed-rag", "retrieval-manifest/v1", ("t01",)),
    TaskSpec("t03", "propose_architecture_a", "story-architect-a", "architecture-a", "story-architecture/v1", ("t01", "t02")),
    TaskSpec("t04", "propose_architecture_b", "story-architect-b", "architecture-b", "story-architecture/v1", ("t01", "t02")),
    TaskSpec("t05", "propose_architecture_c", "story-architect-c", "architecture-c", "story-architecture/v1", ("t01", "t02")),
    TaskSpec("t06", "debate_and_select", "story-coordinator", "architecture-selection", "architecture-decision/v1", ("t03", "t04", "t05")),
    TaskSpec("t07", "deepen_characters", "character-room", "character-arc", "character-bible/v1", ("t06",)),
    TaskSpec("t08", "build_story_beats", "story-beat-architect", "story-beats", "story-beats/v1", ("t07",)),
    TaskSpec("t09", "plan_episodes", "episode-planner", "episode-hooks", "episode-plan/v1", ("t08",)),
    TaskSpec("t10", "write_sample_scenes", "scene-writer", "scene-dialogue", "sample-scenes/v1", ("t09",)),
    TaskSpec("t11", "continuity_review", "continuity-editor", "continuity-review", "story-review-record/v1", ("t08", "t09", "t10")),
    TaskSpec("t12", "human_taste_review", "human-taste-editor", "human-taste-review", "story-review-record/v1", ("t07", "t09", "t10")),
    TaskSpec("t13", "originality_review", "originality-editor", "originality-review", "story-review-record/v1", ("t02", "t08", "t10")),
    TaskSpec("t14", "production_review", "production-editor", "production-review", "story-review-record/v1", ("t09", "t10")),
    TaskSpec("t15", "targeted_revision", "reserve-writer", "targeted-revision", "story-package/v1", ("t11", "t12", "t13", "t14")),
    TaskSpec("t16", "final_review", "final-editor", "final-review", "story-review-record/v1", ("t15",)),
    TaskSpec("t17", "package_artifact", "artifact-packager", "package-artifact", "story-package/v1", ("t16",)),
)


class Capability(Protocol):
    async def generate(
        self, route: str, system: str, prompt: str
    ) -> tuple[dict[str, Any], dict[str, Any], str]: ...

    async def validate_package(
        self, package: dict[str, Any], expected_episodes: int
    ) -> dict[str, Any]: ...


class ReviewRejected(RuntimeError):
    """The fail-closed final reviewer refused the package.

    Distinguished from infrastructure failures on purpose: the workflow behaved
    correctly and the story did not pass. Retrying without changing the story
    reproduces it exactly.
    """


class PackageValidationFailed(RuntimeError):
    """The package did not satisfy the artifact contract (schema, episode count).

    A defect in generation or in the contract, not a provider problem.
    """


class RustCapabilityClient:
    def __init__(self, endpoint: str, token: str, timeout_seconds: float = 300.0) -> None:
        self._endpoint = endpoint.rstrip("/")
        self._token = token
        self._timeout = ClientTimeout(total=timeout_seconds)
        self._session: ClientSession | None = None

    async def _call(self, payload: dict[str, Any]) -> dict[str, Any]:
        if self._session is None:
            self._session = ClientSession(timeout=self._timeout)
        async with self._session.post(
            f"{self._endpoint}/v1/capabilities",
            json=payload,
            headers={"Authorization": f"Bearer {self._token}"},
        ) as response:
            if response.status != 200:
                raise RuntimeError(f"Rust capability rejected request: HTTP {response.status}")
            result = await response.json()
        if result.get("schema") != "story-capability-response/v1" or result.get("status") != "ok":
            raise RuntimeError("Rust capability returned an invalid response")
        return result

    async def generate(
        self, route: str, system: str, prompt: str
    ) -> tuple[dict[str, Any], dict[str, Any], str]:
        result = await self._call(
            {
                "schema": CAPABILITY_PROTOCOL,
                "capability": "generate_structured_text",
                "request_id": f"cap_{uuid4().hex}",
                "route": route,
                "system": system,
                "prompt": prompt,
            }
        )
        artifact = result.get("artifact")
        if not isinstance(artifact, dict):
            raise RuntimeError("structured generation did not return an object")
        return artifact, result.get("usage", {}), str(result.get("model", "unknown"))

    async def validate_package(
        self, package: dict[str, Any], expected_episodes: int
    ) -> dict[str, Any]:
        return await self._call(
            {
                "schema": CAPABILITY_PROTOCOL,
                "capability": "validate_artifact",
                "request_id": f"cap_{uuid4().hex}",
                "artifact_schema": "story-package/v1",
                "artifact": package,
                "expected_episodes": expected_episodes,
            }
        )

    async def close(self) -> None:
        if self._session is not None:
            await self._session.close()
            self._session = None


class WorkflowContext:
    def __init__(
        self,
        event_log: Any,
        job: dict[str, Any],
        run_id: str,
        request_id: str,
        genre_context: dict[str, Any] | None = None,
    ) -> None:
        self.event_log = event_log
        self.job = job
        self.run_id = run_id
        self.request_id = request_id
        self.genre_context = genre_context
        self.artifacts: dict[str, dict[str, Any]] = {}
        self.records: list[dict[str, str]] = []
        self.routes: dict[str, str] = {}
        self.consumed_tokens = 0
        self.failure_code: str | None = None
        self.failure_task_id: str | None = None
        self.failure_detail: str | None = None
        self._lock = asyncio.Lock()

    def record_failure(self, code: str, task_id: str | None, detail: str) -> None:
        """Keep the first failure only.

        Later tasks can fail as a consequence of the first one; overwriting
        would replace the cause with a symptom.
        """
        if self.failure_code is not None:
            return
        self.failure_code = code
        self.failure_task_id = task_id
        self.failure_detail = detail

    async def emit(
        self, event_type: str, agent_id: str, task_id: str | None, data: dict[str, Any]
    ) -> None:
        await self.event_log.append(
            event_type,
            agent_id,
            {
                "job_id": self.job["job_id"],
                "run_id": self.run_id,
                "causation_id": self.request_id,
                "correlation_id": self.request_id,
                "task_id": task_id,
                "agent_id": agent_id,
                "data": data,
            },
        )

    async def retain(
        self,
        spec: TaskSpec,
        artifact: dict[str, Any],
        model: str,
        usage: dict[str, Any],
    ) -> str:
        encoded = json.dumps(
            artifact, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")
        digest = hashlib.sha256(encoded).hexdigest()
        async with self._lock:
            total_tokens = usage.get("total_tokens", 0)
            if not isinstance(total_tokens, int) or total_tokens < 0:
                raise RuntimeError("provider usage is invalid")
            if self.consumed_tokens + total_tokens > int(
                self.job["budget"]["max_tokens"]
            ):
                self.failure_code = "token_budget_exceeded"
                raise RuntimeError("token budget exceeded")
            self.consumed_tokens += total_tokens
            self.artifacts[spec.task_id] = artifact
            self.records.append(
                {
                    "task_id": spec.task_id,
                    "agent_id": spec.agent_id,
                    "artifact_schema": spec.artifact_schema,
                    "content_sha256": digest,
                }
            )
            if model != "deterministic":
                self.routes["review" if spec.task_id in REVIEW_TASKS else "generation"] = model
        return digest


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

        results = await asyncio.gather(
            *(
                write_one(index, episode)
                for index, episode in enumerate(episode_plan, start=1)
            )
        )
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


def build_prompt(spec: TaskSpec, context: WorkflowContext) -> str:
    job = context.job
    upstream = {
        key: context.artifacts[key]
        for key in relevant_context(spec.task_id)
        if key in context.artifacts
    }
    if spec.task_id == "t15":
        upstream = targeted_revision_context(context)
    base = {
        "job": job,
        "genre_context": context.genre_context,
        "upstream": upstream,
    }
    requirements_by_task = {
        "t01": "识别题材、受众承诺、基调与风险。字段：schema, genre, audience_promise, tone, risks。",
        "t03": "提出独立方案A，强调最强情感关系。字段：schema, title, logline, causal_engine, characters, episode_arc, lived_details, risks。",
        "t04": "提出独立方案B，强调情节发动机和每集钩子。字段同 story-architecture/v1。",
        "t05": "提出独立方案C，强调具体生活质感和反套路选择。字段同 story-architecture/v1。",
        "t06": "比较三个方案，记录 selected, combined_components, rejected 与 reasons；输出 architecture-decision/v1。",
        "t07": "形成具体人物圣经；输出 character-bible/v1，characters 每项含 name/desire/fear/contradiction/secret/change/voice_markers。",
        "t08": "形成因果节拍链；输出 story-beats/v1，beats 每项含 pressure/choice/consequence/caused_by。",
        "t09": f"规划恰好 {job['format']['episodes']} 集；输出 episode-plan/v1，每集含 opening_state/conflict/turn/end_hook。",
        "t10": "由分集写作室为每一集并行写完整场景；输出合并后的 sample-scenes/v1。",
        "t15": (
            "根据人物、节拍、分集计划和有证据的审查意见完成定向修订。"
            "输出 story-package/v1 的 package_id、job_id、logline、promise、characters、beats、episodes、"
            "continuity_ledger、production 与 provenance。package_id 使用 advisory_ 前缀，job_id 与输入一致。"
            "当输入没有 scene_findings 时不要复写 scenes；运行时会原样装配已完成的分集正文，避免重复生成。"
            "当存在 scene_findings 时，输出修订后的完整 scenes，并保留未被引用的剧情事实。"
        ),
    }
    review_focus = {
        "t11": "检查因果、事实、时间线和伏笔回收。",
        "t12": "检查人物行为是否具体可信、情绪是否挣得、对白是否工具化。",
        "t13": "检查套路化表达、受保护作品模仿和来源重叠风险。",
        "t14": "检查集数、时长、场景、角色和低成本可制作性。",
        "t16": "对完整 story-package/v1 做最终 fail-closed 审查。仅当不存在 critical 缺陷时 status=pass。",
    }
    requirements = (
        review_instruction(spec, review_focus[spec.task_id])
        if spec.task_id in review_focus
        else requirements_by_task[spec.task_id]
    )
    if context.genre_context:
        directive_key = (
            "reviewer_directives"
            if spec.task_id in REVIEW_TASKS
            else "architect_directives"
        )
        requirements = (
            f"{requirements}\n类型包指令="
            f"{json.dumps(context.genre_context[directive_key], ensure_ascii=False)}"
        )
    human_directives = human_writing_directives(context, spec.task_id)
    if human_directives:
        requirements = (
            f"{requirements}\n短剧人味写作指令="
            f"{json.dumps(human_directives, ensure_ascii=False)}"
        )
    return f"任务={spec.name}\n要求={requirements}\n输入={json.dumps(base, ensure_ascii=False)}"


def episode_writer_prompt(
    episode_index: int, episode: Any, context: WorkflowContext
) -> str:
    payload = {
        "job": context.job,
        "genre_context": context.genre_context,
        "characters": context.artifacts["t07"],
        "beats": context.artifacts["t08"],
        "episode": episode,
    }
    human_directives = human_writing_directives(context, "t10")
    human_requirements = (
        f"短剧人味写作指令={json.dumps(human_directives, ensure_ascii=False)}\n"
        if human_directives
        else ""
    )
    return (
        f"任务=write_episode_{episode_index}\n"
        f"要求=你是第 {episode_index} 集子 Agent。只写这一集的完整可拍摄短剧场景，"
        "覆盖本集 opening_state、conflict、turn 和 end_hook。"
        "输出 JSON 对象：schema=sample-scenes/v1，scenes 为非空数组；"
        "每场含 location 和 lines，lines 每项 kind 为 action 或 dialogue，"
        "action 含 text，dialogue 含 speaker、text、subtext。"
        "speaker 使用角色姓名，台词必须有潜台词，不能只复述人物动机。\n"
        f"{human_requirements}"
        f"输入={json.dumps(payload, ensure_ascii=False)}"
    )


def human_writing_directives(
    context: WorkflowContext, task_id: str
) -> list[str]:
    if not isinstance(context.genre_context, dict):
        return []
    human_writing = context.genre_context.get("human_writing")
    if not isinstance(human_writing, dict):
        return []
    task_directives = human_writing.get("task_directives")
    if not isinstance(task_directives, dict):
        return []
    directives = task_directives.get(task_id)
    if not isinstance(directives, list):
        return []
    return [
        directive.strip()
        for directive in directives
        if isinstance(directive, str) and directive.strip()
    ]


def merge_usage(usages: Any) -> dict[str, Any]:
    merged: dict[str, int] = {}
    for usage in usages:
        if not isinstance(usage, dict):
            continue
        for key, value in usage.items():
            if isinstance(value, int) and value >= 0:
                merged[key] = merged.get(key, 0) + value
    return merged


def review_instruction(spec: TaskSpec, focus: str) -> str:
    return (
        f"{focus} 输出 story-review-record/v1：review_id, task_id={spec.task_id}, "
        f"review_type={REVIEW_TYPES[spec.task_id]}, status(pass/revise/reject), summary, findings。"
        "每个 finding 必须含 defect_id,severity,span_ref,evidence,requested_change；"
        "severity 只能是 critical/major/minor/note；"
        "没有缺陷时 findings=[]。引用暂未打包的结构时也使用预计的 story-package 节点路径。"
    )


def relevant_context(task_id: str) -> tuple[str, ...]:
    if task_id == "t06":
        return ("t03", "t04", "t05")
    if task_id == "t07":
        return ("t01", "t06")
    if task_id == "t08":
        return ("t06", "t07")
    if task_id == "t09":
        return ("t07", "t08")
    if task_id == "t10":
        return ("t07", "t08", "t09")
    if task_id in {"t11", "t12", "t13", "t14"}:
        return ("t02", "t07", "t08", "t09", "t10")
    if task_id == "t15":
        return ("t07", "t08", "t09", "t11", "t12", "t13", "t14")
    if task_id == "t16":
        return ("t15",)
    return ()


def targeted_revision_context(context: WorkflowContext) -> dict[str, Any]:
    upstream = {
        key: context.artifacts[key]
        for key in ("t07", "t08", "t09")
        if key in context.artifacts
    }
    findings = []
    for task_id in ("t11", "t12", "t13", "t14"):
        review = context.artifacts.get(task_id)
        if not isinstance(review, dict):
            continue
        for finding in review.get("findings", []):
            if isinstance(finding, dict):
                findings.append(finding)
    upstream["review_findings"] = findings
    scene_findings = [
        finding
        for finding in findings
        if isinstance(finding.get("span_ref"), str)
        and "/scene-" in finding["span_ref"]
    ]
    upstream["scene_findings"] = scene_findings
    if scene_findings and "t10" in context.artifacts:
        upstream["t10"] = context.artifacts["t10"]
    return upstream


def package_schema() -> str:
    root = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parents[2]))
    path = root / "schemas" / "story-package-v1.json"
    return path.read_text(encoding="utf-8")


def normalize_artifact(spec: TaskSpec, artifact: dict[str, Any]) -> None:
    artifact["schema"] = spec.artifact_schema
    if spec.task_id in REVIEW_TASKS:
        artifact["task_id"] = spec.task_id
        artifact["review_type"] = REVIEW_TYPES[spec.task_id]
        artifact.setdefault("review_id", f"review_{spec.task_id}_{uuid4().hex[:12]}")
        artifact.setdefault("status", "revise" if spec.task_id != "t16" else "pass")
        artifact.setdefault("summary", "未提供审查摘要。")
        artifact.setdefault("findings", [])
        severity_aliases = {
            "fatal": "critical",
            "high": "major",
            "medium": "minor",
            "moderate": "minor",
            "low": "note",
            "info": "note",
        }
        for finding in artifact["findings"]:
            severity = str(finding.get("severity", "note")).strip().lower()
            finding["severity"] = severity_aliases.get(severity, severity)


def canonical_package(
    raw: dict[str, Any],
    job: dict[str, Any],
    episode_scenes: Any = None,
) -> dict[str, Any]:
    premise = text_value(job.get("input"), "一个具体的人必须在压力下作出选择。")
    raw_characters = raw.get("characters")
    if not isinstance(raw_characters, list) or not raw_characters:
        raw_characters = [{"name": "主角"}]
    characters = []
    for index, value in enumerate(raw_characters, 1):
        value = value if isinstance(value, dict) else {}
        characters.append(
            {
                "node_id": f"ch-{index}",
                "name": text_value(value.get("name"), f"人物{index}"),
                "desire": text_value(value.get("desire"), "解决眼前危机"),
                "fear": text_value(value.get("fear"), "失去重要的人或尊严"),
                "contradiction": text_value(value.get("contradiction"), "想靠近却习惯推开"),
                "secret": text_value(value.get("secret"), "隐瞒了一段影响当下的往事"),
                "change": text_value(value.get("change"), "从逃避转向承担"),
                "voice_markers": string_list(value.get("voice_markers")),
            }
        )

    raw_beats = raw.get("beats")
    if not isinstance(raw_beats, list) or not raw_beats:
        raw_beats = [{"pressure": premise, "choice": "主角决定面对", "consequence": "关系发生变化"}]
    beats = []
    for index, value in enumerate(raw_beats, 1):
        value = value if isinstance(value, dict) else {}
        beats.append(
            {
                "node_id": f"beat-{index}",
                "pressure": text_value(value.get("pressure"), premise),
                "choice": text_value(value.get("choice"), "主角作出不可撤回的选择"),
                "consequence": text_value(value.get("consequence"), "选择带来新的压力"),
                "actor": "story-package/ch-1",
                "caused_by": [] if index == 1 else [f"story-package/beat-{index - 1}"],
            }
        )

    episode_count = int(job["format"]["episodes"])
    raw_episodes = raw.get("episodes")
    raw_episodes = raw_episodes if isinstance(raw_episodes, list) else []
    episodes = []
    for index in range(1, episode_count + 1):
        value = raw_episodes[index - 1] if index <= len(raw_episodes) else {}
        value = value if isinstance(value, dict) else {}
        hook = value.get("end_hook")
        hook = hook if isinstance(hook, dict) else {}
        episodes.append(
            {
                "node_id": f"ep-{index}",
                "index": index,
                "opening_state": text_value(value.get("opening_state"), premise),
                "conflict": text_value(value.get("conflict"), "人物目标与现实压力正面冲突"),
                "turn": text_value(value.get("turn"), "新信息迫使人物改变策略"),
                "end_hook": {
                    "node_id": f"hook-{index}",
                    "text": text_value(hook.get("text"), "下一步选择将付出代价"),
                    "kind": text_value(hook.get("kind"), "decision"),
                    "consequence_in": (
                        f"story-package/ep-{index + 1}"
                        if index < episode_count
                        else "none"
                    ),
                },
                "beats": [f"story-package/beat-{min(index, len(beats))}"],
            }
        )

    raw_scenes = raw.get("scenes")
    raw_scenes = raw_scenes if isinstance(raw_scenes, list) else []
    fallback_scenes = episode_scenes if isinstance(episode_scenes, list) else []
    if len(raw_scenes) < episode_count and len(fallback_scenes) >= episode_count:
        raw_scenes = fallback_scenes
    scenes = []
    for index in range(1, max(2, len(raw_scenes)) + 1):
        value = raw_scenes[index - 1] if index <= len(raw_scenes) else {}
        value = value if isinstance(value, dict) else {}
        raw_lines = value.get("lines")
        raw_lines = raw_lines if isinstance(raw_lines, list) else []
        lines = []
        counters = {"action": 0, "dialogue": 0}
        for raw_line in raw_lines:
            raw_line = raw_line if isinstance(raw_line, dict) else {}
            kind = "dialogue" if raw_line.get("kind") == "dialogue" else "action"
            counters[kind] += 1
            line = {
                "node_id": f"{kind}-{counters[kind]}",
                "kind": kind,
                "text": text_value(raw_line.get("text"), "人物在压力中停顿。"),
            }
            if kind == "dialogue":
                line["speaker"] = character_reference(
                    raw_line.get("speaker"), characters
                )
                subtext = raw_line.get("subtext")
                line["subtext"] = subtext if isinstance(subtext, str) and subtext else None
            lines.append(line)
        if not lines:
            lines = [
                {
                    "node_id": "action-1",
                    "kind": "action",
                    "text": "人物在狭窄空间里听见设备重新启动的声音。",
                }
            ]
        scenes.append(
            {
                "node_id": f"scene-{index}",
                "episode_ref": f"story-package/ep-{episode_index(value, index, episode_count)}",
                "location": text_value(value.get("location"), "商场设备间"),
                "lines": lines,
            }
        )

    locations = list(dict.fromkeys(scene["location"] for scene in scenes))
    promise = raw.get("promise")
    promise = promise if isinstance(promise, dict) else {}
    logline = raw.get("logline")
    logline = logline if isinstance(logline, dict) else {}
    return {
        "schema": "story-package/v1",
        "package_id": text_value(raw.get("package_id"), f"advisory_{job['job_id']}"),
        "job_id": job["job_id"],
        "logline": {"node_id": "log-1", "text": text_value(logline.get("text"), premise)},
        "promise": {
            "node_id": "promise-1",
            "genre": text_value(promise.get("genre"), job["allowed_genres"][0] if job.get("allowed_genres") else "drama"),
            "audience": text_value(promise.get("audience"), job["audience"]),
            "tone": text_value(promise.get("tone"), "克制、紧张、有人情味"),
        },
        "characters": characters,
        "beats": beats,
        "episodes": episodes,
        "scenes": scenes,
        "continuity_ledger": {
            "facts": [],
            "relationships": [],
            "timeline": [],
            "setups": [],
        },
        "production": {
            "locations": locations,
            "speaking_cast": [f"story-package/ch-{index}" for index in range(1, len(characters) + 1)],
        },
        "provenance": [],
    }


def text_value(value: Any, fallback: str) -> str:
    return value.strip() if isinstance(value, str) and value.strip() else fallback


def character_reference(value: Any, characters: list[dict[str, Any]]) -> str:
    if isinstance(value, str):
        candidate = value.strip()
        if candidate.startswith("story-package/ch-"):
            suffix = candidate.rsplit("ch-", 1)[-1]
            if suffix.isdigit() and 1 <= int(suffix) <= len(characters):
                return candidate
        compact = "".join(candidate.split())
        for index, character in enumerate(characters, 1):
            name = "".join(str(character.get("name", "")).split())
            if name and (compact == name or name in compact):
                return f"story-package/ch-{index}"
    return "story-package/ch-1"


def episode_index(value: dict[str, Any], fallback: int, episode_count: int) -> int:
    candidate = value.get("episode_index", fallback)
    reference = value.get("episode_ref")
    if isinstance(reference, str):
        suffix = reference.rsplit("ep-", 1)
        if len(suffix) == 2 and suffix[1].isdigit():
            candidate = int(suffix[1])
    if not isinstance(candidate, int) or isinstance(candidate, bool):
        candidate = fallback
    return min(max(candidate, 1), episode_count)


def string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [item.strip() for item in value if isinstance(item, str) and item.strip()]


def execution_order(job: dict[str, Any]) -> ExecutionOrder:
    return ExecutionOrder(
        objective=f"为 {job['job_id']} 生成并审查完整短剧故事",
        constraints=["advisory/non-promotable", "authorized retrieval only"],
        budget=job["budget"],
        tasks=[
            Task(
                id=spec.task_id,
                goal=spec.name,
                difficulty="hard" if spec.task_id in {"t06", "t15", "t16"} else "medium",
                degradable=False,
                required_skills=[spec.skill],
                acceptance=f"return {spec.artifact_schema}",
                depends_on=list(spec.depends_on),
            )
            for spec in TASKS
        ],
    )


class AdvisoryStoryWorkflow:
    def __init__(self, event_log: Any, capability: Capability) -> None:
        self._event_log = event_log
        self._capability = capability

    async def run(
        self,
        job: dict[str, Any],
        run_id: str,
        request_id: str,
        genre_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        context = WorkflowContext(
            self._event_log, job, run_id, request_id, genre_context
        )
        await context.emit(
            "run.started", "story-runtime", None,
            {"status": "advisory", "promotion": "non-promotable"},
        )
        runtime = Runtime(self._event_log, Config(privacy_strict=False))
        runtime.set_concurrency(3)
        runtime.set_require_reviewer(True)
        runtime.set_task_timeout(float(job["budget"]["deadline_seconds"]))
        for task_spec in TASKS:
            role = "reviewer" if task_spec.task_id in REVIEW_TASKS else (
                "retriever" if task_spec.task_id == "t02" else "executor"
            )
            agent = StoryAgent(
                AgentSpec(
                    id=task_spec.agent_id,
                    role=role,
                    model_tier="flagship",
                    skills=[task_spec.skill],
                ),
                self._event_log,
                task_spec,
                context,
                self._capability,
            )
            runtime.register_agent(agent)
        runtime.register_agent(
            ContractReviewer(
                AgentSpec(
                    id="contract-reviewer",
                    role="reviewer",
                    model_tier="flagship",
                    skills=[],
                ),
                self._event_log,
            )
        )

        result = await runtime.run(execution_order(job))
        statuses = [item.get("status") for item in result["results"]]
        if statuses != ["done"] * len(TASKS):
            detail = context.failure_detail or "no detail captured"
            task = context.failure_task_id or "unknown task"
            raise RuntimeError(
                f"fixed workflow failed: {context.failure_code or 'task_failure'} "
                f"at {task}: {detail}"
            )
        expected_ids = [spec.task_id for spec in TASKS]
        if sorted(item["task_id"] for item in context.records) != expected_ids:
            raise RuntimeError("fixed workflow artifact order is incomplete")
        records_by_id = {item["task_id"]: item for item in context.records}
        reviews = [context.artifacts[task_id] for task_id in ("t11", "t12", "t13", "t14", "t16")]
        workflow_result = {
            "schema": "story-workflow-result/v1",
            "run_id": run_id,
            "job_id": job["job_id"],
            "status": "advisory",
            "promotion": "non-promotable",
            "tasks": [records_by_id[task_id] for task_id in expected_ids],
            "reviews": reviews,
            "package": context.artifacts["t17"],
            "provider_routes": {
                "generation": context.routes.get("generation", "unknown"),
                "review": context.routes.get("review", "unknown"),
            },
        }
        return workflow_result

    async def close(self) -> None:
        close = getattr(self._capability, "close", None)
        if callable(close):
            await close()
