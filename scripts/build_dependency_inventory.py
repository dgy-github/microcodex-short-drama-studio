"""Build a deterministic dependency and license inventory from lockfiles."""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import subprocess
import tomllib
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / "config/distribution-license-policy-v1.json"
LICENSE_ROOT = ROOT / "third_party/licenses"
POLICY_FIELDS = {
    "ecosystem",
    "name",
    "source",
    "revision",
    "license",
    "evidence_path",
    "evidence_sha256",
    "approved_for_distribution",
    "reviewed_by",
    "reviewed_at",
}


def cargo_packages() -> list[dict[str, str]]:
    document = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    licenses = {
        (package["name"], package["version"]): package.get("license") or "UNKNOWN"
        for package in metadata["packages"]
    }
    return sorted(
        (
            {
                "ecosystem": "cargo",
                "name": package["name"],
                "version": package["version"],
                "license": licenses.get(
                    (package["name"], package["version"]), "UNKNOWN"
                ),
            }
            for package in document["package"]
        ),
        key=lambda item: (item["name"], item["version"]),
    )


def npm_packages() -> list[dict[str, str]]:
    document = json.loads(
        (ROOT / "apps/desktop/package-lock.json").read_text(encoding="utf-8")
    )
    packages = []
    for location, package in document["packages"].items():
        if not location or "version" not in package:
            continue
        packages.append(
            {
                "ecosystem": "npm",
                "name": location.rsplit("node_modules/", 1)[-1],
                "version": package["version"],
                "license": package.get("license", "UNKNOWN"),
            }
        )
    return sorted(packages, key=lambda item: (item["name"], item["version"]))


def python_packages() -> list[dict[str, str]]:
    document = tomllib.loads(
        (ROOT / "sidecar/pyproject.toml").read_text(encoding="utf-8")
    )
    packages = []
    for dependency in document["project"]["dependencies"]:
        name = dependency.split("@", 1)[0].split("==", 1)[0].strip()
        if " @ git+" in dependency:
            version = dependency.rsplit("@", 1)[-1].strip()
            license_name = "UNKNOWN"
        else:
            try:
                metadata = importlib.metadata.metadata(name)
                version = importlib.metadata.version(name)
                license_name = (
                    metadata.get("License-Expression")
                    or metadata.get("License")
                    or "UNKNOWN"
                )
            except importlib.metadata.PackageNotFoundError:
                version = dependency.split("==", 1)[1]
                license_name = "UNKNOWN"
        packages.append(
            {
                "ecosystem": "python",
                "name": name,
                "version": version,
                "license": license_name,
            }
        )
    return packages


def pinned_git_dependencies() -> dict[tuple[str, str], tuple[str, str]]:
    document = tomllib.loads(
        (ROOT / "sidecar/pyproject.toml").read_text(encoding="utf-8")
    )
    pinned: dict[tuple[str, str], tuple[str, str]] = {}
    for dependency in document["project"]["dependencies"]:
        if " @ git+" not in dependency:
            continue
        name, git_reference = dependency.split(" @ git+", 1)
        source, revision = git_reference.rsplit("@", 1)
        pinned[("python", name.strip())] = (source, revision)
    return pinned


def load_license_policy() -> tuple[dict, str]:
    raw = POLICY_PATH.read_bytes()
    policy = json.loads(raw.decode("utf-8"))
    if policy.get("schema") != "distribution-license-policy/v1":
        raise ValueError("unexpected distribution license policy schema")
    if not isinstance(policy.get("dependencies"), list):
        raise ValueError("distribution license policy dependencies must be a list")
    return policy, hashlib.sha256(raw).hexdigest()


def validate_license_policy(
    policy: dict,
    pinned: dict[tuple[str, str], tuple[str, str]],
) -> dict[tuple[str, str], dict]:
    entries: dict[tuple[str, str], dict] = {}
    license_root = LICENSE_ROOT.resolve()
    for entry in policy["dependencies"]:
        if set(entry) != POLICY_FIELDS:
            raise ValueError("distribution license policy fields are invalid")
        key = (entry["ecosystem"], entry["name"])
        if key in entries:
            raise ValueError(f"duplicate distribution license policy: {key}")
        if key not in pinned:
            raise ValueError(f"license policy does not match a pinned dependency: {key}")
        if pinned[key] != (entry["source"], entry["revision"]):
            raise ValueError(f"license policy pin drift: {entry['name']}")
        approved = entry["approved_for_distribution"]
        if not isinstance(approved, bool):
            raise ValueError("approved_for_distribution must be boolean")
        if not approved:
            if (
                entry["license"] != "UNKNOWN"
                or entry["evidence_path"] is not None
                or entry["evidence_sha256"] is not None
                or entry["reviewed_by"] is not None
                or entry["reviewed_at"] is not None
            ):
                raise ValueError(
                    f"unapproved license policy must remain empty: {entry['name']}"
                )
        else:
            if entry["license"] == "UNKNOWN":
                raise ValueError(f"approved license is UNKNOWN: {entry['name']}")
            evidence_path = entry["evidence_path"]
            evidence_hash = entry["evidence_sha256"]
            if not isinstance(evidence_path, str) or not isinstance(
                evidence_hash, str
            ):
                raise ValueError(f"approved license evidence is incomplete: {entry['name']}")
            evidence = (ROOT / evidence_path).resolve()
            if not evidence.is_relative_to(license_root) or not evidence.is_file():
                raise ValueError(f"approved license evidence is unsafe: {entry['name']}")
            if hashlib.sha256(evidence.read_bytes()).hexdigest() != evidence_hash:
                raise ValueError(f"approved license evidence hash mismatch: {entry['name']}")
            if not entry["reviewed_by"] or not entry["reviewed_at"]:
                raise ValueError(f"approved license review is incomplete: {entry['name']}")
            try:
                reviewed_at = datetime.fromisoformat(
                    entry["reviewed_at"].replace("Z", "+00:00")
                )
            except ValueError as error:
                raise ValueError(
                    f"approved license review time is invalid: {entry['name']}"
                ) from error
            if reviewed_at.tzinfo is None:
                raise ValueError(
                    f"approved license review time lacks timezone: {entry['name']}"
                )
        entries[key] = entry
    return entries


def build_inventory() -> dict:
    packages = cargo_packages() + npm_packages() + python_packages()
    policy, policy_hash = load_license_policy()
    policy_entries = validate_license_policy(policy, pinned_git_dependencies())
    for package in packages:
        package["license_source"] = (
            "package-metadata"
            if package["license"] != "UNKNOWN"
            else "missing"
        )
        policy_entry = policy_entries.get((package["ecosystem"], package["name"]))
        if (
            package["license"] == "UNKNOWN"
            and policy_entry
            and policy_entry["approved_for_distribution"]
        ):
            package["license"] = policy_entry["license"]
            package["license_source"] = "reviewed-policy"
    review_required = sorted(
        {
            package["name"]
            for package in packages
            if package["license"] == "UNKNOWN"
        }
    )
    return {
        "schema": "dependency-inventory/v1",
        "source_lockfiles": [
            "Cargo.lock",
            "apps/desktop/package-lock.json",
            "sidecar/pyproject.toml",
        ],
        "license_policy": "config/distribution-license-policy-v1.json",
        "license_policy_sha256": policy_hash,
        "packages": packages,
        "review_required": review_required,
        "distribution_cleared": not review_required,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "docs/generated/dependency-inventory.json",
    )
    args = parser.parse_args()
    inventory = build_inventory()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(inventory, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"Dependency inventory: {len(inventory['packages'])} packages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
