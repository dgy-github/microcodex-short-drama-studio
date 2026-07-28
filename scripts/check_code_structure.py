"""Check hard source-file and Python function size limits."""

import argparse
import ast
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SUFFIXES = {".py", ".rs", ".ts", ".tsx", ".js", ".svelte"}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--files", nargs="*", default=[])
    args = parser.parse_args()
    errors = []
    for name in args.files:
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
    print(f"Structure check passed for {len(args.files)} requested files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
