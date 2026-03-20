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
- [Trust Platform](#trust-platform)

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
npx rippletide-code@latest connect
```

This sets up Claude Code hooks and generates the agent instruction files. After running:

```
your-project/
├── .claude/settings.json  # Claude Code hooks
└── CLAUDE.md              # Agent instructions
```

### Data privacy

A common question is what Rippletide actually sees when used with Coding Agents.

For Coding Agents, Rippletide only relies on the context available inside your local Claude Code workflow:
- the current Claude Code chat session for the active project
- your `CLAUDE.md`

Your codebase is not centrally analyzed outside of your local Claude Code environment. The analysis happens through your own Claude Code setup.

Rippletide only stores the rules extracted from that context, not the rest of the project content. In practice, this means Rippletide keeps reusable conventions and policies, not your full codebase or unrelated session content.

→ [Coding Agents docs](https://docs.rippletide.com/docs/coding-agents/overview)

---

## Trust Platform

The [Trust Platform](https://app.rippletide.com) brings everything together. Build agents without writing code, connect your knowledge sources, set guardrails that the LLM cannot override, and see exactly how your agent reasons through every decision — all in one place.

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
