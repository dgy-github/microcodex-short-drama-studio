"""Prompt construction for every task in the fixed graph."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

from .graph import REVIEW_TASKS, REVIEW_TYPES, TaskSpec, relevant_context

if TYPE_CHECKING:
    from .context import WorkflowContext

SYSTEM = (
    "你是短剧工作室中的专业 Agent。只输出一个 JSON 对象，不要 Markdown 或解释。"
    "不得模仿受保护作品；只使用提供的故事种子、约束和结构化上游产物。"
    "所有故事正文使用简体中文。"
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


def review_instruction(spec: TaskSpec, focus: str) -> str:
    return (
        f"{focus} 输出 story-review-record/v1：review_id, task_id={spec.task_id}, "
        f"review_type={REVIEW_TYPES[spec.task_id]}, status(pass/revise/reject), summary, findings。"
        "每个 finding 必须含 defect_id,severity,span_ref,evidence,requested_change；"
        "severity 只能是 critical/major/minor/note；"
        "没有缺陷时 findings=[]。引用暂未打包的结构时也使用预计的 story-package 节点路径。"
    )


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
