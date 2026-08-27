"""Validate evaluation-case JSONL files with no third-party dependencies.

Two scopes, deliberately separated:

- `validate_records` checks one split file: field presence, types, and that
  every record agrees with the directory it sits in.
- `validate_corpus` checks the union of all splits: genre quota, difficulty
  coverage, hard-slice markers, licence uniqueness, and premise-family
  integrity.

The second scope is not optional. Genre quota and difficulty coverage are
properties of the case set as a whole and cannot hold inside a single split.
Premise-family leakage is by definition a cross-split property: checked one file
at a time, every family trivially has exactly one split and the check can never
fire.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

GENRES = {
    "family",
    "urban_romance",
    "revenge",
    "suspense",
    "workplace",
    "rural",
    "comedy",
    "historical",
    "cross_genre",
}
DIFFICULTIES = {"ordinary", "ambiguous", "hard"}
SPLITS = ("dev", "train", "validation", "holdout", "challenge")

# The 120-case target distribution from the parent contract (STORY_EVAL_DESIGN
# §3.1). Corpus-level: individual splits are far too small to carry it.
PILOT_QUOTAS = {
    "family": 20,
    "urban_romance": 16,
    "revenge": 16,
    "suspense": 16,
    "workplace": 12,
    "rural": 12,
    "comedy": 12,
    "historical": 8,
    "cross_genre": 8,
}

# Splits licensed for more than evaluation. The parent contract makes train
# visible to nanocodex for skill proposals.
SPLIT_USES = {
    "dev": {"evaluation"},
    "train": {"evaluation", "skill_derivation"},
    "validation": {"evaluation"},
    "holdout": {"evaluation"},
    "challenge": {"evaluation"},
}

REQUIRED_FIELDS = {
    "schema",
    "case_id",
    "split",
    "premise_family",
    "genre",
    "difficulty",
    "hard_slice",
    "input",
    "constraints",
    "required_elements",
    "required_conditions",
    "forbidden_elements",
    "rights",
}


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for line_number, raw_line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), 1
    ):
        if not raw_line.strip():
            continue
        try:
            record = json.loads(raw_line)
        except json.JSONDecodeError as error:
            raise ValueError(
                f"{path}:{line_number}: invalid JSON: {error.msg}"
            ) from error
        if not isinstance(record, dict):
            raise ValueError(f"{path}:{line_number}: record must be an object")
        records.append(record)
    return records


def load_corpus(root: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for split in SPLITS:
        path = root / split / "cases.jsonl"
        if path.exists():
            records.extend(load_jsonl(path))
    return records


def validate_records(
    records: list[dict[str, Any]], expected_split: str
) -> list[str]:
    """Per-file checks. Corpus-level invariants live in validate_corpus."""
    errors: list[str] = []
    seen: set[str] = set()

    for index, record in enumerate(records, 1):
        label = record.get("case_id", f"line {index}")
        missing = sorted(REQUIRED_FIELDS - record.keys())
        if missing:
            errors.append(f"{label}: missing fields: {', '.join(missing)}")
            continue

        case_id = record["case_id"]
        if case_id in seen:
            errors.append(f"{label}: duplicate case_id")
        seen.add(case_id)

        if record["schema"] != "eval-case/v1":
            errors.append(f"{label}: schema must be eval-case/v1")
        if record["split"] != expected_split:
            errors.append(f"{label}: split must be {expected_split}")
        if record["genre"] not in GENRES:
            errors.append(f"{label}: unknown genre {record['genre']!r}")
        if record["difficulty"] not in DIFFICULTIES:
            errors.append(f"{label}: unknown difficulty {record['difficulty']!r}")
        if not str(record["input"]).strip():
            errors.append(f"{label}: input must not be empty")
        if not str(record["premise_family"]).strip():
            errors.append(f"{label}: premise_family must not be empty")
        if not isinstance(record["required_elements"], list):
            errors.append(f"{label}: required_elements must be an array")
        if not isinstance(record["required_conditions"], list):
            errors.append(f"{label}: required_conditions must be an array")

        constraints = record["constraints"]
        if not isinstance(constraints, dict):
            errors.append(f"{label}: constraints must be an object")
        else:
            for field in (
                "episodes",
                "minutes_per_episode",
                "max_locations",
                "max_speaking_cast",
            ):
                if constraints.get(field) is None or constraints[field] <= 0:
                    errors.append(f"{label}: constraints.{field} must be positive")

        rights = record["rights"]
        if not isinstance(rights, dict):
            errors.append(f"{label}: rights must be an object")
        else:
            if not rights.get("license_id"):
                errors.append(f"{label}: rights.license_id is required")
            uses = set(rights.get("allowed_uses", []))
            if "evaluation" not in uses:
                errors.append(f"{label}: rights.allowed_uses must include evaluation")
            expected_uses = SPLIT_USES.get(expected_split)
            if expected_uses is not None and uses != expected_uses:
                errors.append(
                    f"{label}: rights.allowed_uses must be "
                    f"{sorted(expected_uses)} in {expected_split}, got {sorted(uses)}"
                )

    return errors


def validate_corpus(records: list[dict[str, Any]]) -> list[str]:
    """Invariants that only exist across the whole case set."""
    errors: list[str] = []

    ids = Counter(record.get("case_id") for record in records)
    for case_id, count in sorted(ids.items()):
        if count > 1:
            errors.append(f"corpus: case_id {case_id} appears {count} times")

    licences = Counter(
        record.get("rights", {}).get("license_id") for record in records
    )
    for licence, count in sorted(licences.items(), key=lambda item: str(item[0])):
        if count > 1:
            errors.append(f"corpus: license_id {licence} appears {count} times")

    homes: dict[str, set[str]] = defaultdict(set)
    for record in records:
        homes[record.get("premise_family")].add(record.get("split"))
    for family, splits in sorted(homes.items(), key=lambda item: str(item[0])):
        if len(splits) > 1:
            errors.append(
                f"corpus: premise_family {family!r} leaks across {sorted(splits)}"
            )

    counts = Counter(record.get("genre") for record in records)
    if len(records) != sum(PILOT_QUOTAS.values()):
        errors.append(
            f"corpus: expected {sum(PILOT_QUOTAS.values())} cases, got {len(records)}"
        )
    for genre, quota in PILOT_QUOTAS.items():
        if counts[genre] != quota:
            errors.append(f"corpus: {genre} expected {quota} cases, got {counts[genre]}")

    difficulties: dict[str, set[str]] = defaultdict(set)
    hard_slices: Counter = Counter()
    for record in records:
        difficulties[record.get("genre")].add(record.get("difficulty"))
        if record.get("hard_slice") is not None:
            hard_slices[record.get("genre")] += 1
    for genre, quota in PILOT_QUOTAS.items():
        # Genres with at least three pilot cases cover all three levels; the two
        # smallest slices cover two until they expand.
        required = DIFFICULTIES if quota >= 3 else {"ambiguous", "hard"}
        missing = sorted(required - difficulties[genre])
        if missing:
            errors.append(f"corpus: {genre} missing difficulties: {', '.join(missing)}")
        if hard_slices[genre] != 1:
            errors.append(
                f"corpus: {genre} expected exactly one hard_slice marker, "
                f"got {hard_slices[genre]}"
            )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "root",
        nargs="?",
        type=Path,
        default=Path(__file__).parents[1] / "cases",
        help="directory containing <split>/cases.jsonl",
    )
    args = parser.parse_args()

    errors: list[str] = []
    total = 0
    try:
        for split in SPLITS:
            path = args.root / split / "cases.jsonl"
            if not path.exists():
                continue
            records = load_jsonl(path)
            total += len(records)
            errors.extend(validate_records(records, split))
        errors.extend(validate_corpus(load_corpus(args.root)))
    except (OSError, ValueError) as error:
        print(f"ERROR: {error}")
        return 1

    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1

    print(f"OK: {total} valid cases across {args.root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
