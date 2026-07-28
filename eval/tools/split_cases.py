"""Assign pilot cases to splits and enforce the leakage invariant.

Idempotent: reads every existing split file, re-assigns from the table below,
and rewrites all of them. Re-running after editing ASSIGNMENT is safe.

The invariant this script exists to enforce: cases sharing a `premise_family`
must land in the same split. Hand-editing split files eventually breaks that,
because the families are not visible from the prompt wording.

v1 note: `holdout` is deliberately left empty. It has no valid consumer without
a professional panel, and spending it on LLM judges would burn its blindness for
no return.

Usage (from the repository root):
    python eval/tools/split_cases.py            # rewrite split files
    python eval/tools/split_cases.py --check    # verify only, exit 1 on drift
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CASES = ROOT / "eval" / "cases"
SPLITS = ("dev", "train", "validation", "holdout", "challenge")

# Proportions follow the parent contract's dev:train:validation:challenge ratio
# of 30:30:24:12, scaled to the 30-case pilot. `holdout` is sealed in v1.
ASSIGNMENT: dict[str, tuple[str, ...]] = {
    "dev": (
        "family_001",
        "family_004",
        "urban_romance_002",
        "revenge_001",
        "revenge_002",
        "revenge_003",
        "revenge_004",
        "workplace_003",
        "rural_003",
        "comedy_002",
    ),
    "train": (
        "suspense_001",
        "suspense_003",
        "suspense_004",
        "urban_romance_003",
        "urban_romance_004",
        "workplace_002",
        "rural_002",
        "comedy_001",
        "historical_001",
    ),
    "validation": (
        "family_002",
        "family_003",
        "urban_romance_001",
        "suspense_002",
        "workplace_001",
        "rural_001",
        "comedy_003",
    ),
    "holdout": (),
    "challenge": (
        "cross_genre_001",
        "cross_genre_002",
        "family_005",
        "historical_002",
    ),
}

# The parent contract makes train visible to nanocodex for skill proposals.
# A case licensed only for `evaluation` cannot legally be used that way, so the
# right is granted here rather than being assumed at consumption time.
SPLIT_USES = {
    "dev": ["evaluation"],
    "train": ["evaluation", "skill_derivation"],
    "validation": ["evaluation"],
    "holdout": ["evaluation"],
    "challenge": ["evaluation"],
}


def load_all() -> list[dict]:
    cases: list[dict] = []
    for split in SPLITS:
        path = CASES / split / "cases.jsonl"
        if not path.exists():
            continue
        with path.open(encoding="utf-8") as handle:
            cases.extend(json.loads(line) for line in handle if line.strip())
    return cases


def assign(cases: list[dict]) -> dict[str, list[dict]]:
    by_id = {case["case_id"]: case for case in cases}
    planned = [case_id for ids in ASSIGNMENT.values() for case_id in ids]

    unknown = sorted(set(planned) - set(by_id))
    if unknown:
        raise SystemExit(f"assignment references unknown cases: {unknown}")
    unassigned = sorted(set(by_id) - set(planned))
    if unassigned:
        raise SystemExit(f"cases missing from the assignment table: {unassigned}")
    if len(planned) != len(set(planned)):
        raise SystemExit("a case is assigned to more than one split")

    result: dict[str, list[dict]] = {}
    for split, ids in ASSIGNMENT.items():
        bucket = []
        for case_id in ids:
            case = dict(by_id[case_id])
            case["split"] = split
            rights = dict(case["rights"])
            rights["allowed_uses"] = list(SPLIT_USES[split])
            case["rights"] = rights
            bucket.append(case)
        result[split] = bucket
    return result


def check_families(assigned: dict[str, list[dict]]) -> list[str]:
    homes: dict[str, set[str]] = {}
    for split, bucket in assigned.items():
        for case in bucket:
            homes.setdefault(case["premise_family"], set()).add(split)
    return [
        f"premise_family {family!r} is spread across {sorted(splits)}"
        for family, splits in sorted(homes.items())
        if len(splits) > 1
    ]


def write(assigned: dict[str, list[dict]]) -> None:
    for split, bucket in assigned.items():
        directory = CASES / split
        directory.mkdir(parents=True, exist_ok=True)
        path = directory / "cases.jsonl"
        lines = [
            json.dumps(case, ensure_ascii=False, separators=(",", ":"))
            for case in bucket
        ]
        path.write_text(
            "\n".join(lines) + ("\n" if lines else ""), encoding="utf-8"
        )


def main() -> int:
    cases = load_all()
    if not cases:
        raise SystemExit("no cases found under eval/cases")

    assigned = assign(cases)
    violations = check_families(assigned)
    if violations:
        for violation in violations:
            print(f"LEAKAGE: {violation}", file=sys.stderr)
        return 1

    if "--check" in sys.argv:
        for split, bucket in assigned.items():
            path = CASES / split / "cases.jsonl"
            existing = []
            if path.exists():
                with path.open(encoding="utf-8") as handle:
                    existing = [
                        json.loads(line) for line in handle if line.strip()
                    ]
            if existing != bucket:
                print(f"DRIFT: {split} differs from the assignment table")
                return 1
        print("split files match the assignment table")
        return 0

    write(assigned)
    total = sum(len(bucket) for bucket in assigned.values())
    print(f"wrote {total} cases")
    for split in SPLITS:
        bucket = assigned[split]
        uses = ",".join(SPLIT_USES[split])
        note = "  (sealed in v1)" if split == "holdout" else ""
        print(f"  {split:11} {len(bucket):>2}  uses={uses}{note}")
    families = {case["premise_family"] for bucket in assigned.values() for case in bucket}
    print(f"premise families: {len(families)} across {total} cases, none split")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
