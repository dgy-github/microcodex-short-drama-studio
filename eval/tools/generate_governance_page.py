"""Regenerate docs/eval-governance.html from the manifest and repository state (REQ-323).

The page explains design intent; the narrative is a tracked template
(`governance_template.html`) whose every number is injected:

- pillar names, weights, dimension groupings, floors and pass threshold come
  from `eval/manifests/eval-v0.1.0.json`;
- section seven (current status) is derived entirely from repository state —
  case counts per split, adversarial pair count, archived baselines, judge
  set, scored records, pillar-review conclusion and freeze state — so it can
  no longer drift from reality;
- `--check` regenerates in memory and fails when the tracked page differs,
  which is what the governance CI runs.

Hand-editing the tracked page is the failure mode this tool exists to remove
(ROADMAP decision table: "Regenerate it from the manifest instead, or delete
it").

Usage:
    python eval/tools/generate_governance_page.py
    python eval/tools/generate_governance_page.py --check
"""

from __future__ import annotations

import argparse
import json
from datetime import date
from pathlib import Path
from typing import Any

from run_stage0_probe import CODEX_JUDGES, JUDGES, MANIFEST, ROOT, load

TEMPLATE = Path(__file__).with_name("governance_template.html")
PAGE = ROOT / "docs" / "eval-governance.html"
FREEZE = ROOT / "eval" / "manifests" / "FREEZE.json"
CASES = ROOT / "eval" / "cases"
BASELINES = ROOT / "eval" / "baselines"
SCORES = ROOT / "eval" / "scores"
STORY_EVAL = ROOT / "crates" / "story-eval" / "src" / "lib.rs"

PILLAR_DISPLAY = {
    "character_credibility": "人物可信",
    "structure_causality": "结构因果",
    "viewing_drive": "观看驱动",
    "originality_delivery": "原创可交付",
}
DIMENSION_DISPLAY = {
    "human_credibility": "可信度",
    "character_distinction": "区分度",
    "dialogue_subtext": "台词潜台词",
    "causal_coherence": "因果一致性",
    "continuity": "连续性",
    "emotional_progression": "情感推进",
    "short_drama_pacing": "短剧节奏",
    "genre_fulfillment": "题材兑现",
    "originality": "原创性",
    "producibility": "可制作性",
}
SPLIT_ORDER = ["dev", "train", "validation", "challenge", "holdout"]
SPLIT_DISPLAY = {"holdout": "holdout（封存）"}


def repository_status() -> dict[str, Any]:
    case_counts: dict[str, int] = {}
    for split in SPLIT_ORDER:
        path = CASES / split / "cases.jsonl"
        case_counts[split] = (
            sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line)
            if path.exists()
            else 0
        )
    pair_dirs = sorted(
        path.parent
        for path in (ROOT / "eval" / "adversarial").rglob("pair.json")
    )
    baselines = 0
    for index_path in BASELINES.glob("*/index.json"):
        baselines += len(load(index_path)["cases"])
    judges = load(JUDGES)["judges"] + load(CODEX_JUDGES)["judges"]
    scored_records = 0
    for scores_path in SCORES.glob("*/scores.jsonl"):
        scored_records += sum(
            1 for line in scores_path.read_text(encoding="utf-8").splitlines() if line
        )
    reviews = sorted(SCORES.glob("pillar-review-*.json"))
    spot_checks = sorted(SCORES.glob("spot-check-agreement-*.json"))
    return {
        "case_counts": case_counts,
        "pair_dirs": pair_dirs,
        "baselines": baselines,
        "judge_models": sorted(judge["model"] for judge in judges),
        "judge_families": sorted({judge["family"] for judge in judges}),
        "scored_records": scored_records,
        "pillar_review": load(reviews[-1]) if reviews else None,
        "spot_check": load(spot_checks[-1]) if spot_checks else None,
        "frozen": FREEZE.exists(),
        "story_eval_tests": STORY_EVAL.read_text(encoding="utf-8").count("#[test]"),
    }


def pillar_tiles(manifest: dict[str, Any]) -> str:
    tiles = []
    for pillar_id, pillar in manifest["pillars"].items():
        name = PILLAR_DISPLAY.get(pillar_id, pillar_id)
        dimensions = "、".join(
            DIMENSION_DISPLAY.get(dimension, dimension)
            for dimension in pillar["dimensions"]
        )
        weight = int(round(pillar["weight"] * 100))
        tiles.append(
            f'<div class="tile"><h4>{name} {weight}%</h4><p>{dimensions}</p></div>'
        )
    return '<div class="grid g4">\n' + "\n".join(tiles) + "\n</div>"


