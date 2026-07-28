"""Detect multiple owners for protected business-rule categories."""

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
PROTECTED = {"authorization", "validation", "state-machine", "retry", "pricing", "configuration", "error-mapping"}


def main() -> int:
    registry = yaml.safe_load((ROOT / "docs/project-memory/PROJECT_REGISTRY.yaml").read_text(encoding="utf-8"))
    owners = {}
    for capability in registry.get("capabilities", []):
        for keyword in set(capability.get("keywords", [])) & PROTECTED:
            owners.setdefault(keyword, set()).add(capability["owner"])
    errors = [f"protected rule '{rule}' has multiple owners: {sorted(paths)}" for rule, paths in owners.items() if len(paths) > 1]
    if errors:
        print("\n".join(errors))
        return 1
    print("Protected business-rule ownership is unique")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
