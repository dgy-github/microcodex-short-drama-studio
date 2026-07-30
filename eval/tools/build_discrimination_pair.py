"""Construct a candidate professional discrimination pair from licensed artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

import jsonschema

ROOT = Path(__file__).resolve().parents[2]


def artifact_id(value: dict[str, Any]) -> str:
    encoded = json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def construct_pair(
    positive: dict[str, Any],
    negative: dict[str, Any],
    *,
    pair_id: str,
    case_id: str,
    author_id: str,
    seeded_defects: list[dict[str, Any]],
    masking_virtue: list[str],
    split: str = "challenge",
) -> dict[str, Any]:
    schema = json.loads((ROOT / "schemas/story-package-v1.json").read_text("utf-8"))
    jsonschema.Draft202012Validator(schema).validate(positive)
    jsonschema.Draft202012Validator(schema).validate(negative)
    if not pair_id.strip() or not case_id.strip() or not author_id.strip():
        raise ValueError("pair, case and professional author identities are required")
    if positive.get("job_id") != negative.get("job_id"):
        raise ValueError("pair artifacts must share one job")
    positive_id, negative_id = artifact_id(positive), artifact_id(negative)
    if positive_id == negative_id or not seeded_defects:
        raise ValueError("pair members must differ and declare a defect key")
    positive_chars = len(json.dumps(positive, ensure_ascii=False))
    negative_chars = len(json.dumps(negative, ensure_ascii=False))
    char_delta = abs(negative_chars - positive_chars) / positive_chars
    episodes_match = len(positive["episodes"]) == len(negative["episodes"])
    if not episodes_match or negative_chars > positive_chars or char_delta > 0.10:
        raise ValueError("pair fails length or episode confound controls")
    pair = {
        "schema": "eval-adversarial-pair/v1",
        "pair_id": pair_id,
        "pair_kind": "discrimination",
        "case_id": case_id,
        "split": split,
        "positive_artifact_id": positive_id,
        "negative_artifact_id": negative_id,
        "construction": "authored",
        "author_id": author_id,
        "masking_virtue": masking_virtue,
        "seeded_defects": seeded_defects,
        "confound_controls": {
            "episodes_match": True,
            "char_count_delta_ratio": char_delta,
            "scene_count_match": len(positive["scenes"]) == len(negative["scenes"]),
            "negative_surface_metrics_at_or_above_median": False,
        },
        "admission_checks": "pass",
        "status": "candidate",
        "rights": {"allowed_uses": ["evaluation"]},
    }
    pair_schema = json.loads(
        (ROOT / "schemas/eval-adversarial-pair-v1.json").read_text("utf-8")
    )
    jsonschema.Draft202012Validator(pair_schema).validate(pair)
    return pair


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--positive", type=Path, required=True)
    parser.add_argument("--negative", type=Path, required=True)
    parser.add_argument("--pair-id", required=True)
    parser.add_argument("--case-id", required=True)
    parser.add_argument("--author-id", required=True)
    parser.add_argument("--defects", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError("pair output already exists")
    pair = construct_pair(
        json.loads(args.positive.read_text("utf-8")),
        json.loads(args.negative.read_text("utf-8")),
        pair_id=args.pair_id,
        case_id=args.case_id,
        author_id=args.author_id,
        seeded_defects=json.loads(args.defects.read_text("utf-8")),
        masking_virtue=[],
    )
    args.output.write_text(
        json.dumps(pair, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
