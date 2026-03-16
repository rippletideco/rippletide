#!/bin/bash
set -euo pipefail

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
if [[ "$#" -gt 0 ]]; then
  REQUEST="$*"
else
  REQUEST="$(cat)"
fi

unset CLAUDECODE
unset CLAUDE_PROJECT_DIR

if [[ -z "${REQUEST//[[:space:]]/}" ]]; then
  exit 0
fi

if [[ -n "${RIPPLETIDE_PLAN_CLI_BIN:-}" ]]; then
  PLAN_CMD=("$RIPPLETIDE_PLAN_CLI_BIN")
else
  PACKAGE_VERSION="${RIPPLETIDE_PLAN_CLI_VERSION:-0.4.1}"
  PLAN_CMD=(npx -y "rippletide-mcp@${PACKAGE_VERSION}")
fi

cd "$PROJECT_DIR"
printf '%s' "$REQUEST" | "${PLAN_CMD[@]}" plan --raw --stdin
