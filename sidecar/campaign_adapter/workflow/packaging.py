"""Artifact normalisation and canonical story-package assembly."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any
from uuid import uuid4

from .graph import REVIEW_TASKS, REVIEW_TYPES, TaskSpec


def merge_usage(usages: Any) -> dict[str, Any]:
    merged: dict[str, int] = {}
    for usage in usages:
        if not isinstance(usage, dict):
            continue
        for key, value in usage.items():
            if isinstance(value, int) and value >= 0:
                merged[key] = merged.get(key, 0) + value
    return merged


def package_schema() -> str:
    # parents: [0]=workflow, [1]=campaign_adapter, [2]=sidecar, [3]=repository
    root = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parents[3]))
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
    characters = _canonical_characters(raw.get("characters"))
    beats = _canonical_beats(raw.get("beats"), premise)
    episode_count = int(job["format"]["episodes"])
    episodes = _canonical_episodes(raw.get("episodes"), episode_count, beats, premise)
    scenes = _canonical_scenes(
        raw.get("scenes"), episode_scenes, episode_count, characters
    )
    promise = raw.get("promise")
    promise = promise if isinstance(promise, dict) else {}
    logline = raw.get("logline")
    logline = logline if isinstance(logline, dict) else {}
    return _assembled_package(
        raw, job, premise, promise, logline, characters, beats, episodes, scenes
    )


def _canonical_characters(raw_characters: Any) -> list[dict[str, Any]]:
    if not isinstance(raw_characters, list) or not raw_characters:
        raw_characters = [{"name": "主角"}]
    characters: list[dict[str, Any]] = []
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

    return characters


def _canonical_beats(raw_beats: Any, premise: str) -> list[dict[str, Any]]:
    if not isinstance(raw_beats, list) or not raw_beats:
        raw_beats = [{"pressure": premise, "choice": "主角决定面对", "consequence": "关系发生变化"}]
    beats: list[dict[str, Any]] = []
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

    return beats


def _canonical_episodes(
    raw_episodes: Any,
    episode_count: int,
    beats: list[dict[str, Any]],
    premise: str,
) -> list[dict[str, Any]]:
    raw_episodes = raw_episodes if isinstance(raw_episodes, list) else []
    episodes: list[dict[str, Any]] = []
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

    return episodes


def _canonical_scenes(
    raw_scenes: Any,
    episode_scenes: Any,
    episode_count: int,
    characters: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    raw_scenes = raw_scenes if isinstance(raw_scenes, list) else []
    fallback_scenes = episode_scenes if isinstance(episode_scenes, list) else []
    if len(raw_scenes) < episode_count and len(fallback_scenes) >= episode_count:
        raw_scenes = fallback_scenes
    scenes: list[dict[str, Any]] = []
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

    return scenes


def _assembled_package(
    raw: dict[str, Any],
    job: dict[str, Any],
    premise: str,
    promise: dict[str, Any],
    logline: dict[str, Any],
    characters: list[dict[str, Any]],
    beats: list[dict[str, Any]],
    episodes: list[dict[str, Any]],
    scenes: list[dict[str, Any]],
) -> dict[str, Any]:
    locations = list(dict.fromkeys(scene["location"] for scene in scenes))
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
