![Rippletide](./assets/Rippletide_github.jpeg)

<p align="center">
  <strong>Rippletide adds an authority layer that validates, constrains, or blocks agent actions at runtime before they impact real systems or customers.</strong>
</p>

<p align="center">
  <a href="https://app.rippletide.com">Web Platform</a>
  ·
  <a href="https://github.com/rippletideco/rippletide">GitHub</a>
  ·
  <a href="https://docs.rippletide.com">Documentation</a>
  ·
  <a href="https://discord.gg/zUPTRH5eFv">Discord</a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/rippletide"><img src="https://img.shields.io/npm/v/rippletide?style=flat-square&logo=npm" alt="npm" /></a>
  <a href="https://www.npmjs.com/package/rippletide"><img src="https://img.shields.io/npm/dm/rippletide?style=flat-square&logo=npm" alt="Downloads" /></a>
  <a href="https://github.com/rippletideco/rippletide/stargazers"><img src="https://img.shields.io/github/stars/rippletideco/rippletide?style=flat-square&logo=github" alt="Stars" /></a>
  <a href="https://github.com/rippletideco/rippletide/issues"><img src="https://img.shields.io/github/issues/rippletideco/rippletide?style=flat-square&logo=github" alt="Issues" /></a>
  <a href="https://discord.gg/zUPTRH5eFv"><img src="https://img.shields.io/badge/Discord-Join_us-7289DA?style=flat-square&logo=discord" alt="Discord" /></a>
</p>


<br />

This repository is your starting point for Rippletide — Eval, Context Graph MCP, and Coding Agents.

---

## Table of Contents

