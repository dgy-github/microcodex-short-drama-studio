"""Validate the tracked OpenAPI contract and operation-status coverage."""

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
METHODS = {"get", "put", "post", "delete", "patch", "head", "options", "trace"}


def main() -> int:
    contract = yaml.safe_load((ROOT / "contracts/openapi.yaml").read_text(encoding="utf-8"))
    status = yaml.safe_load((ROOT / "contracts/operation-status.yaml").read_text(encoding="utf-8"))
    errors = []
    if not str(contract.get("openapi", "")).startswith("3.1"):
        errors.append("OpenAPI version must be 3.1")
    operations = {
        operation.get("operationId")
        for path in contract.get("paths", {}).values()
        for method, operation in path.items()
        if method.lower() in METHODS and operation.get("operationId")
    }
    tracked = set(status.get("operations", {}))
    if operations != tracked:
        errors.append(f"operation status mismatch: missing={sorted(operations-tracked)}, orphaned={sorted(tracked-operations)}")
    if errors:
        print("\n".join(errors))
        return 1
    print(f"OpenAPI valid with {len(operations)} tracked operations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
