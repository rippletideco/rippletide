<img width="2000" height="491" alt="Rippletide" src="./assets/banner.png" />

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
| 1 | [Agent Evaluation](#agent-evaluation) | Test and validate AI agent responses |
| 2 | [Context Graph](#context-graph) | Persistent memory and rules for coding agents |
| 3 | [Decision Runtime](#decision-runtime) | Deterministic agents with <1% hallucination |

---

## What is Rippletide?

Rippletide is an authority layer that sits between your AI agents and your users. It validates, constrains, and traces agent actions at runtime — replacing fragile prompt-based guardrails with an engine-level decision system.

| | Without Rippletide | With Rippletide |
|---|---|---|
| Hallucinations | Variable | <1% |
| Memory | Lost between sessions | Persistent context graph |
| Guardrails | Prompt-based | Engine-level enforcement |
| Explainability | Black box | Fully traceable |

---

## Agent Evaluation

Test and validate AI agent responses before and after deployment. The evaluation CLI sends your Q&A pairs to any agent endpoint, fact-checks responses against expected answers, and reports pass/fail with justifications.

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

A Rust-based MCP (Model Context Protocol) server that gives coding agents — Claude Code, Cursor, Codex — a persistent, shared source of truth for your codebase rules and conventions.

### Demo

<p align="center">
  <video autoplay muted loop playsinline controls width="800" src="./docs/img/coding-agents-demo.mp4"></video>
</p>

### The Problem

Local rule files like `CLAUDE.md` and `.cursorrules` don't scale:
- Siloed per engineer, never shared
- Go stale and are rarely updated
- Lost between sessions
- No enforcement — agents can ignore them

The Context Graph solves this by storing your rules externally and injecting them automatically into every agent session.

### Getting Started

```bash
npx rippletide-code@latest connect
```

This command:
1. Authenticates with your Rippletide account
2. Generates `.mcp.json` for Claude Code and Cursor
3. Generates `.codex/config.toml` for Codex
4. Creates `CLAUDE.md` / `AGENTS.md` with hook-first planning instructions

From that point on, every coding agent session automatically receives your team's rules before generating any code or plan.

### Supported Clients

| Client | Config file generated |
|--------|-----------------------|
| Claude Code | `.mcp.json` |
| Cursor | `.mcp.json` |
| Claude Desktop | MCP settings |
| VS Code | `.mcp.json` |
| Codex | `.codex/config.toml` |

→ [Context Graph docs](https://docs.rippletide.com/docs/mcp/overview) · [Coding Agents docs](https://docs.rippletide.com/docs/coding-agents/overview)

---

## Decision Runtime

Build deterministic agents with a hypergraph reasoning engine. The LLM handles language only — input understanding and output generation. All decisions are made by the engine using a structured knowledge graph of Q&A pairs, tags, actions, and state predicates.

The result: agents that follow your business logic exactly, with <1% hallucination rate and full traceability.

### Playground Proxy

A lightweight Node.js proxy server for the Rippletide MCP playground. Deployable to Vercel or Heroku.

```bash
cd decision-runtime/playground-proxy
npm install
npm start
```

Configure via environment variables:

```bash
cp .env.example .env
# Set RIPPLETIDE_API_KEY and other vars
```

### Python SDK

A Python client for the Rippletide evaluation and knowledge APIs.

**Installation:**
```bash
pip install -r decision-runtime/rippletide_client/requirements.txt
```

**Basic usage:**
```python
from rippletide_sdk import RippletideClient

client = RippletideClient(api_key="your-api-key")

# Create an agent
agent = client.create_agent(name="My Eval Agent")
agent_id = agent['id']

# Extract Q&A pairs from a PDF
result = client.extract_questions_from_pdf(
    agent_id=agent_id,
    pdf_path="path/to/document.pdf"
)

# Evaluate a response
report = client.evaluate(
    agent_id=agent_id,
    question="What is this document about?",
    expected_answer="Optional expected answer"
)

print(f"Label: {report['label']}")
print(f"Justification: {report['justification']}")
```

→ [Decision Runtime docs](https://docs.rippletide.com/docs/hypergraph_overview)

---

## Trust Platform

All three modules are accessible through the [Trust Platform](https://trust.rippletide.com) — a unified web dashboard:

- **Visual Agent Builder** — build and configure agents without code
- **Knowledge Connectors** — import from Amazon Bedrock, PDFs, or manual Q&A
- **Knowledge Visualization** — interactive graph view of your agent's knowledge
- **Guardrail Configuration** — set engine-level rules that the LLM cannot override
- **MCP Integration** — expose agents directly to Cursor, Claude, and VS Code

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
