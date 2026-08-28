"""Machine-check premise-family separation at the 120-case scale (P11).

The parent contract separates splits by premise family, and `split_cases.py`
enforces that cases sharing a *label* never separate. What no label-based
check can see is the opposite failure: two premises whose text is nearly
identical but which landed in different families — and therefore potentially
different splits. At 30 cases reading them sufficed; at 120 it does not
(ROADMAP P11: the check "becomes due before freezing").

Method: character 3-grams suit one-line Chinese premises. MinHash signatures
estimate Jaccard similarity between every case pair cheaply; every pair whose
estimated similarity clears the pre-filter is then verified with exact set
Jaccard so the report never flags on approximation error alone. Only
cross-family pairs above the threshold are violations — near-duplicates
inside one family are what a family means.

Usage:
    python eval/tools/check_premise_families.py            # write report, exit 1 on violations
    python eval/tools/check_premise_families.py --threshold 0.5
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import unicodedata
from itertools import combinations
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
CASES = ROOT / "eval" / "cases"
REPORT = ROOT / "eval" / "cases" / "premise-family-check.json"
DEFAULT_THRESHOLD = 0.5
SHINGLE_SIZE = 3
PERMUTATIONS = 128

NON_TEXT = re.compile(r"[\s，。！？、；：''\"\"（）()\[\]【】,.!?;:'\"-]+")
SPLITS = ("dev", "train", "validation", "holdout", "challenge")


def normalize(text: str) -> str:
    text = unicodedata.normalize("NFKC", text)
    return NON_TEXT.sub("", text)


def shingles(text: str, size: int = SHINGLE_SIZE) -> set[str]:
    normalized = normalize(text)
    if len(normalized) < size:
        return {normalized} if normalized else set()
    return {normalized[i : i + size] for i in range(len(normalized) - size + 1)}


def minhash_signature(bag: set[str], permutations: int = PERMUTATIONS) -> list[int]:
    signature = [1 << 62] * permutations
    for token in bag:
        for index in range(permutations):
            digest = hashlib.blake2b(
                token.encode("utf-8"), key=index.to_bytes(2, "big"), digest_size=8
            ).digest()
            value = int.from_bytes(digest, "big")
            if value < signature[index]:
                signature[index] = value
    return signature


def estimated_jaccard(left: list[int], right: list[int]) -> float:
    matches = sum(1 for a, b in zip(left, right) if a == b)
    return matches / len(left)


def exact_jaccard(left: set[str], right: set[str]) -> float:
    if not left and not right:
        return 1.0
    if not left or not right:
        return 0.0
    return len(left & right) / len(left | right)


def load_cases() -> list[dict[str, Any]]:
    records = []
    for split in SPLITS:
        path = CASES / split / "cases.jsonl"
        if not path.exists():
            continue
        records.extend(
            json.loads(line)
            for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        )
    return records


def build_report(threshold: float) -> dict[str, Any]:
    records = load_cases()
    bags = {case["case_id"]: shingles(case["input"]) for case in records}
    signatures = {
        case_id: minhash_signature(bag) for case_id, bag in bags.items()
    }
    by_id = {case["case_id"]: case for case in records}

    violations = []
    inspected = 0
    for left, right in combinations(sorted(bags), 2):
        if by_id[left]["premise_family"] == by_id[right]["premise_family"]:
            continue
        if estimated_jaccard(signatures[left], signatures[right]) < threshold * 0.8:
            continue
        inspected += 1
        similarity = exact_jaccard(bags[left], bags[right])
        if similarity >= threshold:
            violations.append(
                {
                    "case_a": left,
                    "family_a": by_id[left]["premise_family"],
                    "split_a": by_id[left]["split"],
                    "case_b": right,
                    "family_b": by_id[right]["premise_family"],
                    "split_b": by_id[right]["split"],
                    "exact_jaccard": round(similarity, 4),
                }
            )

    # within-family coherence: a family whose members share nothing may be a
    # mislabelled pair the other direction (same text idea, different labels
    # is a violation; same label, unrelated text is a naming smell)
    family_smells = []
    homes: dict[str, list[str]] = {}
    for case in records:
        homes.setdefault(case["premise_family"], []).append(case["case_id"])
    for family, members in sorted(homes.items()):
        if len(members) < 2:
            continue
        worst = 1.0
        for left, right in combinations(sorted(members), 2):
            worst = min(worst, exact_jaccard(bags[left], bags[right]))
        if worst == 0.0:
            family_smells.append(
                {"premise_family": family, "min_within_family_jaccard": 0.0}
            )

    return {
        "schema": "premise-family-check/v1",
        "cases": len(records),
        "method": {
            "shingle": f"character-{SHINGLE_SIZE}-gram",
            "minhash_permutations": PERMUTATIONS,
            "threshold": threshold,
            "pre_filter": "estimated >= 0.8 * threshold, then exact jaccard",
        },
        "cross_family_near_duplicates": violations,
        "zero_overlap_within_family": family_smells,
        "passes": not violations,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD)
    parser.add_argument(
        "--report", type=Path, default=REPORT, help="output path for the report"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    report = build_report(args.threshold)
    report_path = args.report
    report_path.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    verdict = "PASS" if report["passes"] else "FAIL"
    print(
        f"{verdict}: {report['cases']} cases, "
        f"{len(report['cross_family_near_duplicates'])} cross-family near-duplicates "
        f"(threshold {report['method']['threshold']}), "
        f"{len(report['zero_overlap_within_family'])} zero-overlap families"
    )
    for item in report["cross_family_near_duplicates"]:
        print(
            f"  NEAR-DUP {item['case_a']} ({item['family_a']}/{item['split_a']}) ~ "
            f"{item['case_b']} ({item['family_b']}/{item['split_b']}) "
            f"jaccard={item['exact_jaccard']}"
        )
    print(f"report: {report_path}")
    return 0 if report["passes"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
