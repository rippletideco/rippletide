![Rippletide](./assets/Rippletide_github.jpeg)

# Rippletide

Rippletide connects to agents you already run, records runtime evidence, and evaluates business rules before important actions or response delivery. Start in Observe, review real matches, then deliberately enable Enforce on the releases you trust.

- [Platform](https://app.rippletide.com)
- [Documentation](https://docs.rippletide.com)
- [TypeScript SDK and CLI](https://www.npmjs.com/package/rippletide-package)
- [Python SDK](https://pypi.org/project/rippletide-python/)

## Connect an existing agent

For a JavaScript or TypeScript agent on Node.js 18+:

```bash
npm install -g rippletide-package
rippletide login
rippletide connect --kind agent
```

Apply the generated setup prompt in your repository, start your application normally, and run a real tool-using turn that delivers a response. Check the inventory in **Context & actions** and the run's decisions and delivery outcome in **Traces**:

```bash
rippletide verify
rippletide events --wait
```

For Python, use [Connect Python](https://docs.rippletide.com/docs/connect-python). JavaScript/TypeScript MCP servers use [Connect an MCP server](https://docs.rippletide.com/docs/connect-mcp-server).

To govern OpenAI Codex itself through supported local hooks, use `rippletide codex setup` and read the [Codex coverage and privacy guide](https://docs.rippletide.com/docs/connect-codex). This is a different path from using a coding agent to instrument your application.

## Review evidence before enforcement

[Business context](https://docs.rippletide.com/docs/business-context) connects documents, Notion snapshots and recorded activity to proposed Rules. Review the source passages, cases and required setup, then [test in Observe](https://docs.rippletide.com/docs/evaluate-improve).

An enforced blocking match must be honored by the real tool or response-delivery boundary. The caller reports whether the action executed, the response was delivered, or the effect was prevented. A policy decision alone is not proof that an effect happened.

Read [Observe & enforce](https://docs.rippletide.com/docs/observe-and-enforce) and [Privacy & capture](https://docs.rippletide.com/docs/privacy) for behavior and data-handling defaults. Runtime credentials stay connection-scoped; never install a Platform key in an agent.

## Repository and documentation

Customer-facing documentation is maintained in `docs/`. Mintlify publishes that directory from this repository's `main` branch to [docs.rippletide.com](https://docs.rippletide.com).

```text
rippletide/
├── context-graph/    # Existing packages and runtime components
└── docs/             # Public MDX pages and docs.json navigation
```

From `docs/`, use the Mintlify CLI to validate and preview documentation:

```bash
npx mint validate
npx mint broken-links
npx mint dev
```

Check examples against the current published SDK/CLI and actual platform contracts before editing. Preserve existing public URLs or add redirects in `docs/docs.json`. Keep support limits, capture defaults and required runtime proof explicit. A successful documentation build does not by itself validate the integration described.

## Contributing and support

Read [Contributing](./CONTRIBUTING.md), [Code of Conduct](./CODE_OF_CONDUCT.md), and [Security](./SECURITY.md).

For help, use the [documentation](https://docs.rippletide.com), [GitHub issues](https://github.com/rippletideco/rippletide/issues), or [community Discord](https://discord.gg/zUPTRH5eFv).
