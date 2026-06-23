# Security Policy

Tide is a public technical preview. Treat the runtime and policy projects as
local, trusted developer tooling unless a later release states otherwise.

## Supported Versions

Only the current `main` branch is supported during the preview.

## Reporting Vulnerabilities

Email security reports to security@rippletide.co. Do not open public GitHub
issues for vulnerabilities.

Include:

- affected commit or release;
- reproduction steps;
- expected impact;
- any logs or policy files needed to reproduce, with secrets removed.

## Trust Boundaries

- The runtime is unauthenticated and refuses non-loopback hosts unless
  `--unsafe-bind-public` is passed.
- MCP upstream commands are trusted local commands.
- Policy project `resolvers.js` files are trusted code and run during
  compile/test/run.
- `setup` may send policy text and tool schemas to OpenAI when
  `OPENAI_API_KEY` is configured.
- Decision traces redact common secret-shaped keys and token values, but policy
  authors should still avoid sending secrets in tool params.
