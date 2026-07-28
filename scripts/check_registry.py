"""Validate project capability and interface ownership records."""

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
CAP = {"id", "name", "summary", "keywords", "owner", "interfaces", "reuse_rule"}
IFACE = {"id", "name", "kind", "location", "purpose", "inputs", "outputs", "errors", "side_effects"}


def main() -> int:
    value = yaml.safe_load((ROOT / "docs/project-memory/PROJECT_REGISTRY.yaml").read_text(encoding="utf-8"))
    errors = []
    for label, records, required in (
        ("capability", value.get("capabilities", []), CAP),
        ("interface", value.get("interfaces", []), IFACE),
    ):
        ids = [record.get("id") for record in records]
        for index, record in enumerate(records):
            missing = sorted(required - set(record))
            if missing:
                errors.append(f"{label}[{index}] missing: {', '.join(missing)}")
            owner = record.get("owner") or record.get("location")
            if owner and not (ROOT / owner.split(":", 1)[0]).exists():
                errors.append(f"{label}[{index}] owner does not exist: {owner}")
        errors += [f"duplicate {label} id: {item}" for item in set(ids) if item and ids.count(item) > 1]
    if errors:
        print("\n".join(errors))
        return 1
    print("Project registry is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
