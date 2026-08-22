![Rippletide](./assets/Rippletide_github.jpeg)

# Rippletide

## Ground every sensitive agent action and approval to reduce human intervention

Define checks before sensitive tool calls or MCP calls.

Rippletide evaluates each sensitive action against your business rules and current business context before execution:

- **Allow** — the action can proceed.
- **Block** — the action is prevented, and the reason is sent back to the agent orchestrator.
- **Escalate** — the action is sent to a person for review.

Use Rippletide through the visual web platform or connect it directly from your repository with a coding agent.

<p align="center">
  <a href="https://app.rippletide.com">Web Platform</a>
  ·
  <a href="https://docs.rippletide.com">Documentation</a>
  ·
  <a href="https://www.npmjs.com/package/rippletide-package">npm</a>
  ·
  <a href="https://discord.gg/zUPTRH5eFv">Discord</a>
</p>

---

## Why Rippletide?

AI agents can already call APIs, update records, issue refunds, change orders, approve requests, and trigger production workflows.

But sensitive actions still require human review when the agent cannot reliably determine:

- whether an action is allowed;
- which business rule applies;
- whether the information is current;
- when an exception requires approval;
- why an action was allowed or blocked.

Prompt instructions alone are not enough for these decisions. They can be incomplete, outdated, or bypassed by the agent.

Rippletide places enforceable checks between the agent and its tools.

```text
Agent or MCP client
        ↓
Sensitive tool or MCP call
        ↓
Rippletide check
        ↓
Allow / Block / Escalate
        ↓
Tool, API, CRM, or production system
```

The agent handles routine cases. People keep the exceptions.

---

## Get started

You can use Rippletide in two ways.

### Option 1: Use the web platform

Open the [Rippletide Platform](https://app.rippletide.com) and follow the guided connection flow.

From the visual interface, you can:

- connect an existing agent or MCP server;
- inspect its prompts, tools, and connections;
- identify sensitive tool and MCP calls;
- define and test checks;
- release checks in Observe or Enforce mode;
- review runtime outcomes and their reasons.

Rippletide connects to the agent you already operate. You do not need to rebuild it or replace its existing infrastructure.

### Option 2: Connect with a coding agent

> Supported today: existing Node.js 18+ JavaScript or TypeScript agents and MCP servers.

From the repository that contains your agent:

```bash
npm install -g rippletide-package
rippletide login
rippletide connect
```

The CLI creates or links the agent and generates a setup prompt specific to the repository.

Paste this prompt into Codex, Claude Code, Cursor, or your preferred coding agent. It applies the required instrumentation while keeping your existing agent, orchestration, credentials, and execution environment.

---

## Protect your first sensitive action

The web platform and coding agent setup lead to the same runtime workflow.

### 1. Run a real tool-using turn

Start your agent normally and send it a request that causes it to call a tool.

If you connected through the CLI, verify that Rippletide received the agent’s configuration and runtime activity:

```bash
rippletide verify
rippletide events --wait
```

In **Harness management**, you can inspect the agent’s identity, prompts, tools, and connections.

### 2. Identify sensitive actions

Review the agent’s tools and MCP calls and select the actions that require checks.

Examples include:

- issuing a refund;
- changing a price or discount;
- modifying an order;
- approving a request;
- updating a customer record;
- sending an external message;
- triggering a production workflow.

### 3. Define a check

Attach an explicit business rule to a sensitive tool or MCP call.

For example:

```text
Allow a refund when the amount is below €100
and the customer is eligible under the current refund policy.

Escalate the refund when the amount is €100 or more.

Block the refund when the order is outside the allowed refund period.
```

Checks can be grounded in the business information that supports them, including the source, version, and approval status.

### 4. Test the check in Observe mode

Release the check in **Observe** mode and run the action again.

The action still proceeds, while Rippletide records:

- which action was requested;
- which check applied;
- what the outcome would have been;
- why that outcome was produced.

This lets you validate the check against real agent activity before changing its behavior.

### 5. Enforce Allow, Block, or Escalate

When the check behaves correctly, release it in **Enforce** mode.

| Outcome | What happens |
| --- | --- |
| **Allow** | The action can proceed. |
| **Block** | The action is prevented, and the reason is sent back to the agent orchestrator. |
| **Escalate** | The action is sent to a person for review. |

This reduces routine human review while keeping people responsible for exceptions and sensitive decisions.

### 6. Trace the outcome

Each decision produces runtime evidence showing:

- the requested action;
- the relevant inputs;
- the check that was evaluated;
- the resulting outcome;
- the reason for that outcome.

This makes every sensitive agent action inspectable and explainable.

Read the complete [Quickstart](https://docs.rippletide.com/docs/quickstart).

---

## Business rules, not prompt guardrails

Rippletide checks run at the action boundary, before a sensitive call reaches the target system.

This keeps the decision separate from the agent’s own reasoning and prompt.

A check can use:

- the requested action;
- the tool arguments;
- runtime context;
- relevant business information;
- explicit limits and conditions;
- approval requirements.

The result is a traceable **Allow**, **Block**, or **Escalate** outcome.

---

## Keep checks aligned with the business

Business rules change. Policies are updated, contracts are revised, approval limits move, and exceptions are introduced.

Rippletide connects each check to the business information that supports it, so teams can understand:

- where the check came from;
- which version was used;
- who approved it;
- which sensitive actions depend on it.

This keeps operational decisions grounded in the business context that applies when the action is requested.

---

## Connect an MCP server

MCP servers follow the same workflow:

1. connect the MCP server;
2. inspect the exposed tools;
3. identify sensitive calls;
4. define checks;
5. test them in Observe mode;
6. enforce Allow, Block, or Escalate outcomes;
7. inspect the runtime evidence.

See [Connect an MCP server](https://docs.rippletide.com/docs/connect-mcp-server).

---

## Trust and data boundaries

Rippletide connects to the agent you already operate.

- Your agent remains in its existing infrastructure.
- Its model and tool credentials remain under your control.
- Rippletide connection keys are stored locally and must remain git-ignored.
- Checks are evaluated at the boundary of sensitive tool and MCP calls.
- Runtime events provide evidence for each outcome.

Never commit Rippletide connection keys or your agent’s secrets.

---

## Documentation

- [What is Rippletide?](https://docs.rippletide.com/)
- [Quickstart](https://docs.rippletide.com/docs/quickstart)
- [Connect an agent](https://docs.rippletide.com/docs/connect-agent)
- [Connect an MCP server](https://docs.rippletide.com/docs/connect-mcp-server)
- [Observe and Enforce](https://docs.rippletide.com/docs/observe-and-enforce)
- [Write rules](https://docs.rippletide.com/docs/write-rules)
- [Runtime events](https://docs.rippletide.com/docs/runtime-events)
- [CLI reference](https://docs.rippletide.com/docs/cli)

---

## Repository structure

Customer-facing documentation is maintained in `docs/` and published at [docs.rippletide.com](https://docs.rippletide.com).

```text
rippletide/
├── context-graph/    # Rippletide packages and runtime components
└── docs/             # Public documentation
```

---

## Contributing

Contributions are welcome.

Before opening a pull request, read:

- [Contributing Guidelines](./CONTRIBUTING.md)
- [Code of Conduct](./CODE_OF_CONDUCT.md)
- [Security Policy](./SECURITY.md)

---

## Support

- [Documentation](https://docs.rippletide.com)
- [Discord community](https://discord.gg/zUPTRH5eFv)
- [GitHub issues](https://github.com/rippletideco/rippletide/issues)

---

Built by the [Rippletide](https://rippletide.com) team.
