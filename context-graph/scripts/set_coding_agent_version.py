#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


PACKAGE_JSON_PATHS = [
    Path("context-graph/npm/rippletide-code/package.json"),
    Path("context-graph/npm/rippletide-code-darwin-arm64/package.json"),
    Path("context-graph/npm/rippletide-code-darwin-x64/package.json"),
    Path("context-graph/npm/rippletide-code-linux-arm64/package.json"),
    Path("context-graph/npm/rippletide-code-linux-x64/package.json"),
    Path("context-graph/npm/rippletide-code-win32-arm64/package.json"),
    Path("context-graph/npm/rippletide-code-win32-x64/package.json"),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Set the Coding Agent CLI version across Cargo.toml and npm package manifests."
    )
    parser.add_argument("version", help="Semver version to write, for example 0.5.47 or 0.5.47-internal.1")
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[2],
        help="Repository root. Defaults to the rippletide repo root.",
    )
    return parser.parse_args()


def update_cargo_toml(path: Path, version: str) -> None:
    lines = path.read_text(encoding="utf-8").splitlines()
    in_package_block = False
    updated = False
    new_lines: list[str] = []

    for line in lines:
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_package_block = stripped == "[package]"
        if in_package_block and stripped.startswith("version = "):
            line = re.sub(r'version\s*=\s*"[^"]+"', f'version = "{version}"', line, count=1)
            updated = True
            in_package_block = False
        new_lines.append(line)

    if not updated:
        raise RuntimeError(f"Could not find package version in {path}")

    path.write_text("\n".join(new_lines) + "\n", encoding="utf-8")


def update_package_json(path: Path, version: str) -> None:
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["version"] = version
    if path.name == "package.json" and payload.get("name") == "rippletide-code":
        optional = payload.get("optionalDependencies")
        if isinstance(optional, dict):
            for key in list(optional.keys()):
                optional[key] = version
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    repo_root = args.root.resolve()
    update_cargo_toml(repo_root / "context-graph/Cargo.toml", args.version)
    for relative_path in PACKAGE_JSON_PATHS:
        update_package_json(repo_root / relative_path, args.version)


if __name__ == "__main__":
    main()
