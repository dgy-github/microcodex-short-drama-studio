"""Require TEST links when feature traceability declares requirements."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    features = ROOT / "docs" / "features"
    if not features.exists():
        print("No feature directories yet; traceability check passed")
        return 0
    errors = []
    for feature in (path for path in features.iterdir() if path.is_dir()):
        files = list(feature.glob("*traceability*"))
        if not files:
            errors.append(f"{feature.relative_to(ROOT)}: missing traceability file")
            continue
        text = "\n".join(path.read_text(encoding="utf-8") for path in files)
        if re.search(r"REQ-\d{3,}", text) and not re.search(r"TEST-\d{3,}", text):
            errors.append(f"{feature.relative_to(ROOT)}: requirements lack TEST links")
    if errors:
        print("\n".join(errors))
        return 1
    print("Traceability check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
