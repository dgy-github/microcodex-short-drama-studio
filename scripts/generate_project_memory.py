"""Generate lightweight module and public-symbol catalogs."""

from __future__ import annotations

import argparse
import ast
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MEMORY = ROOT / "docs" / "project-memory"
SUFFIXES = {".py", ".rs", ".ts", ".tsx", ".js", ".svelte"}
SKIP = {".git", ".venv", "node_modules", "target", "dist", "build"}
ITEM = re.compile(
    r"^\s*(?:export\s+|pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
    r"(?P<kind>fn|function|class|struct|enum|trait|interface|type|const)\s+"
    r"(?P<name>[A-Za-z_$][A-Za-z0-9_$]*)",
    re.MULTILINE,
)


def symbols(path: Path) -> list[dict]:
    relative = path.relative_to(ROOT).as_posix()
    language = "Python" if path.suffix == ".py" else "Rust" if path.suffix == ".rs" else "Web"
    result = [{"language": language, "path": relative, "kind": "module", "name": path.stem, "line": 1}]
    text = path.read_text(encoding="utf-8")
    if path.suffix == ".py":
        try:
            tree = ast.parse(text)
        except SyntaxError:
            return result
        for node in ast.walk(tree):
            if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
                result.append(
                    {
                        "language": language,
                        "path": relative,
                        "kind": type(node).__name__,
                        "name": node.name,
                        "line": node.lineno,
                    }
                )
    else:
        for match in ITEM.finditer(text):
            result.append(
                {
                    "language": language,
                    "path": relative,
                    "kind": match.group("kind"),
                    "name": match.group("name"),
                    "line": text.count("\n", 0, match.start()) + 1,
                }
            )
    return result


def discover() -> list[dict]:
    found = []
    for path in sorted(ROOT.rglob("*")):
        if (
            path.is_file()
            and path.suffix in SUFFIXES
            and not any(part in SKIP for part in path.relative_to(ROOT).parts)
        ):
            found.extend(symbols(path))
    return found


def render(items: list[dict]) -> dict[Path, str]:
    modules = [item for item in items if item["kind"] == "module"]
    public = [item for item in items if item["kind"] != "module" and not item["name"].startswith("_")]
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
        path.write_text(text, encoding="utf-8", newline="\n")
    print("Project memory generated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
