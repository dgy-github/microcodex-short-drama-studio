"""Archive generated baseline artifacts out of the gitignored run directory.

`.gitignore` excludes `/eval/runs/`, so baselines produced there exist only on
the machine that paid for them. Generation is stochastic, so regenerating does
not reproduce them: losing the directory loses the reference point that every
later comparison is measured against.

The split is deliberate:

- **artifacts** (story-package + wrapper) are evaluation inputs and are
  archived into `eval/baselines/<run_id>/`, which is tracked;
- **raw provider responses** are run telemetry — larger, and carrying usage and
  billing metadata — and stay in the ignored run directory.

Content hashes are verified on the way in and recorded in the index, so a
silently edited archive is detectable.

Usage (from the repository root):
    python eval/tools/archive_baselines.py
    python eval/tools/archive_baselines.py --check
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).parents[2]
RUNS = ROOT / "eval" / "runs"
ARCHIVE = ROOT / "eval" / "baselines"


def sha256(path: Path) -> str:
    """Hash the content with line endings normalized to LF.

    Windows checkouts with autocrlf smudge the archived packages to CRLF,
    which changes the bytes without changing the content; recorded hashes
    were produced from LF files. Tampering still changes the digest.
    """
    normalized = path.read_bytes().replace(b"\r\n", b"\n")
    return "sha256:" + hashlib.sha256(normalized).hexdigest()


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def collect(run_dir: Path) -> tuple[dict[str, Any], list[tuple[str, Path, Path]]]:
    config = load(run_dir / "config.json")
    artifacts = run_dir / "artifacts"
    entries: list[tuple[str, Path, Path]] = []
    for case_id in config["case_ids"]:
        package = artifacts / f"{case_id}.story-package.json"
        wrapper = artifacts / f"{case_id}.artifact.json"
        if not package.exists() or not wrapper.exists():
            raise SystemExit(f"{case_id}: missing artifact files under {artifacts}")
        entries.append((case_id, package, wrapper))
    return config, entries


def build_index(config: dict[str, Any], entries: list[tuple[str, Path, Path]]) -> dict[str, Any]:
    cases = []
    for case_id, package, wrapper in entries:
        wrapper_body = load(wrapper)
        digest = sha256(package)
        if wrapper_body["content_hash"] != digest:
            raise SystemExit(
                f"{case_id}: wrapper content_hash does not match the package "
                f"({wrapper_body['content_hash']} vs {digest})"
            )
        cases.append(
            {
                "case_id": case_id,
                "artifact_id": wrapper_body["artifact_id"],
                "content_hash": digest,
                "package": f"{case_id}.story-package.json",
                "wrapper": f"{case_id}.artifact.json",
            }
        )
    return {
        "schema": "baseline-archive/v1",
        "run_id": config["run_id"],
        "story_package_schema": config["story_package_schema"],
        "generator": config["generator"],
        "created_at": config["created_at"],
        "note": (
            "Artifacts archived from eval/runs/, which .gitignore excludes. "
            "Raw provider responses stay in the run directory: they are run "
            "telemetry, not evaluation inputs."
        ),
        "cases": cases,
    }


def write(run_id: str, entries: list[tuple[str, Path, Path]], index: dict[str, Any]) -> None:
    destination = ARCHIVE / run_id
    destination.mkdir(parents=True, exist_ok=True)
    for _, package, wrapper in entries:
        for source in (package, wrapper):
            (destination / source.name).write_bytes(source.read_bytes())
    (destination / "index.json").write_text(
        json.dumps(index, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def verify(run_id: str, index: dict[str, Any]) -> list[str]:
    destination = ARCHIVE / run_id
    problems: list[str] = []
    for case in index["cases"]:
        package = destination / case["package"]
        if not package.exists():
            problems.append(f"{case['case_id']}: missing from the archive")
            continue
        digest = sha256(package)
        if digest != case["content_hash"]:
            problems.append(
                f"{case['case_id']}: archived copy hash {digest} != "
                f"recorded {case['content_hash']}"
            )
    return problems


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    run_dirs = sorted(p for p in RUNS.glob("*") if (p / "config.json").exists())
    if not run_dirs:
        if args.check:
            # Nothing local to compare against; the archive is the record.
            print("no run directories present; archive is the only copy")
            return 0
        raise SystemExit(f"no run directories with config.json under {RUNS}")

    failures = 0
    for run_dir in run_dirs:
        config, entries = collect(run_dir)
        index = build_index(config, entries)
        run_id = config["run_id"]

        if args.check:
            archived_index = ARCHIVE / run_id / "index.json"
            if not archived_index.exists():
                print(f"MISSING {run_id}: not archived")
                failures += 1
                continue
            problems = verify(run_id, load(archived_index))
            for problem in problems:
                print(f"DRIFT {run_id}: {problem}")
            failures += len(problems)
            if not problems:
                print(f"OK {run_id}: {len(entries)} baselines match")
            continue

        write(run_id, entries, index)
        problems = verify(run_id, index)
        for problem in problems:
            print(f"DRIFT {run_id}: {problem}", file=sys.stderr)
        failures += len(problems)
        print(f"archived {len(entries)} baselines to eval/baselines/{run_id}/")

    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
