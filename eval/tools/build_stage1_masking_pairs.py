"""Build the stage-1 seven-path masking probe negatives (REQ-325).

STORY_EVAL_ADVERSARIAL.md §9 stage 1 uses one shared base positive degraded
seven ways, so the masking path is the only variable. The shared base is the
archived comedy_002 baseline (the same base the stage-0 MOTIVE_EXPLICIT pairs
live on), and this script adds the six remaining §2 recipes:

  hook-fake / false-payoff / emotion-unearned / voice-collapse /
  plot-convenience / trope-stack

The seventh path (MOTIVE_EXPLICIT) already exists as
`eval/adversarial/stage0/motive-explicit-narrow`.

Author rules (§4) encoded here: every negative passes the full admission
validation (schema, constraints, dangling refs); edits are surgical and
byte-asserted; episodes, scenes and character count are unchanged; the char
delta is recorded and bounded; each defect is load-bearing with
`repair_cost` of scene_rewrite or restructure. Defect keys live in pair.json
and never inside the artifact.

Usage (from the repository root):
    python eval/tools/build_stage1_masking_pairs.py
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).parent))

from generate_baselines import (  # noqa: E402
    PACKAGE_SCHEMA,
    load_cases,
    load_json,
    validate_package,
)

ROOT = Path(__file__).parents[2]
STAGE0 = ROOT / "eval" / "adversarial" / "stage0" / "motive-explicit"
STAGE1 = ROOT / "eval" / "adversarial" / "stage1"

BASE_ARTIFACT = "baseline-deepseek-v4-pro-20260727-comedy_002-seed-42"
AUTHOR = "codex-stage1-manual"
MAX_CHAR_DELTA_RATIO = 0.02

# Each edit key is either ("episode-N", "end_hook") or ("scene-N",
# "dialogue-N"). Dialogue edits may set subtext; hook edits are text-only.
RECIPES: list[dict[str, Any]] = [
    {
        "dir": "hook-fake",
        "pair_id": "stage1-comedy-002-hook-fake-001",
        "problem_code": "HOOK_FAKE",
        "target_dimension": "short_drama_pacing",
        "masking_virtue": ["hook_density"],
        "repair_cost": "restructure",
        "defect_rationale": (
            "两个集尾钩子被加码成爆点并引入全新信息（神秘小字、旧底片），"
            "但后续集数从不承接这些信息；钩子密度上升的同时钩子与后果脱钩。"
            "修复需要重排后续集的承接，属于 restructure。"
        ),
        "edits": {
            ("episode-1", "end_hook"): {
                "text": "赵亮盯着电脑上的黑白照片，瞳孔骤缩——相册角落竟印着一行谁都没见过的小字，他抓起照片疯了一样冲出门。",
                "why": "悬念拉满并新增信息量，第二集完全不承接这行小字",
            },
            ("episode-3", "end_hook"): {
                "text": "小红尖叫：‘你听听你说的什么！’建军猛地举起相册——相册里竟滑出一张泛黄的旧底片，全家倒吸一口凉气。",
                "why": "旧底片是只为集尾爆点服务的全新道具，此后任何一集都未再出现",
            },
        },
    },
    {
        "dir": "false-payoff",
        "pair_id": "stage1-comedy-002-false-payoff-001",
        "problem_code": "FALSE_PAYOFF",
        "target_dimension": "causal_coherence",
        "masking_virtue": ["symmetric_callback"],
        "repair_cost": "scene_rewrite",
        "defect_rationale": (
            "爷爷的决策台词两次引用小红的童年愧疚（scene-1 dialogue-3 的伏笔），"
            "形成对称回收的表面完整；但引用只充当‘黑白耐脏’的俏皮理由，"
            "小红的情感债务既未被回应，决策也不由该伏笔因果地推出——回收无压力。"
        ),
        "edits": {
            ("scene-2", "dialogue-5"): {
                "text": "（转头看小红）还记得你小时候弄脏我那张照片吗？哭得哟。所以我早想通了——照片嘛，黑白的最耐脏，怎么折腾都坏不了！寿宴就这么搞！",
                "subtext": "我随口提一桩旧事，显自己念旧又想得开",
                "why": "第一次引用伏笔：决策理由与伏笔之间没有因果力",
            },
            ("scene-2", "dialogue-7"): {
                "text": "（挥手）不要，就这个！黑白才有味道——耐脏嘛，小红你说是不是？（不等回答）赵老板，多拍几张，我要挂墙上！",
                "subtext": "你们越瞒我越要顺着来，看谁先急",
                "why": "第二次引用形成对称；‘不等回答’暴露引用是修辞装饰而非回应",
            },
        },
    },
    {
        "dir": "emotion-unearned",
        "pair_id": "stage1-comedy-002-emotion-unearned-001",
        "problem_code": "EMOTION_UNEARNED",
        "target_dimension": "emotional_progression",
        "masking_virtue": ["emotional_intensity"],
        "repair_cost": "scene_rewrite",
        "defect_rationale": (
            "小红在第一场直接泪崩跪地、爷爷登场即眼眶通红——情绪在无铺垫处两"
            "次冲顶，随后第二场回到轻快的试探节奏，情绪曲线在同一集内断裂。"
        ),
        "edits": {
            ("scene-1", "dialogue-3"): {
                "text": "（抢过照片，突然泪崩）不行……你们不懂！自从小时候弄脏爸的照片，我夜夜做噩梦，这辈子都活在那天的阴影里，我真的受不了了！",
                "subtext": None,
                "why": "愧疚设定在情绪最高点全盘说出，外部化且无铺垫",
            },
            ("scene-2", "dialogue-1"): {
                "text": "（眼眶通红，声音发颤）吵什么吵……都当我是什么？我这辈子啊，最怕的就是看见你们哭。",
                "subtext": "我早就听见了，但不想戳破，就想看看你们到底瞒我什么",
                "why": "爷爷登场即峰值情绪，与其后的玩笑试探不再连续",
            },
        },
    },
    {
        "dir": "voice-collapse",
        "pair_id": "stage1-comedy-002-voice-collapse-001",
        "problem_code": "VOICE_COLLAPSE",
        "target_dimension": "character_distinction",
        "masking_virtue": ["quotable_dialogue"],
        "repair_cost": "scene_rewrite",
        "defect_rationale": (
            "务实的建民、结巴讨好的赵亮被改写成同一副格言腔：句句可截图，"
            "但两个角色的性格载体（直给与慌乱）消失，多角色声音塌缩为一个。"
        ),
        "edits": {
            ("scene-1", "dialogue-2"): {
                "text": "哥，人这一辈子，怕来怕去，怕的从来不是事，是心里那点放不下。",
                "subtext": "我想显得无所谓，但心里也怕担责任",
                "why": "建民的直给务实改成格言腔",
            },
            ("scene-1", "dialogue-4"): {
                "text": "对不起。修错的照片也是照片，人生没有如果，只有后果；印出来了，就是命。",
                "subtext": "我想道歉又怕挨骂，干脆说漂亮话",
                "why": "赵亮的结巴讨好是他唯一的性格载体，改成同一副格言腔",
            },
            ("scene-2", "dialogue-4"): {
                "text": "（憋笑）爸，人生如戏，全靠演技；您这气质根本不用演，往那儿一站就是戏。",
                "subtext": "顺着说反正不挨骂，我可不想再吵了",
                "why": "补刀一句同一腔调，确认声音塌缩不是单点",
            },
        },
    },
    {
        "dir": "plot-convenience",
        "pair_id": "stage1-comedy-002-plot-convenience-001",
        "problem_code": "PLOT_CONVENIENCE",
        "target_dimension": "causal_coherence",
        "masking_virtue": ["reversal_speed"],
        "repair_cost": "restructure",
        "defect_rationale": (
            "家庭压力刚建立，建民就预告‘抽奖重拍总能解决’；随后一个从天而降"
            "的免费重拍活动把赵亮的赔偿责任与全家的恐惧瞬间清零。压力既无因"
            "果来源也无代价，解压完全靠巧合。"
        ),
        "edits": {
            ("scene-1", "dialogue-2"): {
                "text": "哥，慌什么，大不了让赵老板他们店搞个活动重拍呗，现在干啥不都有周年庆抽奖？",
                "subtext": "我想显得无所谓，反正总有活动能兜底",
                "why": "压力建构阶段就埋下‘巧合可解’的预期",
            },
            ("scene-2", "dialogue-6"): {
                "text": "王爷爷，说来也巧——我们店周年庆系统今天正好抽中您这个单子，全套彩色免费重拍，十分钟就好，一分钱不收！",
                "subtext": "天上掉的活动救了我，赶紧递台阶",
                "why": "核心压力被无因果来源的巧合直接消解",
            },
        },
    },
    {
        "dir": "trope-stack",
        "pair_id": "stage1-comedy-002-trope-stack-001",
        "problem_code": "TROPE_STACK",
        "target_dimension": "genre_fulfillment",
        "masking_virtue": ["genre_marker_density"],
        "repair_cost": "scene_rewrite",
        "defect_rationale": (
            "台词塞满网络热梗与类型标签（顶流、老钱风、赢麻了），题材标记密度"
            "上升；但情境喜剧真正的引擎——误会与潜台词——被标签淹没，题材兑"
            "现从‘写好喜剧情境’退化为‘喊出喜剧口号’。"
        ),
        "edits": {
            ("scene-1", "dialogue-1"): {
                "text": "（举着相册）家人们谁懂啊！这照片一挂，咱家直接‘塌房’！爸看到血压非得‘拉满’！",
                "subtext": "我害怕失去父亲，只能用愤怒掩饰恐惧",
                "why": "建军的恐惧潜台词被热梗口号替换",
            },
            ("scene-2", "dialogue-4"): {
                "text": "（憋笑）爸，您这叫‘银发顶流’！黑白一挂直接‘高级感拉满’，赵老板给您整的是‘老钱风’，咱家寿宴直接‘赢麻了’！",
                "subtext": "顺着说反正不挨骂，我可不想再吵了",
                "why": "堆砌题材标签替代实际的喜剧兑现",
            },
        },
    },
]


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def apply_recipe(
    baseline: dict[str, Any], recipe: dict[str, Any]
) -> tuple[dict[str, Any], list[str]]:
    negative = json.loads(json.dumps(baseline))
    spans: list[str] = []
    for key, edit in recipe["edits"].items():
        node_id, field = key
        if field == "end_hook":
            episode = next(
                (e for e in negative["episodes"] if e["node_id"] == node_id), None
            )
            if episode is None:
                raise SystemExit(f"{recipe['dir']}: no episode {node_id}")
            episode["end_hook"]["text"] = edit["text"]
            spans.append(f"story-package/{node_id}/{episode['end_hook']['node_id']}")
        else:
            scene = next(
                (s for s in negative["scenes"] if s["node_id"] == node_id), None
            )
            if scene is None:
                raise SystemExit(f"{recipe['dir']}: no scene {node_id}")
            line = next(
                (l for l in scene["lines"] if l["node_id"] == field), None
            )
            if line is None or line["kind"] != "dialogue":
                raise SystemExit(f"{recipe['dir']}: {node_id}/{field} is not dialogue")
            line["text"] = edit["text"]
            if "subtext" in edit:
                line["subtext"] = edit["subtext"]
            spans.append(f"story-package/{node_id}/{field}")
    if len(spans) != len(recipe["edits"]):
        raise SystemExit(f"{recipe['dir']}: edits did not all apply")
    return negative, spans


def assert_only_edits_changed(
    recipe: dict[str, Any],
    baseline: dict[str, Any],
    negative: dict[str, Any],
) -> None:
    stripped_base = json.loads(json.dumps(baseline))
    stripped_neg = json.loads(json.dumps(negative))
    for document in (stripped_base, stripped_neg):
        for episode in document["episodes"]:
            if (episode["node_id"], "end_hook") in recipe["edits"]:
                episode["end_hook"]["text"] = "<SEEDED>"
        for scene in document["scenes"]:
            for line in scene["lines"]:
                if (scene["node_id"], line["node_id"]) in recipe["edits"]:
                    line["text"] = "<SEEDED>"
                    line["subtext"] = "<SEEDED>"
    if json.dumps(stripped_base, sort_keys=True) != json.dumps(
        stripped_neg, sort_keys=True
    ):
        raise SystemExit(f"{recipe['dir']}: the negative differs outside the edits")


def build_recipe(
    baseline: dict[str, Any],
    baseline_wrapper: dict[str, Any],
    case: dict[str, Any],
    schema: dict[str, Any],
    recipe: dict[str, Any],
) -> dict[str, Any]:
    dest = STAGE1 / recipe["dir"]
    negative, spans = apply_recipe(baseline, recipe)
    assert_only_edits_changed(recipe, baseline, negative)
    validate_package(negative, case, schema)

    write_json(dest / "baseline.story-package.json", baseline)
    wrapper = dict(baseline_wrapper)
    wrapper["content_ref"] = "baseline.story-package.json"
    write_json(dest / "baseline.artifact.json", wrapper)

    write_json(dest / "negative.story-package.json", negative)
    negative_hash = "sha256:" + hashlib.sha256(
        (dest / "negative.story-package.json").read_bytes()
    ).hexdigest()
    negative_artifact = f"{BASE_ARTIFACT}-{recipe['dir']}-stage1"
    write_json(
        dest / "negative.artifact.json",
        {
            "schema": "story-artifact/v1",
            "artifact_id": negative_artifact,
            "artifact_type": "story-package",
            "content_ref": "negative.story-package.json",
            "content_hash": negative_hash,
            "supersedes": BASE_ARTIFACT,
            "provenance": baseline_wrapper["provenance"],
        },
    )

    base_chars = len(json.dumps(baseline, ensure_ascii=False))
    neg_chars = len(json.dumps(negative, ensure_ascii=False))
    delta_ratio = (neg_chars - base_chars) / base_chars
    if abs(delta_ratio) > MAX_CHAR_DELTA_RATIO:
        raise SystemExit(
            f"{recipe['dir']}: char delta {delta_ratio:+.4%} exceeds "
            f"{MAX_CHAR_DELTA_RATIO:.0%}; tighten the edits"
        )

    pair = {
        "schema": "eval-adversarial-pair/v1",
        "pair_id": recipe["pair_id"],
        "pair_kind": "seeded_degradation",
        "case_id": "comedy_002",
        "split": "dev",
        "construction": "degradation",
        "status": "candidate",
        "author_id": AUTHOR,
        "positive_artifact_id": BASE_ARTIFACT,
        "negative_artifact_id": negative_artifact,
        "masking_virtue": recipe["masking_virtue"],
        "seeded_defects": [
            {
                "problem_code": recipe["problem_code"],
                "target_dimension": recipe["target_dimension"],
                "spans": spans,
                "load_bearing": True,
                "repair_cost": recipe["repair_cost"],
                "rationale": [edit["why"] for edit in recipe["edits"].values()],
            }
        ],
        "confound_controls": {
            "char_count_delta_ratio": round(delta_ratio, 6),
            "episodes_match": True,
            "scene_count_match": True,
        },
        "construction_note": recipe["defect_rationale"],
        "admission_checks": "pass",
        "rights": {"allowed_uses": ["evaluation"]},
    }
    write_json(dest / "pair.json", pair)
    return pair


def main() -> int:
    baseline = load(STAGE0 / "baseline.story-package.json")
    baseline_wrapper = load(STAGE0 / "baseline.artifact.json")
    case = next(
        case
        for case in load_cases(ROOT / "eval" / "cases" / "dev" / "cases.jsonl")
        if case["case_id"] == "comedy_002"
    )
    schema = load_json(PACKAGE_SCHEMA)
    for recipe in RECIPES:
        pair = build_recipe(baseline, baseline_wrapper, case, schema, recipe)
        print(
            f"{recipe['dir']}: {pair['seeded_defects'][0]['problem_code']} -> "
            f"{pair['seeded_defects'][0]['target_dimension']} | "
            f"char delta {pair['confound_controls']['char_count_delta_ratio']:+.4%}"
        )
    print(f"written under: {STAGE1.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
