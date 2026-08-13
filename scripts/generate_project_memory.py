"""Generate lightweight module and public-symbol catalogs."""

from __future__ import annotations

import argparse
import ast
import json
import os
import re
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MEMORY = ROOT / "docs" / "project-memory"
SUFFIXES = {".py", ".rs", ".ts", ".tsx", ".js", ".svelte"}
SKIP = {
    ".git",
    ".venv",
    ".workbuddy",
    ".zcode",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    "playwright-report",
    "test-results",
}
TEST_PARTS = {"tests", "e2e", "e2e-tauri"}
RUST_ITEM = re.compile(
    r"^(?P<indent>[ \t]*)(?P<visibility>pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
    r"(?P<kind>fn|struct|enum|trait|type|const|static|mod)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)",
    re.MULTILINE,
)
WEB_ITEM = re.compile(
    r"^(?P<indent>[ \t]*)(?P<export>export\s+)?(?:default\s+)?(?:declare\s+)?(?:async\s+)?"
    r"(?P<kind>function|class|interface|type|const|let|var|enum)\s+"
    r"(?P<name>[A-Za-z_$][A-Za-z0-9_$]*)",
    re.MULTILINE,
)
REPLACE_ATTEMPTS = 5
REPLACE_BACKOFF_SECONDS = 0.05


def _symbol(language: str, relative: str, kind: str, name: str, line: int, public: bool) -> dict:
    return {
        "language": language,
        "path": relative,
        "kind": kind,
        "name": name,
        "line": line,
        "public": public,
    }


def symbols(path: Path) -> list[dict]:
    relative = path.relative_to(ROOT).as_posix()
    language = "Python" if path.suffix == ".py" else "Rust" if path.suffix == ".rs" else "Web"
    result = [_symbol(language, relative, "module", path.stem, 1, False)]
    text = path.read_text(encoding="utf-8")
    if path.suffix == ".py":
        try:
            tree = ast.parse(text)
        except SyntaxError:
            return result
        for node in tree.body:
            if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
                result.append(
                    _symbol(
                        language,
                        relative,
                        type(node).__name__,
                        node.name,
                        node.lineno,
                        not node.name.startswith("_"),
                    )
                )
        return result

    item_pattern = RUST_ITEM if path.suffix == ".rs" else WEB_ITEM
    for match in item_pattern.finditer(text):
        public = bool(match.group("visibility")) if path.suffix == ".rs" else bool(match.group("export"))
        result.append(
            _symbol(
                language,
                relative,
                match.group("kind"),
                match.group("name"),
                text.count("\n", 0, match.start()) + 1,
                public,
            )
        )
    return result


def _is_source(path: Path) -> bool:
    relative = path.relative_to(ROOT)
    parts = set(relative.parts)
    name = path.name
    return (
        path.is_file()
        and path.suffix in SUFFIXES
        and not parts.intersection(SKIP | TEST_PARTS)
        and MEMORY not in path.parents
        and ".test." not in name
        and ".spec." not in name
        and not (path.suffix == ".py" and name.startswith("test_"))
    )


def discover() -> list[dict]:
    found = []
    for path in sorted(ROOT.rglob("*")):
        if _is_source(path):
            found.extend(symbols(path))
    return found


def render(items: list[dict]) -> dict[Path, str]:
    modules = [item for item in items if item["kind"] == "module"]
    public = [item for item in items if item["kind"] != "module" and item["public"]]
    module_lines = ["# Module catalog", "", "> Generated; do not edit.", "", "| Module | Language |", "| --- | --- |"]
    module_lines += [f"| `{item['path']}` | {item['language']} |" for item in modules]
    interface_lines = ["# Interface catalog", "", "> Generated candidates; registry is authoritative.", "", "| Interface | Kind | Location |", "| --- | --- | --- |"]
    interface_lines += [
        f"| `{item['name']}` | {item['kind']} | `{item['path']}:{item['line']}` |"
        for item in public
    ]
    symbol_lines = ["# Symbol index", "", "> Generated; do not edit.", "", "| Language | Path | Line | Kind | Name |", "| --- | --- | ---: | --- | --- |"]
    symbol_lines += [
        f"| {item['language']} | `{item['path']}` | {item['line']} | {item['kind']} | `{item['name']}` |"
        for item in items
    ]
    return {
        MEMORY / "MODULE_CATALOG.md": "\n".join(module_lines) + "\n",
        MEMORY / "INTERFACE_CATALOG.md": "\n".join(interface_lines) + "\n",
        MEMORY / "SYMBOL_INDEX.md": "\n".join(symbol_lines) + "\n",
        MEMORY / "SYMBOL_INDEX.json": json.dumps(items, indent=2) + "\n",
    }


def _replace_with_retry(temporary: Path, target: Path) -> None:
    for attempt in range(REPLACE_ATTEMPTS):
        try:
            temporary.replace(target)
            return
        except OSError as error:
            if attempt == REPLACE_ATTEMPTS - 1:
                raise OSError(
                    f"Could not replace target {target} with temporary file {temporary} "
                    f"after {REPLACE_ATTEMPTS} attempts"
                ) from error
            time.sleep(REPLACE_BACKOFF_SECONDS * (attempt + 1))


def _write_atomic(path: Path, text: str) -> None:
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp"
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
        _replace_with_retry(temporary, path)
    finally:
        try:
            temporary.unlink(missing_ok=True)
        except OSError as cleanup_error:
            print(
                f"Warning: could not clean up temporary file {temporary} "
                f"for target {path}: {cleanup_error}",
                file=sys.stderr,
            )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    outputs = render(discover())
    stale = [path for path, text in outputs.items() if not path.exists() or path.read_text(encoding="utf-8") != text]
    if args.check:
        if stale:
            print("Project memory is stale: " + ", ".join(path.name for path in stale))
            return 1
        print("Project memory is current")
        return 0
    MEMORY.mkdir(parents=True, exist_ok=True)
    for path, text in outputs.items():
        _write_atomic(path, text)
    print("Project memory generated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
