#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

npm --prefix apps/runtime ci
npm --prefix apps/runtime test
(cd apps/runtime && npm pack --dry-run)
npm --prefix apps/runtime audit --omit=dev

npm --prefix adapters/mcp-gateway-js test
(cd adapters/mcp-gateway-js && npm pack --dry-run)

python3 -m venv .venv
.venv/bin/pip install -e cli
.venv/bin/python -m pip wheel ./cli -w /tmp/tide-wheel-test --no-deps

node examples/tide-demo-agent/agent.mjs demo
