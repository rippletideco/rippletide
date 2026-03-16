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

cd "$PROJECT_DIR"
printf '%s' "$REQUEST" | cargo run --quiet -- plan --raw --stdin
