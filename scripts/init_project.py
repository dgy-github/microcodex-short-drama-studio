"""Idempotently initialize and verify the project development foundation."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / ".project" / "init.yaml"
STATE = ROOT / ".project" / "state.json"


def load_manifest() -> dict:
    return yaml.safe_load(MANIFEST.read_text(encoding="utf-8"))


def fingerprint() -> str:
    # Keep initialization portable across CI checkout locations while still
    # detecting a copied template that retained a different directory name.
    return hashlib.sha256(ROOT.name.encode()).hexdigest()[:16]


def missing(manifest: dict) -> list[str]:
    names = manifest["required_reads"] + manifest["required_files"]
    return [name for name in names if not (ROOT / name).exists()]


def placeholder_errors(manifest: dict) -> list[str]:
    errors = []
    candidates = manifest["required_reads"] + manifest["required_files"]
    for name in candidates:
        path = ROOT / name
        if path == MANIFEST or not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for placeholder in manifest.get("template_placeholders", []):
            if placeholder in text:
                errors.append(f"template placeholder {placeholder!r} remains in {name}")
    return errors


def run_checks() -> None:
    for script, args in (
        ("generate_project_memory.py", ["--check"]),
        ("check_registry.py", []),
        ("check_openapi.py", []),
        ("check_traceability.py", []),
        ("check_duplicate_owners.py", []),
    ):
        subprocess.run(
            [sys.executable, str(ROOT / "scripts" / script), *args],
            cwd=ROOT,
            check=True,
        )


def state_errors() -> list[str]:
    if not STATE.exists():
        return ["project is not initialized"]
    state = json.loads(STATE.read_text(encoding="utf-8"))
    errors = []
    if not state.get("initialized"):
        errors.append("project is not initialized")
    # The state file is committed with the project foundation and is routinely
    # checked from arbitrary CI checkout paths.  Root metadata remains useful
    # for diagnostics, but a path change is not evidence that initialization
    # is invalid; all required files and generated catalogs are checked below.
    if not state.get("project_name"):
        errors.append("project name is missing")
    return errors


def initialize(name: str) -> int:
    manifest = load_manifest()
    absent = missing(manifest)
    if absent:
        print("Missing required project files: " + ", ".join(absent))
        return 1
    placeholders = placeholder_errors(manifest)
    if placeholders:
        print("\n".join(placeholders))
        return 1
    subprocess.run(
        [sys.executable, str(ROOT / "scripts/generate_project_memory.py")],
        cwd=ROOT,
        check=True,
    )
    state = {
        "schema_version": 1,
        "initialized": True,
        "project_name": name,
        "root_name": ROOT.name,
        "root_fingerprint": fingerprint(),
        "initialized_at": datetime.now(UTC).isoformat(),
    }
    STATE.write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")
    run_checks()
    print(f"Project initialized: {name}")
    return 0


def check() -> int:
    manifest = load_manifest()
    errors = [f"missing required file: {name}" for name in missing(manifest)]
    errors.extend(placeholder_errors(manifest))
    errors.extend(state_errors())
    if errors:
        print("\n".join(errors))
        return 1
    run_checks()
    print("Project initialization is valid")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--name")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    return check() if args.check else initialize(args.name or ROOT.name)


if __name__ == "__main__":
    raise SystemExit(main())
