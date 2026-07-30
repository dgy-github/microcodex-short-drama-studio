"""Create or verify a public commitment for private holdout files."""

from __future__ import annotations

import argparse
import hashlib
import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from uuid import uuid4


def private_commitment(private_dir: Path) -> tuple[str, int, int]:
    root = private_dir.resolve(strict=True)
    if not root.is_dir():
        raise ValueError("private holdout path must be a directory")
    files = sorted(path for path in root.rglob("*") if path.is_file())
    if not files or any(path.is_symlink() for path in files):
        raise ValueError("holdout must contain regular files")
    digest = hashlib.sha256()
    case_count = 0
    for path in files:
        relative = path.relative_to(root).as_posix()
        content = path.read_bytes()
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(hashlib.sha256(content).digest())
        digest.update(b"\0")
        digest.update(str(len(content)).encode("ascii"))
        digest.update(b"\0")
        if path.suffix == ".jsonl":
            case_count += sum(1 for line in content.splitlines() if line.strip())
        elif path.suffix == ".json":
            case_count += 1
    if case_count == 0:
        raise ValueError("holdout contains no JSON cases")
    return digest.hexdigest(), case_count, len(files)


def create_seal(private_dir: Path, eval_version: str) -> dict[str, Any]:
    if not eval_version.strip():
        raise ValueError("eval version is required")
    commitment, case_count, file_count = private_commitment(private_dir)
    return {
        "schema": "holdout-seal/v1",
        "seal_id": f"seal_{uuid4().hex}",
        "eval_version": eval_version,
        "case_count": case_count,
        "file_count": file_count,
        "commitment_sha256": commitment,
        "created_at": datetime.now(UTC).isoformat(),
        "allowed_uses": ["evaluation"],
        "status": "sealed",
    }


def verify_seal(private_dir: Path, seal: dict[str, Any]) -> bool:
    commitment, case_count, file_count = private_commitment(private_dir)
    return (
        seal.get("schema") == "holdout-seal/v1"
        and seal.get("status") == "sealed"
        and seal.get("allowed_uses") == ["evaluation"]
        and seal.get("commitment_sha256") == commitment
        and seal.get("case_count") == case_count
        and seal.get("file_count") == file_count
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    subcommands = parser.add_subparsers(dest="command", required=True)
    seal = subcommands.add_parser("seal")
    seal.add_argument("--private-dir", type=Path, required=True)
    seal.add_argument("--eval-version", required=True)
    seal.add_argument("--output", type=Path, required=True)
    verify = subcommands.add_parser("verify")
    verify.add_argument("--private-dir", type=Path, required=True)
    verify.add_argument("--seal", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "seal":
        private_root = args.private_dir.resolve(strict=True)
        output = args.output.resolve()
        if output == private_root or private_root in output.parents or output.exists():
            raise ValueError("public seal output must be new and outside private holdout")
        payload = create_seal(args.private_dir, args.eval_version)
        output.write_text(
            json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        return 0
    payload = json.loads(args.seal.read_text(encoding="utf-8"))
    return 0 if verify_seal(args.private_dir, payload) else 1


if __name__ == "__main__":
    raise SystemExit(main())
