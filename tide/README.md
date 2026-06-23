# Tide - MCP Policy Gateway

Tide adds deterministic pre-execution guardrails for MCP tool calls. It sits in
front of MCP servers, discovers available tools, turns policy text into
reviewable rules, then authorizes every `tools/call` before the real tool runs.

```text
MCP client -> Tide gateway -> MCP server
                 |
                 v
          allow / deny / escalate
```

This repo is a public technical preview, not a production v1. The preview is
GitHub-first; npm and PyPI publishing are deferred until the install surface is
settled.

## Scope

Covered:

- MCP tool calls.
- Local runtime and console.
- Generated policy projects with rules, tests, and findings.
- `allow`, `deny`, and `escalate` before the MCP server executes.

Not covered:

- native SDK tools;
- direct HTTP calls;
- shell or browser actions;
- hosted multi-tenant runtime;
- authenticated public API.

## Install From A Fresh Clone

Requires Node >= 20 and Python >= 3.10.

```bash
cd tide
npm --prefix apps/runtime ci
npm --prefix apps/runtime run build
python3 -m venv .venv
.venv/bin/pip install -e cli
```

`OPENAI_API_KEY` is optional. Without it, setup uses local schema heuristics. If
you set it, `setup` sends the supplied policy text and MCP tool schemas to
OpenAI, so do not pass confidential policy docs unless that is intended.

## Generate A Policy Project

Use any trusted MCP server command. For a local smoke test, this repo includes a
tiny MCP server with read/write/delete tools:

```bash
node adapters/mcp-gateway-js/index.js setup \
  --yes \
  --project /tmp/tide-policy \
  --upstream "node examples/mcp-test-agent/server.js" \
  --from examples/tide-demo-agent/policy.md
```

That writes:

```text
/tmp/tide-policy/
  ontology/actions.yaml
  policies/rules.policy.yaml
  tests/golden.scenarios.yaml
  findings.md
```

Review the generated files, then validate them:

```bash
.venv/bin/tide compile --project /tmp/tide-policy
.venv/bin/tide test --project /tmp/tide-policy
```

## Enforce Calls

Start the runtime:

```bash
.venv/bin/tide run --project /tmp/tide-policy
```

The runtime binds to `127.0.0.1` by default. It refuses non-loopback hosts unless
you pass `--unsafe-bind-public`; only use that on a trusted network.

Put the gateway in front of the MCP server:

```bash
node adapters/mcp-gateway-js/index.js run \
  --mode enforce \
  --upstream "node examples/mcp-test-agent/server.js"
```

Behavior:

- `allow`: forwarded to the MCP server.
- `deny`: not forwarded; the agent receives a blocked tool result with the reason.
- `escalate`: held for human approval in the runtime console.

## Demo Smoke

```bash
node examples/tide-demo-agent/agent.mjs demo
```

Expected final line:

```text
tide demo agent smoke ok
```

## Security Model

- The runtime is local-only and unauthenticated in this preview.
- MCP upstream commands are trusted local commands. Tide launches them as local
  processes; do not point Tide at untrusted config.
- Upstream MCP processes receive a minimal environment by default. Set
  `TIDE_INHERIT_ENV=1` only when a trusted upstream needs your full environment.
- Policy project `resolvers.js` files are trusted code and execute during
  compile/test/run.
- Decision traces redact common secret-shaped keys and token values, but callers
  should still avoid sending secrets in tool params.

See [SECURITY.md](SECURITY.md) for reporting and trust-boundary details.

## Release Check

```bash
scripts/check-release.sh
```

The check runs runtime tests, gateway tests, package dry-runs, a Python wheel
build, production dependency audit for the runtime, and the demo smoke.

## Runtime API

| Endpoint | Purpose |
|---|---|
| `POST /v1/decisions` | authorize one proposed tool action |
| `GET /v1/decisions/:trace_id` | poll escalated decisions |
| `GET /v1/approvals?status=pending` | approval queue |
| `POST /v1/approvals/:id` | approve or reject a held action |
| `GET /v1/traces` | decision feed |
| `GET /v1/bundle` | active policy bundle summary |
| `GET /` | local console |

## Repo Layout

```text
adapters/mcp-gateway-js/  MCP discovery, rule generation, and tool-call gateway
apps/runtime/            deterministic decision engine, HTTP API, local console
cli/                     compile/test/run commands for policy projects
examples/mcp-test-agent/ tiny MCP server used for smoke tests
examples/tide-demo-agent product-loop smoke test
```
