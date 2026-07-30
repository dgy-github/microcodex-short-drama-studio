"""Install-time verification for the pinned Campaign Python dependency."""

from __future__ import annotations

import argparse
import json
import re
import tomllib
from importlib.metadata import PackageNotFoundError, distribution
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SIDECAR_PROJECT = ROOT / "sidecar/pyproject.toml"
DEPENDENCY_NAME = "campaign-muti-agent"
PIN_PATTERN = re.compile(
    r"^campaign-muti-agent\s*@\s*git\+(?P<url>https://[^@]+)"
    r"@(?P<revision>[a-f0-9]{40})$"
)


def pinned_requirement(path: Path = SIDECAR_PROJECT) -> tuple[str, str, str]:
    project = tomllib.loads(path.read_text(encoding="utf-8"))["project"]
    matches = [
        dependency
        for dependency in project["dependencies"]
        if dependency.split("@", 1)[0].strip() == DEPENDENCY_NAME
    ]
    if len(matches) != 1:
        raise ValueError("exactly one campaign-muti-agent dependency is required")
    requirement = matches[0]
    match = PIN_PATTERN.fullmatch(requirement)
    if match is None:
        raise ValueError("campaign-muti-agent must use an exact HTTPS git revision")
    return requirement, match.group("url"), match.group("revision")


def verify_direct_url(
    document: dict, expected_url: str, expected_revision: str
) -> None:
    vcs = document.get("vcs_info", {})
    if document.get("url") != expected_url:
        raise ValueError("installed Campaign source URL does not match the pin")
    if vcs.get("vcs") != "git":
        raise ValueError("installed Campaign dependency is not git-backed")
    if vcs.get("commit_id") != expected_revision:
        raise ValueError("installed Campaign commit does not match the pin")
    if vcs.get("requested_revision") != expected_revision:
        raise ValueError("installed Campaign requested revision does not match the pin")


def verify_installed() -> None:
    _, expected_url, expected_revision = pinned_requirement()
    try:
        installed = distribution(DEPENDENCY_NAME)
    except PackageNotFoundError as error:
        raise ValueError("campaign-muti-agent is not installed") from error
    raw = installed.read_text("direct_url.json")
    if raw is None:
        raise ValueError("installed Campaign dependency has no direct_url evidence")
    verify_direct_url(json.loads(raw), expected_url, expected_revision)
    declared_license = (
        installed.metadata.get("License-Expression")
        or installed.metadata.get("License")
    )
    if declared_license != "MIT":
        raise ValueError("installed Campaign dependency does not declare MIT")


def main() -> int:
    parser = argparse.ArgumentParser()
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--print-requirement", action="store_true")
    action.add_argument("--verify-installed", action="store_true")
    args = parser.parse_args()
    try:
        if args.print_requirement:
            print(pinned_requirement()[0])
        else:
            verify_installed()
            print("Pinned Campaign installation verified")
    except (ValueError, json.JSONDecodeError, OSError, KeyError) as error:
        print(error)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
