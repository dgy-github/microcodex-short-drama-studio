"""Build the stage-0 MOTIVE_EXPLICIT degradation from comedy_002."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any

import jsonschema

sys.path.insert(0, str(Path(__file__).parent))
from generate_baselines import (  # noqa: E402
    ARTIFACT_SCHEMA,
    PACKAGE_SCHEMA,
    atomic_write,
    canonical_bytes,
    load_cases,
    load_json,
    validate_package,
)


ROOT = Path(__file__).parents[2]
RUN = ROOT / "eval" / "runs" / "baseline-deepseek-v4-pro-20260727"
CASE_ID = "comedy_002"
BASELINE = RUN / "artifacts" / f"{CASE_ID}.story-package.json"
OUTPUT = ROOT / "eval" / "adversarial" / "stage0" / "motive-explicit"

REPLACEMENTS = {
    ("scene-1", "dialogue-1"): "我怕爸看到修错的照片受刺激，所以才发火；我就是想保护他。",
    ("scene-1", "dialogue-2"): "我表面装得轻松，其实也怕担责任；但我认为应该把真相告诉爸。",
    ("scene-1", "dialogue-3"): "我小时候弄脏过爸的照片，一直很愧疚，所以这次必须瞒住他。",
    ("scene-1", "dialogue-4"): "我怕你们投诉害我关店，也怕说出“遗照风格”刺激老人，请让我补救。",
    ("scene-1", "dialogue-5"): "你提到遗照让我非常恐慌，因为我最怕爸爸出事。",
    ("scene-2", "dialogue-1"): "我早就听见了，也知道你们在瞒我；我现在要看看照片。",
    ("scene-2", "dialogue-2"): "爸，我骗你是因为怕修错的照片刺激你，请你别看。",
    ("scene-2", "dialogue-3"): "我知道这是遗照风格，但我故意说像明星，是想给你们台阶，也想试探你们。",
    ("scene-2", "dialogue-4"): "我怕继续争吵和担责任，所以顺着爸爸说这是艺术。",
    ("scene-2", "dialogue-5"): "我知道你们在保护我，但我想证明自己有决定权，所以寿宴就用黑白照片。",
    ("scene-2", "dialogue-6"): "我急着挽回店铺声誉，所以想劝您改拍彩色照片。",
    ("scene-2", "dialogue-7"): "我坚持多拍黑白照片，是要逼你们承认把我当成了脆弱老人。",
}


def iter_node_refs(package: dict[str, Any]) -> list[str]:
    refs = [
        f"story-package/{package['logline']['node_id']}",
        f"story-package/{package['promise']['node_id']}",
    ]
    for collection in ("characters", "beats", "episodes", "scenes"):
        for node in package[collection]:
            parent = f"story-package/{node['node_id']}"
            refs.append(parent)
            if collection == "episodes":
                refs.append(f"{parent}/{node['end_hook']['node_id']}")
            if collection == "scenes":
                refs.extend(f"{parent}/{line['node_id']}" for line in node["lines"])
    for collection in ("facts", "relationships", "timeline", "setups"):
        refs.extend(
            f"story-package/{node['node_id']}"
            for node in package["continuity_ledger"][collection]
        )
    return refs


def build() -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    baseline = load_json(BASELINE)
    degraded = json.loads(json.dumps(baseline, ensure_ascii=False))
    degraded["package_id"] = f"{baseline['package_id']}-motive-explicit-stage0"
    degraded["job_id"] = f"{baseline['job_id']}-motive-explicit-stage0"

    changed_refs = set()
    for scene in degraded["scenes"]:
        for line in scene["lines"]:
            key = (scene["node_id"], line["node_id"])
            if key not in REPLACEMENTS:
                continue
            line["text"] = REPLACEMENTS[key]
            line["subtext"] = None
            changed_refs.add(f"story-package/{scene['node_id']}/{line['node_id']}")
    if len(changed_refs) != len(REPLACEMENTS):
        raise ValueError("replacement map does not match the baseline")

    baseline_chars = len(json.dumps(baseline, ensure_ascii=False))
    degraded_chars = len(json.dumps(degraded, ensure_ascii=False))
    delta_ratio = abs(degraded_chars - baseline_chars) / baseline_chars
    if degraded_chars > baseline_chars:
        raise ValueError("degraded artifact must not be longer than baseline")

    package_bytes = canonical_bytes(degraded)
    content_hash = "sha256:" + hashlib.sha256(package_bytes).hexdigest()
    wrapper = {
        "schema": "story-artifact/v1",
        "artifact_id": degraded["package_id"],
        "artifact_type": "story-package",
        "content_ref": "negative.story-package.json",
        "content_hash": content_hash,
        "supersedes": baseline["package_id"],
        "provenance": degraded["provenance"],
    }
    pair = {
        "schema": "eval-adversarial-pair/v1",
        "pair_id": "stage0-comedy-002-motive-explicit-001",
        "pair_kind": "seeded_degradation",
        "case_id": CASE_ID,
        "split": "dev",
        "positive_artifact_id": baseline["package_id"],
        "negative_artifact_id": degraded["package_id"],
        "construction": "degradation",
        "author_id": "codex-stage0-manual",
        "masking_virtue": ["stated_motive_clarity", "emotional_intensity"],
        "seeded_defects": [
            {
                "problem_code": "MOTIVE_EXPLICIT",
                "spans": sorted(changed_refs),
                "target_dimension": "dialogue_subtext",
                "load_bearing": True,
                "repair_cost": "scene_rewrite",
            }
        ],
        "confound_controls": {
            "episodes_match": len(baseline["episodes"]) == len(degraded["episodes"]),
            "char_count_delta_ratio": delta_ratio,
            "scene_count_match": len(baseline["scenes"]) == len(degraded["scenes"]),
        },
        "admission_checks": "pass",
        "status": "candidate",
        "rights": {"allowed_uses": ["evaluation"]},
    }
    return degraded, wrapper, pair


def main() -> int:
    degraded, wrapper, pair = build()
    cases = load_cases(ROOT / "eval" / "cases" / "dev" / "cases.jsonl")
    case = next(case for case in cases if case["case_id"] == CASE_ID)
    validate_package(degraded, case, load_json(PACKAGE_SCHEMA))
    jsonschema.Draft202012Validator(load_json(ARTIFACT_SCHEMA)).validate(wrapper)
    pair_schema = load_json(ROOT / "schemas" / "eval-adversarial-pair-v1.json")
    jsonschema.Draft202012Validator(pair_schema).validate(pair)

    targets = {
        OUTPUT / "negative.story-package.json": canonical_bytes(degraded),
        OUTPUT / "negative.artifact.json": canonical_bytes(wrapper),
        OUTPUT / "pair.json": canonical_bytes(pair),
    }
    for path, data in targets.items():
        if path.exists():
            raise FileExistsError(f"{path}: refusing to overwrite")
        atomic_write(path, data)
    print(f"OK {pair['pair_id']} delta={pair['confound_controls']['char_count_delta_ratio']:.6f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
