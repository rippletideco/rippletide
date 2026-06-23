"""Locate and invoke the TypeScript evaluator/runtime.

One engine everywhere: `tide compile/test` call the same compiled evaluator
the runtime serves, so a green test means the production verdict, not a Python
re-implementation of it.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path


def repo_root() -> Path:
    """Find the monorepo root (contains apps/runtime). Override with TIDE_HOME."""
    env = os.environ.get("TIDE_HOME")
    candidates = [Path(env)] if env else []
    candidates += list(Path(__file__).resolve().parents) + list(Path.cwd().resolve().parents) + [Path.cwd()]
    for c in candidates:
        if (c / "apps" / "runtime" / "package.json").exists():
            return c
    sys.exit("error: cannot locate the tide runtime (apps/runtime). Set TIDE_HOME to the repo root.")


def _node() -> str:
    node = shutil.which("node")
    if not node:
        sys.exit("error: node is required (>= 20). Install Node.js or use the Docker image.")
    return node


def evalcli_path() -> Path:
    dist = repo_root() / "apps" / "runtime" / "dist" / "src" / "evalcli.js"
    if not dist.exists():
        sys.exit(f"error: runtime not built ({dist} missing). Run: cd {repo_root() / 'apps' / 'runtime'} && npm install && npm run build")
    return dist


def server_path() -> Path:
    dist = repo_root() / "apps" / "runtime" / "dist" / "src" / "server.js"
    if not dist.exists():
        sys.exit(f"error: runtime not built ({dist} missing). Run: cd {repo_root() / 'apps' / 'runtime'} && npm install && npm run build")
    return dist


def run_evalcli(command: str, project: Path, extra: list[str] | None = None) -> dict:
    """Invoke the evaluator and parse its JSON output. Exit code is preserved by callers."""
    cmd = [_node(), str(evalcli_path()), command, "--project", str(project)] + (extra or [])
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if not proc.stdout.strip():
        sys.exit(f"error: evaluator produced no output\n{proc.stderr}")
    try:
        result = json.loads(proc.stdout)
    except json.JSONDecodeError:
        sys.exit(f"error: evaluator output was not JSON:\n{proc.stdout}\n{proc.stderr}")
    result["_exit_code"] = proc.returncode
    return result


def run_server(project: Path, port: int, host: str, unsafe_bind_public: bool = False) -> None:
    """Foreground the runtime server (Ctrl-C to stop)."""
    cmd = [_node(), str(server_path()), "--project", str(project), "--port", str(port), "--host", host]
    if unsafe_bind_public:
        cmd.append("--unsafe-bind-public")
    try:
        subprocess.run(cmd)
    except KeyboardInterrupt:
        pass
