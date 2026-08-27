"""Enforce the frozen evaluation contract (REQ-324).

A freeze record (`eval/manifests/FREEZE.json`) pins the sha256 of the eval
manifest and the rubric together with links to the evidence that justified the
freeze. Writing the record is a human decision (VERSIONS.md records it); this
script is the mechanical backstop: once the record exists, any drift between
the pinned hashes and the tracked files fails the governance CI, so changing
the frozen set requires the documented MAJOR bump instead of a quiet edit.

Without a record the check passes — nothing is frozen yet.

Usage:
    python scripts/check_eval_freeze.py
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FREEZE_RECORD = ROOT / "eval" / "manifests" / "FREEZE.json"
REQUIRED_FIELDS = (
    "frozen_at",
    "eval_version",
    "rubric_version",
    "manifest_sha256",
    "rubric_sha256",
    "evidence",
)


def sha256_of(path: Path) -> str:
    return f"sha256:{hashlib.sha256(path.read_bytes()).hexdigest()}"


def check(root: Path = ROOT) -> int:
    record_path = root / "eval" / "manifests" / "FREEZE.json"
    if not record_path.exists():
        print("no freeze record; eval manifest and rubric remain editable")
        return 0
    try:
        record: dict[str, Any] = json.loads(
            record_path.read_text(encoding="utf-8")
        )
    except (json.JSONDecodeError, OSError) as error:
        print(f"FREEZE.json is unreadable: {error}")
        return 1
    missing = [field for field in REQUIRED_FIELDS if field not in record]
    if missing:
        print(f"FREEZE.json is missing fields: {missing}")
        return 1

    failures = []
    manifest_path = root / "eval" / "manifests" / f"{record['eval_version']}.json"
    rubric_path = root / "eval" / "rubrics" / f"{record['rubric_version']}.yaml"
    for path, pinned in (
        (manifest_path, record["manifest_sha256"]),
        (rubric_path, record["rubric_sha256"]),
    ):
        if not path.exists():
            failures.append(f"{path.relative_to(root)}: pinned file is missing")
        elif sha256_of(path) != pinned:
            failures.append(
                f"{path.relative_to(root)}: hash drifted from the pinned "
                f"{pinned}; a frozen contract changes only through a MAJOR bump "
                "(VERSIONS.md section 4)"
            )
    for name, evidence in record["evidence"].items():
        targets = evidence if isinstance(evidence, list) else [evidence]
        for target in targets:
            if not (root / target).exists():
                failures.append(f"evidence {name}: missing {target}")
    if failures:
        print("\n".join(failures))
        return 1
    print(
        f"eval contract frozen since {record['frozen_at']}: "
        f"{record['eval_version']} and {record['rubric_version']} match their pins"
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.parse_args(argv)
    return check()


if __name__ == "__main__":
    sys.exit(main())
