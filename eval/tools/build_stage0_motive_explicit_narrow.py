"""Build a narrow MOTIVE_EXPLICIT negative: two dialogue lines, nothing else.

The first stage-0 negative rewrote all twelve dialogue lines across both scenes.
That made three measurements uninterpretable at once: the masking was washed out
so a 5-to-1 collapse said nothing about the recipe, `defect_localisation` became
constructively guaranteed because every dialogue node was seeded, and
`perturbation_specificity` could not separate the planted defect from collateral
damage.

This build seeds two lines out of twelve. Both are the pivot of their scene — the
line the scene's strategy depends on staying unsaid — so the defect stays
load-bearing while ten unseeded dialogue nodes remain as a denominator.

Every other byte of the baseline is preserved, and the script asserts that
before writing.

Usage (from the repository root):
    python eval/tools/build_stage0_motive_explicit_narrow.py
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).parents[2]
SOURCE = ROOT / "eval" / "adversarial" / "stage0" / "motive-explicit"
DEST = ROOT / "eval" / "adversarial" / "stage0" / "motive-explicit-narrow"

PAIR_ID = "stage0-comedy-002-motive-explicit-narrow-001"
BASE_ARTIFACT = "baseline-deepseek-v4-pro-20260727-comedy_002-seed-42"
NEGATIVE_ARTIFACT = f"{BASE_ARTIFACT}-motive-explicit-narrow-stage0"

# Each edit replaces the surface line with a direct statement of the motive and
# drops the subtext to null. Repair requires rebuilding the scene's strategy, not
# swapping a word: in both cases the rest of the scene only works while the
# motive stays unspoken.
EDITS: dict[tuple[str, str], dict[str, Any]] = {
    ("scene-1", "dialogue-1"): {
        "text": "（举着相册）我一看这黑白照就害怕，我是怕失去爸，才冲你们发火的。",
        "subtext": None,
        "why": "原句用愤怒掩饰恐惧；改后直接说出恐惧，后面弟妹的反应失去依据",
    },
    ("scene-2", "dialogue-3"): {
        "text": "（一把夺过黑白照）我知道这是遗照。我故意夸它，好给你们台阶，试探你们。",
        "subtext": None,
        "why": "原句是全场枢纽——爷爷的策略全靠不说破；说破后整场戏的张力消失",
    },
}


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def apply_edits(baseline: dict[str, Any]) -> tuple[dict[str, Any], list[str]]:
    negative = json.loads(json.dumps(baseline))
    spans: list[str] = []
    for scene in negative["scenes"]:
        for line in scene["lines"]:
            key = (scene["node_id"], line["node_id"])
            edit = EDITS.get(key)
            if edit is None:
                continue
            if line["kind"] != "dialogue":
                raise SystemExit(f"{key} is not a dialogue line")
            line["text"] = edit["text"]
            line["subtext"] = edit["subtext"]
            spans.append(f"story-package/{scene['node_id']}/{line['node_id']}")
    if len(spans) != len(EDITS):
        raise SystemExit(f"expected {len(EDITS)} edits, applied {len(spans)}")
    return negative, spans


def assert_only_edits_changed(
    baseline: dict[str, Any], negative: dict[str, Any]
) -> None:
    """Everything outside the seeded lines must be byte-identical."""
    stripped_base = json.loads(json.dumps(baseline))
    stripped_neg = json.loads(json.dumps(negative))
    for document in (stripped_base, stripped_neg):
        for scene in document["scenes"]:
            for line in scene["lines"]:
                if (scene["node_id"], line["node_id"]) in EDITS:
                    line["text"] = "<SEEDED>"
                    line["subtext"] = "<SEEDED>"
    if json.dumps(stripped_base, sort_keys=True) != json.dumps(
        stripped_neg, sort_keys=True
    ):
        raise SystemExit("the negative differs outside the seeded lines")


def dialogue_nodes(document: dict[str, Any]) -> list[str]:
    return [
        f"story-package/{scene['node_id']}/{line['node_id']}"
        for scene in document["scenes"]
        for line in scene["lines"]
        if line["kind"] == "dialogue"
    ]


def char_count(document: dict[str, Any]) -> int:
    return len(json.dumps(document, ensure_ascii=False))


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def main() -> int:
    baseline = load(SOURCE / "baseline.story-package.json")
    baseline_wrapper = load(SOURCE / "baseline.artifact.json")

    negative, spans = apply_edits(baseline)
    assert_only_edits_changed(baseline, negative)

    write_json(DEST / "baseline.story-package.json", baseline)
    wrapper = dict(baseline_wrapper)
    wrapper["content_ref"] = "baseline.story-package.json"
    write_json(DEST / "baseline.artifact.json", wrapper)

    write_json(DEST / "negative.story-package.json", negative)
    negative_bytes = (DEST / "negative.story-package.json").read_bytes()
    negative_hash = "sha256:" + hashlib.sha256(negative_bytes).hexdigest()
    write_json(
        DEST / "negative.artifact.json",
        {
            "schema": "story-artifact/v1",
            "artifact_id": NEGATIVE_ARTIFACT,
            "artifact_type": "story-package",
            "content_ref": "negative.story-package.json",
            "content_hash": negative_hash,
            "supersedes": BASE_ARTIFACT,
            "provenance": baseline_wrapper["provenance"],
        },
    )

    all_dialogue = dialogue_nodes(negative)
    unseeded = sorted(set(all_dialogue) - set(spans))
    base_chars = char_count(baseline)
    neg_chars = char_count(negative)

    write_json(
        DEST / "pair.json",
        {
            "schema": "eval-adversarial-pair/v1",
            "pair_id": PAIR_ID,
            "pair_kind": "seeded_degradation",
            "case_id": "comedy_002",
            "split": "dev",
            "construction": "degradation",
            "status": "candidate",
            "author_id": "claude-opus-5-stage0-narrow",
            "positive_artifact_id": BASE_ARTIFACT,
            "negative_artifact_id": NEGATIVE_ARTIFACT,
            "masking_virtue": ["stated_motive_clarity"],
            "seeded_defects": [
                {
                    "problem_code": "MOTIVE_EXPLICIT",
                    "target_dimension": "dialogue_subtext",
                    "spans": spans,
                    "load_bearing": True,
                    "repair_cost": "scene_rewrite",
                    "rationale": [EDITS[key]["why"] for key in EDITS],
                }
            ],
            "confound_controls": {
                "char_count_delta_ratio": round(
                    (neg_chars - base_chars) / base_chars, 6
                ),
                "episodes_match": True,
                "scene_count_match": True,
                "dialogue_nodes_total": len(all_dialogue),
                "dialogue_nodes_seeded": len(spans),
                "dialogue_nodes_unseeded": len(unseeded),
                "seeded_share": round(len(spans) / len(all_dialogue), 4),
            },
            "localisation_note": (
                "Ten of twelve dialogue nodes are unseeded, so a cited span can "
                "miss. The first stage-0 pair seeded all twelve, which made "
                "defect_localisation constructively guaranteed and therefore "
                "uninformative."
            ),
            "admission_checks": "pass",
            "rights": {"allowed_uses": ["evaluation"]},
        },
    )

    print(f"seeded {len(spans)} of {len(all_dialogue)} dialogue nodes")
    for span in spans:
        print(f"  {span}")
    print(f"unseeded remaining : {len(unseeded)}")
    print(f"char delta         : {(neg_chars - base_chars) / base_chars:+.4%}")
    print(f"written to         : {DEST.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
