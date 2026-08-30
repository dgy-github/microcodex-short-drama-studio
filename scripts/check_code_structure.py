"""Check hard source-file and Python function size limits."""

import argparse
import ast
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SUFFIXES = {".py", ".rs", ".ts", ".tsx", ".js", ".svelte"}
IGNORED_PARTS = {
    ".git",
    "node_modules",
    "__pycache__",
    ".venv",
    ".release-venv",
    ".mimosa",
    ".workbuddy",
    ".zcode",
    "story-sidecar",
}


def source_files() -> list[str]:
    """Return repository source files, excluding generated and dependency trees."""
    files = []
    for path in ROOT.rglob("*"):
        parts = path.relative_to(ROOT).parts
        if path.is_file() and path.suffix in SUFFIXES and not any(
            part in IGNORED_PARTS or part.startswith("target") for part in parts
        ):
            files.append(path.relative_to(ROOT).as_posix())
    return sorted(files)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--files",
        nargs="*",
        default=[],
        help="specific repository-relative files",
    )
    parser.add_argument("--all", action="store_true", help="scan all repository source files")
    args = parser.parse_args()
    if args.all and args.files:
        parser.error("--all and --files cannot be used together")
    names = source_files() if args.all else args.files
    errors = []
    for name in names:
        path = ROOT / name
        if not path.exists() or path.suffix not in SUFFIXES:
            continue
        lines = path.read_text(encoding="utf-8").splitlines()
        limit = 300 if path.suffix == ".svelte" else 700
        if len(lines) > limit:
            errors.append(f"{name}:1 has {len(lines)} lines; limit is {limit}")
        if path.suffix == ".py":
            tree = ast.parse("\n".join(lines))
            for node in ast.walk(tree):
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.end_lineno:
                    logical = sum(bool(line.strip()) and not line.lstrip().startswith("#") for line in lines[node.lineno - 1 : node.end_lineno])
                    if logical > 80:
                        errors.append(f"{name}:{node.lineno} {node.name} has {logical} logical lines; limit is 80")
    if errors:
        print("\n".join(errors))
        return 1
    print(f"Structure check passed for {len(names)} requested files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