- [What is Rippletide?](#what-is-rippletide)
- [Rippletide Platform](#rippletide-platform)

**Core Modules:**


| #   | Module                                          | What it does                                       |
| --- | ----------------------------------------------- | -------------------------------------------------- |
| 1   | [Agent Evaluation - CLI](#agent-evaluation-cli) | Validate before you ship                           |
| 2   | [Context Graph - MCP](#context-graph---mcp)     | Give your agents persistent memory across sessions |
| 3   | [Coding Agents](#coding-agents)                 | A persistent memory layer for Claude               |


<sub>**Decision Runtime** — Enterprise Only. Build deterministic agents with less than 1% hallucination rate. [Contact us](https://rippletide.com) to learn more.</sub>

---

## What is Rippletide?

Rippletide adds an authority layer that validates, constrains, or blocks agent actions at runtime before they impact real systems or customers.


|                    | Without Rippletide           | With Rippletide               |
| ------------------ | ---------------------------- | ----------------------------- |
| **Hallucinations** | Variable, hard to control    | Less than 1% by design        |
| **Memory**         | Lost between conversations   | Persistent context graph      |
| **Guardrails**     | Prompt-based, easy to bypass | Engine-level, 100% compliance |
| **Explainability** | Black box                    | Every decision is traceable   |
| **Evaluation**     | Manual spot checks           | Automated, CI-ready testing   |


---

## Agent Evaluation CLI

Eval is the entry point to Rippletide. Before adding memory or decision runtime, start by testing what your agent already does. Plug your agent into our CLI and Rippletide auto-generates test questions to evaluate its responses. Rippletide spots hallucinations by fact-checking each output and suggests improvements when sources are missing.

**When to use it:** Before every deployment, in CI pipelines, and during development to catch regressions.

<p align="center">
  <img src="https://raw.githubusercontent.com/rippletideco/rippletide/main/assets/demo.gif" alt="Agent Evaluation Demo" width="800">
</p>



### Installation

Install globally via npm:

```bash
npm install -g rippletide
```

Or use directly with npx:

```bash
npx rippletide
```

### Quick Start

Simply run:

```bash
rippletide
```

You'll be prompted for:

1. **Agent endpoint** — Your API URL (e.g. `http://localhost:8000`)
2. **Knowledge source** — Choose between files, Pinecone, or PostgreSQL

The CLI will then:

- Load your test questions
- Send them to your agent
- Show real-time progress
- Display evaluation results with pass/fail and justifications

### Command Line Options

```bash
rippletide eval [options]
```


| Option                     | Description                                      | Example                              |
| -------------------------- | ------------------------------------------------ | ------------------------------------ |
| `-t, --template <name>`    | Use a pre-configured template                    | `rippletide eval -t banking_analyst` |
| `-a, --agent <url>`        | Agent endpoint URL                               | `rippletide eval -a localhost:8000`  |
| `-k, --knowledge <source>` | Knowledge source: files, pinecone, or postgresql | `rippletide eval -k pinecone`        |
| `--debug`                  | Show detailed error information                  | `rippletide eval --debug`            |
| `-h, --help`               | Show help message                                | `rippletide --help`                  |


### Data Source Options

**Local Files (default):**

```bash
rippletide eval -a localhost:8000
```

Reads Q&A pairs from `qanda.json` in the current directory.

**Pinecone:**

```bash
rippletide eval -a localhost:8000 -k pinecone \
  -pu https://db.pinecone.io \
  -pk pcsk_xxxxx
```

**PostgreSQL:**

```bash
rippletide eval -a localhost:8000 -k postgresql \
  -pg "postgresql://user:pass@localhost:5432/db"
```

### Custom Endpoint Options

For non-standard APIs:

```bash
rippletide eval -a localhost:8000 \
  -H "Authorization: Bearer token, X-API-Key: key" \
  -B '{"prompt": "{question}"}' \
  -rf "data.response"
```


| Option                  | Description                                          |
| ----------------------- | ---------------------------------------------------- |
| `-H, --headers`         | Custom headers (comma-separated)                     |
| `-B, --body`            | Request body template (use `{question}` placeholder) |
| `-rf, --response-field` | Path to response in JSON (dot notation)              |


### Templates

Pre-built configurations for common agent use cases:


| Template            | Description                 |
| ------------------- | --------------------------- |
| `banking_analyst`   | Financial Q&A agent         |
| `customer_service`  | Support agent testing       |
| `blog_to_linkedin`  | Content repurposing agent   |
| `luxe_concierge`    | Luxury services agent       |
| `local_dev`         | Local development agent     |
| `openai_compatible` | OpenAI-compatible endpoints |
| `project_manager`   | Project management agent    |


```bash
rippletide eval -t customer_service
```

→ [Full Evaluation docs](https://docs.rippletide.com/docs/evaluation_overview)

---

## Context Graph - MCP

A persistent memory layer for your AI agents. Connect any MCP-compatible client (Cursor, Claude Desktop, Claude Code) and your agent can remember facts, decisions, and context across sessions.

### Quick Start

Add this to your MCP client config:

```json
{
  "mcpServers": {
    "rippletide": {
      "type": "url",
      "url": "https://mcp.rippletide.com/mcp?agentId=your-agent-id"
    }
  }
}
```

Get your `agentId` from the [Rippletide platform](https://app.rippletide.com).

| Client | Config location |
|--------|----------------|
| Cursor | `~/.cursor/mcp.json` |
| Claude Desktop | MCP settings in the app |
| Claude Code | `.mcp.json` at project root |

→ [MCP docs](https://docs.rippletide.com/docs/mcp/overview)

---

## Coding Agents

Give Claude Code a shared, persistent memory. Store your team's conventions once — naming rules, architecture decisions, error handling policies — and every Claude session pulls from the same source automatically.

### Quick Start

```bash
npx rippletide-code
```

One command to authenticate, scan your repo, select rules, and install hooks. Every Claude Code session in this project will have access to your rules from that point on.

If you need to point the CLI at a specific coding-agent backend, set one base URL and the CLI will derive the related endpoints from it:

```bash
RIPPLETIDE_API_URL="https://coding-agent.up.railway.app" npx rippletide-code
```

### Setup Flow

The CLI now has two setup flows.

**Individual**

- Start the CLI and choose `1. Individual workspace`
- The CLI uses the standard Rippletide cloud login flow
- After login, rules sync with the individual workspace

**Enterprise**

- Point the CLI at a client-hosted coding-agent backend:

```bash
RIPPLETIDE_API_URL="https://company-coding-agent.internal" npx rippletide-code
```

- Start the CLI and choose `2. Enterprise backend`
- The CLI stores that backend as the active coding-agent backend for later runs
- The enterprise backend handles uploads, rule extraction, and Anthropic-backed processing

In practice:

- Individual = Rippletide cloud auth + cloud workspace
- Enterprise = company backend + enterprise-local coding-agent flow

For the maintainer release flow for the internal CLI channel, see [context-graph/CLI_RELEASE_FLOW.md](./context-graph/CLI_RELEASE_FLOW.md).

### Features

| Feature | What it does |
|---|---|
| **Rule enforcement** | Rules are injected into every prompt. Code that violates a rule is blocked before it hits the file. Claude auto-rewrites until it complies. |
| **Rule management** | Add, edit, or delete rules in natural language. No config files. Changes take effect immediately. |
| **Rule sharing** | Send your rule set to a colleague with `invite-rules <email>`. They type `receive-rules <otp>` and get a conflict report with new rules, duplicates, and contradictions. |
| **Planning** | `/plan` generates an implementation plan and reviews it against your rules. Violations are revised automatically until the plan passes. |
| **Team governance** | Create a team, push your rules, and have every engineer sync from the same source. Read-only mode lets engineers inherit standards without modifying them. |

### Commands

| Action | Command |
|---|---|
| Share rules | `invite-rules <email>` |
| Receive rules | `receive-rules <otp>` |
| Create team | `create-team <name>` |
| Join team | `join-team <name> [approver_email]` |
| Approve member | `approve-join <team> <otp> <email>` |
| Push to team | `push-rules <team>` |
| Sync from team | `sync-rules <team>` |
| Read-only connect | `npx rippletide-code --read-only` |

### Data privacy

Rippletide only relies on the context available inside your local Claude Code workflow — your current chat session and your `CLAUDE.md`. Your codebase is not centrally analyzed. Rippletide stores only the extracted rules, not project content.

→ [Coding Agents docs](https://docs.rippletide.com/docs/coding-agents/overview)

---

## Rippletide Platform

The [Rippletide Platform](https://app.rippletide.com) brings everything together. Build agents without writing code, connect your knowledge sources, set guardrails that the LLM cannot override, and see exactly how your agent reasons through every decision — all in one place.

- **Visual Agent Builder** — configure agents without code
- **Knowledge Connectors** — import from Amazon Bedrock, PDFs, or manual Q&A
- **Knowledge Visualization** — see your agent's full knowledge graph
- **Guardrail Configuration** — rules enforced at runtime, not in the prompt

---

## Repository Structure

```
rippletide/
├── agent-evaluation/       # TypeScript CLI for agent evaluation
│   ├── bin/rippletide      # CLI entry point
│   ├── src/                # Source (api, components, errors, utils)
│   └── templates/          # Pre-built agent configs
├── context-graph/          # Rust MCP server for coding agents
│   ├── src/                # Rust source
│   └── npm/                # Multi-platform binary packages
└── docs/                   # Documentation site (Mintlify)
```

---

## Development

```bash
git clone https://github.com/rippletideco/rippletide.git

# Agent Evaluation CLI
cd rippletide/agent-evaluation
npm install
npm run build
npm run eval         # run development version

# Context Graph MCP server
cd rippletide/context-graph
cargo build --release

```

---

## Contributing

We welcome contributions. Please read our [Contributing Guidelines](./CONTRIBUTING.md), [Code of Conduct](./CODE_OF_CONDUCT.md), and [Security Policy](./SECURITY.md) before opening a PR.

---

## Support

- **Discord**: [Join our community](https://discord.gg/zUPTRH5eFv)
- **GitHub Issues**: [Report bugs](https://github.com/rippletideco/rippletide/issues)
- **Docs**: [docs.rippletide.com](https://docs.rippletide.com)

---

Built with ❤️ by the [Rippletide](https://rippletide.com) team