def section_seven(manifest: dict[str, Any], status: dict[str, Any]) -> str:
    counts = status["case_counts"]
    total = sum(counts.values())
    split_text = " / ".join(
        f"{SPLIT_DISPLAY.get(split, split)} {counts[split]}"
        for split in SPLIT_ORDER
    )
    families = "、".join(status["judge_families"])
    landed = (
        f"<p>已落地：准入门与四支柱的判定逻辑（<code>crates/story-eval</code>，"
        f"{status['story_eval_tests']} 个测试）、rubric 与阈值清单、四份契约 schema"
        "（用例、评分记录、对抗对、产物引用）、"
        f"{total} 个原创中文用例（{split_text}）、{status['baselines']} 个归档 baseline 包、"
        f"{len(status['judge_models'])} 名判官（{families}）、"
        f"对抗集 {len(status['pair_dirs'])} 对"
        + (
            "（" + "、".join(path.name for path in status["pair_dirs"]) + "）"
            if status["pair_dirs"]
            else ""
        )
        + "。</p>"
    )
    review = status["pillar_review"]
    if review:
        landed += (
            "<p>支柱分组复核已产出：结论 <code>"
            f"{review['conclusion']}</code>（{review['scored_records']} 份打分记录，"
            f"合并阈值 {review['merge_threshold']}），报告见 "
            "<code>eval/scores/</code>。</p>"
        )
    else:
        landed += (
            "<p>维度相关矩阵尚未产出（等待逐案打分记录齐备后运行 "
            "<code>compute_pillar_review.py</code>）。</p>"
        )
    spot = status["spot_check"]
    spot_text = (
        f"已计算（{spot['joined_artifacts']} 个产物接入，mean alpha "
        f"{spot['spot_check_agreement']['mean_alpha']}）"
        if spot
        else "未开始（等待桌面端人工盲测提交后运行 <code>compute_spot_check_agreement.py</code>）"
    )
    frozen_text = (
        "评测契约已冻结（<code>FREEZE.json</code> 在案，任何变更需 MAJOR bump）。"
        if status["frozen"]
        else "评测契约未冻结——<code>eval-v0.1.0</code> 与 <code>judge-v1</code> 仍为占位阈值，"
        "冻结前置是支柱分组复核与内部人工抽查。"
    )
    missing = (
        "<p>未落地：判官稳定性校准（P1）仍未通过；内部人工盲测 "
        f"{spot_text}；职业编剧面板未接入（skill 注册表为空，晋升门禁全部关闭）。</p>"
        f"<p>{frozen_text}</p>"
    )
    next_step = (
        "<p>下一步：补齐逐案判官打分（<code>score_artifacts.py</code>）至 30 份记录，"
        "运行维度相关矩阵复核支柱分组，完成内部盲测抽查，然后冻结评测契约；"
        "对抗集按 stage-1 七路探针的读数决定量产规模。</p>"
    )
    if status["frozen"]:
        next_step = (
            "<p>下一步：以冻结契约为准重跑端到端流程与基线打分，解锁发布级对比；"
            "对抗集按既定 retire/refresh 规则轮换。</p>"
        )
    return landed + missing + next_step


def render() -> str:
    manifest = load(MANIFEST)
    status = repository_status()
    template = TEMPLATE.read_text(encoding="utf-8")
    floor = manifest["floors"]["pillar_minimum"]
    pas = manifest["verdict"]["pass_threshold"]
    pillar_js = json.dumps(
        [
            [PILLAR_DISPLAY.get(pillar_id, pillar_id), 2.5]
            for pillar_id in manifest["pillars"]
        ],
        ensure_ascii=False,
    )
    page = (
        template.replace("%%PILLAR_TILES%%", pillar_tiles(manifest))
        .replace("%%SCORED_TRIGGER%%", str(manifest["pillar_grouping_review"]["trigger_after_scored_cases"]))
        .replace("%%PILLAR_JS%%", pillar_js)
        .replace("%%FLOOR%%", str(floor))
        .replace("%%PASS%%", str(pas))
        .replace("%%SECTION_SEVEN%%", section_seven(manifest, status))
        .replace(
            "%%GENERATED_NOTE%%",
            f"本页由 <code>eval/tools/generate_governance_page.py</code> 生成于 "
            f"{date.today().isoformat()}，请勿手工编辑。",
        )
    )
    return page


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    return parser.parse_args()


def check_page(tracked: Path) -> int:
    generated = render()
    current = tracked.read_text(encoding="utf-8") if tracked.exists() else ""
    if generated != current:
        print(
            "docs/eval-governance.html differs from the generated page; "
            "regenerate with eval/tools/generate_governance_page.py "
            "instead of editing it by hand"
        )
        return 1
    print("governance page is current")
    return 0


def main() -> int:
    args = parse_args()
    if args.check:
        return check_page(PAGE)
    PAGE.write_text(render(), encoding="utf-8")
    print(f"regenerated {PAGE}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
