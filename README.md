<img width="2000" alt="Rippletide" src="./Rippletide_github.jpeg" />

<p align="center">
  <strong>Rippletide is an authority layer for AI agents — evaluate responses, persist context, and run deterministic decisions with full traceability.</strong>
</p>

<p align="center">
  <a href="https://trust.rippletide.com">Web Platform</a>
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

---

## Table of Contents

- [What is Rippletide?](#what-is-rippletide)
- [Trust Platform](#trust-platform)

**Core Modules:**
| # | Module | What it does |
|---|--------|-------------|
| 1 | [Agent Evaluation CLI](#agent-evaluation-cli) | CLI tool to test and validate AI agent responses |
| 2 | [Context Graph](#context-graph) | Persistent memory and rules for AI agents |
| | &nbsp;&nbsp;↳ [Coding Agents](#use-case--coding-agents) | Shared conventions for Claude Code |
| | &nbsp;&nbsp;↳ [MCP](#mcp) | Persistent isolated memory for any agent `Enterprise` |
| 3 | [Decision Runtime](#decision-runtime) | Deterministic agents, <1% hallucination `Enterprise` |

---

## What is Rippletide?

Rippletide is an authority layer that sits between your AI agents and your users. It validates, constrains, and traces agent actions at runtime — replacing fragile prompt-based guardrails with an engine-level decision system.

| | Without Rippletide | With Rippletide |
|---|---|---|
| Hallucinations | Variable | <1% |
| Memory | Lost between sessions | Persistent context graph |
| Guardrails | Prompt-based | Runtime enforcement |
| Explainability | Black box | Fully traceable |

---

## Agent Evaluation CLI

A CLI tool for testing and validating AI agent responses directly from your terminal. Point it at any agent endpoint, provide your Q&A pairs, and get instant pass/fail results with justifications — no custom scripts needed.

> **Note:** Evaluation is also available via the [API](https://docs.rippletide.com/api-reference/introduction) and the [Trust Platform](https://trust.rippletide.com). This module covers the CLI only.

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

| Option | Description | Example |
|--------|-------------|---------|
| `-t, --template <name>` | Use a pre-configured template | `rippletide eval -t banking_analyst` |
| `-a, --agent <url>` | Agent endpoint URL | `rippletide eval -a localhost:8000` |
| `-k, --knowledge <source>` | Knowledge source: files, pinecone, or postgresql | `rippletide eval -k pinecone` |
| `--debug` | Show detailed error information | `rippletide eval --debug` |
| `-h, --help` | Show help message | `rippletide --help` |

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

| Option | Description |
|--------|-------------|
| `-H, --headers` | Custom headers (comma-separated) |
| `-B, --body` | Request body template (use `{question}` placeholder) |
| `-rf, --response-field` | Path to response in JSON (dot notation) |

### Templates

Pre-built configurations for common agent use cases:

| Template | Description |
|----------|-------------|
| `banking_analyst` | Financial Q&A agent |
| `customer_service` | Support agent testing |
| `blog_to_linkedin` | Content repurposing agent |
| `luxe_concierge` | Luxury services agent |
| `local_dev` | Local development agent |
| `openai_compatible` | OpenAI-compatible endpoints |
| `project_manager` | Project management agent |

```bash
rippletide eval -t customer_service
```

→ [Full Evaluation docs](https://docs.rippletide.com/docs/evaluation_overview)

---

## Context Graph

Persistent, shared memory for AI agents. Store your rules and knowledge once — every agent session reads from the same source of truth.

### Use Case — Coding Agents

Every engineer on your team has their own `CLAUDE.md`. They're all different, all outdated, and Claude ignores half of them anyway. When someone defines a good convention, it stays on their machine.

The Context Graph gives Claude Code a shared, external memory. Your team defines the rules once — naming conventions, architecture decisions, error handling policies — and every Claude session pulls from the same source automatically. No copy-pasting. No drift between engineers. No more "why did Claude do it differently this time?"

→ [Coding Agents docs](https://docs.rippletide.com/docs/coding-agents/overview)

### MCP

Your agents forget everything between sessions. The MCP layer fixes that — rules, context, and conventions stored once and available to every agent, every time.

> **Enterprise only** — [Contact us](https://rippletide.com) to learn more.

---

## Decision Runtime

Build AI agents that never hallucinate. The Decision Runtime replaces probabilistic LLM reasoning with a deterministic engine — agents that follow your business logic exactly, every time, with full traceability.

> **Enterprise only** — [Contact us](https://rippletide.com) to learn how we can bring this to your team.

---

## Trust Platform

All three modules are accessible through the [Trust Platform](https://trust.rippletide.com) — a unified web dashboard:

- **Visual Agent Builder** — build and configure agents without code
- **Knowledge Connectors** — import from Amazon Bedrock, PDFs, or manual Q&A
- **Knowledge Visualization** — interactive graph view of your agent's knowledge
- **Guardrail Configuration** — set engine-level rules that the LLM cannot override
- **MCP Integration** — expose agents directly to Claude

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
├── decision-runtime/       # Runtime layer
│   ├── playground-proxy/   # Node.js proxy server
│   └── rippletide_client/  # Python SDK
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

# Playground Proxy
cd rippletide/decision-runtime/playground-proxy
npm install
npm start
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
