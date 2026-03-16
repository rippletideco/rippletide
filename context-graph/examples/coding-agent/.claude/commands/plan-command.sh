#!/bin/bash
set -euo pipefail

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
REQUEST="$(cat)"

if [[ -z "${REQUEST//[[:space:]]/}" ]]; then
  exit 0
fi

cd "$PROJECT_DIR"
printf '%s' "$REQUEST" | cargo run --quiet -- plan --raw --stdin
