#!/bin/bash
set -euo pipefail

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
if [[ "$#" -gt 0 ]]; then
  REQUEST="$*"
else
  REQUEST="$(cat)"
fi

PLAN="$(cat)"

unset CLAUDECODE
unset CLAUDE_PROJECT_DIR

if [[ -z "${REQUEST//[[:space:]]/}" ]]; then
  echo "Plan review request cannot be empty" >&2
  exit 1
fi

if [[ -z "${PLAN//[[:space:]]/}" ]]; then
  echo "Candidate plan cannot be empty" >&2
  exit 1
fi

if [[ -n "${RIPPLETIDE_PLAN_CLI_BIN:-}" ]]; then
  PLAN_CMD=("$RIPPLETIDE_PLAN_CLI_BIN")
else
  PACKAGE_VERSION="${RIPPLETIDE_PLAN_CLI_VERSION:-0.5.8}"
  PLAN_CMD=(npx -y "rippletide-code@${PACKAGE_VERSION}")
fi

cd "$PROJECT_DIR"
printf '%s' "$PLAN" | "${PLAN_CMD[@]}" review-plan "$REQUEST" --stdin --json
