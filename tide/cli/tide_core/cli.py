"""tide — compile, test, and run an MCP policy project."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from . import runtime as rt

BOLD, DIM, RED, GREEN, YELLOW, CYAN, RESET = "\033[1m", "\033[2m", "\033[31m", "\033[32m", "\033[33m", "\033[36m", "\033[0m"


def _project(args) -> Path:
    p = Path(getattr(args, "project", ".") or ".").resolve()
    if not (p / "tide.yaml").exists():
        sys.exit(f"error: {p} is not a tide project (tide.yaml not found). Run `tide init` first.")
    return p


def cmd_init(args) -> None:
    target = Path(args.dir).resolve()
    if target.exists() and any(target.iterdir()):
        sys.exit(f"error: {target} already exists and is not empty")

    for d in ("ontology", "policies", "tests"):
        (target / d).mkdir(parents=True, exist_ok=True)
    (target / "tide.yaml").write_text(f"name: {target.name}\nversion: \"1\"\nruntime:\n  url: http://localhost:8787\n")
    (target / "ontology" / "actions.yaml").write_text(
        "# Auto-filled by MCP gateway setup from MCP tools/list.\n"
        "actions: {}\n"
    )
    (target / "policies" / "rules.policy.yaml").write_text(
        "# Auto-filled by MCP gateway setup with --from <business-doc>.\n"
        "[]\n"
    )
    (target / "tests" / "golden.scenarios.yaml").write_text(
        "# Auto-filled with generated policy checks. Run with `tide test`.\n"
        "[]\n"
    )
    (target / "fixtures.json").write_text("{}\n")
    (target / "resolvers.js").write_text("export default function resolvers(_fixtures) {\n  return {};\n}\n")
    print(f"{GREEN}✓{RESET} initialized policy project '{target.name}'")
    print("\nNext: run MCP gateway setup against your MCP server and business docs")


def cmd_compile(args) -> None:
    project = _project(args)
    result = rt.run_evalcli("compile", project)
    for d in result.get("diagnostics", []):
        color = RED if d["level"] == "error" else YELLOW
        print(f"  {color}{d['level']}{RESET} {d['where']}: {d['message']}")
    if result.get("ok"):
        b = result["bundle"]
        print(f"{GREEN}✓{RESET} compiled bundle '{b['name']}' v{b['version']}: {b['actions']} actions, {b['rules']} rules -> .tide/bundle.json")
    else:
        sys.exit(f"{RED}✗ compile failed{RESET}")


def cmd_test(args) -> None:
    project = _project(args)
    result = rt.run_evalcli("test", project)
    if "results" not in result:
        sys.exit(f"{RED}✗{RESET} {result.get('diagnostics', result)}")
    for r in result["results"]:
        mark = f"{GREEN}✓{RESET}" if r["pass"] else f"{RED}✗{RESET}"
        line = f"  {mark} {r['name']}  {DIM}expect {r['expect']}, got {r['got']}{RESET}"
        if r["missing_rules"]:
            line += f" {RED}(missing rules: {', '.join(r['missing_rules'])}){RESET}"
        print(line)
    cov = result["coverage"]
    print(f"\n{BOLD}{result['passed']}/{result['total']} scenarios passed{RESET} · rule coverage {cov['covered']}/{cov['rules']}", end="")
    print(f" {YELLOW}(uncovered: {', '.join(cov['uncovered'])}){RESET}" if cov["uncovered"] else "")
    if not result.get("ok"):
        sys.exit(1)


def cmd_run(args) -> None:
    project = _project(args)
    result = rt.run_evalcli("compile", project)
    if not result.get("ok"):
        for d in result.get("diagnostics", []):
            print(f"  {RED}{d['level']}{RESET} {d['where']}: {d['message']}")
        sys.exit(f"{RED}✗ fix compile errors before running{RESET}")
    print(f"{CYAN}Console:{RESET} http://{args.host}:{args.port}  {DIM}(Ctrl-C to stop){RESET}")
    rt.run_server(project, args.port, args.host, args.unsafe_bind_public)


def main() -> None:
    parser = argparse.ArgumentParser(prog="tide", description="Compile, test, and run an MCP policy project.")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("init", help="scaffold an empty MCP policy project")
    p.add_argument("dir")
    p.set_defaults(fn=cmd_init)

    p = sub.add_parser("compile", help="validate and compile the policy bundle")
    p.add_argument("--project", default=".")
    p.set_defaults(fn=cmd_compile)

    p = sub.add_parser("test", help="run generated policy scenarios")
    p.add_argument("--project", default=".")
    p.set_defaults(fn=cmd_test)

    p = sub.add_parser("run", help="boot the Decision Runtime and Console")
    p.add_argument("--project", default=".")
    p.add_argument("--port", type=int, default=8787)
    p.add_argument("--host", default="127.0.0.1", help="bind host; use 0.0.0.0 only on a trusted network")
    p.add_argument("--unsafe-bind-public", action="store_true", help="allow unauthenticated runtime APIs on a non-loopback host")
    p.set_defaults(fn=cmd_run)

    args = parser.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
