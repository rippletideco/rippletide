# Tide v0.1.0-preview

Tide is ready for a public technical preview, not a production v1.

## What Is Included

- MCP gateway for pre-execution policy decisions.
- Local decision runtime and console.
- CLI commands for compile/test/run of policy projects.
- Local demo MCP server and smoke test.

## Known Limits

- MCP tool calls only.
- Runtime state is local and in-memory except the decision log.
- Runtime APIs are unauthenticated and local-only by default.
- MCP upstream commands and policy `resolvers.js` files are trusted local code.
- Generated policies must be reviewed before enforce mode.

## Release Gate

Run:

```bash
scripts/check-release.sh
```

Expected proof:

- runtime tests pass;
- gateway tests pass;
- both npm packages dry-run pack;
- the Python CLI builds a wheel;
- the runtime dependency audit has no production vulnerabilities;
- the demo blocks `rippletide-deny-test` before the MCP server executes.

## Launch Copy

Short:

> Tide adds deterministic pre-execution guardrails to MCP tool calls. Same agent,
> same tool, but unsafe calls are denied before the MCP server executes.

Show HN title:

> Show HN: Tide, deterministic guardrails before MCP tool calls execute

First paragraph:

> Agents are getting real tools. Prompt guardrails help, but they do not stop a
> tool call once the agent decides to execute it. Tide sits between an MCP client
> and MCP server, evaluates every `tools/call`, then returns `allow`, `deny`, or
> `escalate` before the real tool runs.

## Publishing Notes

The current private repository history contains prototype fixtures and internal
testbed material. For a public launch, publish from a squashed clean main branch
or a fresh public repository initialized from the reviewed release tree.

npm and PyPI publishing are intentionally deferred for this preview.
