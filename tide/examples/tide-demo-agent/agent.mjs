#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defaultMcpConfigs, mcpServerToUpstream, readMcpConfig } from "../../adapters/mcp-gateway-js/mcp-config.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const gateway = path.join(root, "adapters/mcp-gateway-js/index.js");
const defaultPolicy = path.join(root, "examples/tide-demo-agent/policy.md");

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});

async function main() {
  const { command, opts } = parseArgs(process.argv.slice(2));
  opts.mcpConfig ??= defaultMcpConfigs().find((file) => file.includes(".codex"));
  if (command === "demo") return run(process.execPath, [path.join(root, "examples/tide-demo-agent/smoke.mjs")], { env: childEnv({ OPENAI_API_KEY: "" }) });
  if (command === "preview") return preview(opts);
  if (command === "setup") return setup(opts);
  if (command === "call") return call(opts);
  if (command === "ask") return ask(opts);
  throw new Error("usage: agent.mjs demo|preview|setup|call|ask --project <dir> [--mcp-config <file>]");
}

function preview(opts) {
  const project = opts.project ?? "./tide";
  const config = opts.mcpConfig ?? "~/.codex/config.toml";
  console.log([
    "Tide agent UX preview",
    "",
    "1. Generate guardrails from your MCP tools and policy text",
    `   node examples/tide-demo-agent/agent.mjs setup --project ${project} --mcp-config ${config}`,
    "   UI: select suggested rules with arrow keys, space toggles, enter confirms.",
    "",
    "2. Run the generated policy tests",
    `   .venv/bin/tide test --project ${project}`,
    "   UI: pass/fail list before any real agent call is enforced.",
    "   Findings: read findings.md for concrete blocked calls and undecided risky tools.",
    "",
    "3. Call the agent through Tide enforcement",
    `   node examples/tide-demo-agent/agent.mjs call --project ${project} --mcp-config ${config} --server business --tool add_note --args '{\"recordId\":\"REC-1\",\"body\":\"rippletide-deny-test\"}'`,
    "   UX: allowed calls execute; denied calls return BLOCKED by Tide policy.",
    "",
    "4. Local self-check",
    "   node examples/tide-demo-agent/agent.mjs demo",
  ].join("\n"));
}

async function setup(opts) {
  if (!opts.project) throw new Error("setup requires --project");
  await run(process.execPath, [
    gateway,
    "setup",
    "--yes",
    "--project", opts.project,
    "--mcp-config", opts.mcpConfig,
    "--from", opts.from ?? defaultPolicy,
  ], { env: childEnv({
    OPENAI_API_KEY: process.env.OPENAI_API_KEY ?? "",
    ...(process.env.OPENAI_BASE_URL ? { OPENAI_BASE_URL: process.env.OPENAI_BASE_URL } : {}),
    ...(process.env.OPENAI_MODEL ? { OPENAI_MODEL: process.env.OPENAI_MODEL } : {}),
  }) });
}

async function call(opts) {
  if (!opts.project || !opts.server || !opts.tool) throw new Error("call requires --project --server --tool");
  const upstream = mcpServerToUpstream(readMcpConfig(opts.mcpConfig).find((s) => s.name === opts.server))?.command;
  if (!upstream) throw new Error(`unknown MCP server '${opts.server}' in ${opts.mcpConfig}`);
  const port = String(opts.port ?? 8900 + Math.floor(Math.random() * 500));
  const runtime = spawn(process.execPath, ["apps/runtime/dist/src/server.js", "--project", opts.project, "--port", port], {
    cwd: root,
    stdio: ["ignore", "ignore", "inherit"],
    env: childEnv(),
  });
  try {
    await waitForBundle(port);
    const result = await gatewayCall(port, `${opts.server}=${upstream}`, opts.tool, opts.args ? JSON.parse(opts.args) : {});
    console.log(JSON.stringify(result, null, 2));
  } finally {
    runtime.kill();
  }
}

async function ask(opts) {
  loadEnv();
  if (!opts.request) throw new Error("ask requires --request");
  if (!process.env.OPENAI_API_KEY) throw new Error("OPENAI_API_KEY missing");
  const choice = await chooseTool(opts.request);
  console.log(`User request: ${opts.request}`);
  console.log(`Agent selected: ${choice.server}.${choice.tool}`);
  console.log(`Agent arguments: ${JSON.stringify(choice.args)}`);
  await call({ ...opts, server: choice.server, tool: choice.tool, args: JSON.stringify(choice.args) });
}

async function chooseTool(request) {
  const tools = [
    { server: "business", tool: "read_records", description: "Read/search business records", args: { query: "string" } },
    { server: "business", tool: "create_record", description: "Create a business record", args: { title: "string", priority: "number", body: "string" } },
    { server: "business", tool: "add_note", description: "Write a note on a business record", args: { recordId: "string", body: "string" } },
    { server: "business", tool: "delete_record", description: "Delete a business record", args: { id: "string" } },
  ];
  const res = await fetch(`${(process.env.OPENAI_BASE_URL ?? "https://api.openai.com/v1").replace(/\/+$/, "")}/responses`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${process.env.OPENAI_API_KEY}` },
    body: JSON.stringify({
      model: process.env.OPENAI_MODEL ?? "gpt-4.1",
      input: [
        { role: "system", content: "You are a tiny demo agent. Pick exactly one tool. Return JSON only: {server,tool,args}." },
        { role: "user", content: JSON.stringify({ request, tools }) },
      ],
      text: { format: { type: "json_object" } },
    }),
  });
  if (!res.ok) throw new Error(`OpenAI ${res.status}`);
  const choice = JSON.parse(extractText(await res.json()));
  if (!tools.some((t) => t.server === choice.server && t.tool === choice.tool)) throw new Error(`agent chose invalid tool: ${JSON.stringify(choice)}`);
  return { server: choice.server, tool: choice.tool, args: choice.args ?? {} };
}

function parseArgs(argv) {
  const command = argv.shift();
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const key = argv[i].startsWith("--") ? argv[i].slice(2).replaceAll("-", "_") : null;
    if (!key) continue;
    opts[key === "mcp_config" ? "mcpConfig" : key] = argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : true;
  }
  return { command, opts };
}

function extractText(data) {
  if (data.output_text) return data.output_text;
  for (const item of data.output ?? []) for (const c of item.content ?? []) if (c.text) return c.text;
  throw new Error("no text in model response");
}

function loadEnv(file = ".env") {
  if (!fs.existsSync(file)) return;
  for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    const m = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)=(.*)\s*$/);
    if (!m || Object.prototype.hasOwnProperty.call(process.env, m[1])) continue;
    process.env[m[1]] = m[2].replace(/^['"]|['"]$/g, "");
  }
}

function gatewayCall(port, upstream, tool, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [gateway, "run", "--mode", "enforce", "--url", `http://127.0.0.1:${port}`, "--upstream", upstream], {
      cwd: root,
      stdio: ["pipe", "pipe", "ignore"],
      env: childEnv(),
    });
    const timer = setTimeout(() => done(new Error("gateway timeout")), 8000);
    let out = "";
    child.stdout.on("data", (chunk) => {
      out += chunk;
      const line = out.split("\n").find((l) => l.includes('"id":3'));
      if (line) done(null, JSON.parse(line));
    });
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "tide-demo-agent", version: "0.1.0" } } }) + "\n");
    setTimeout(() => {
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized", params: {} }) + "\n");
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 3, method: "tools/call", params: { name: tool, arguments: args } }) + "\n");
    }, 150);
    function done(error, value) {
      clearTimeout(timer);
      child.kill();
      error ? reject(error) : resolve(value);
    }
  });
}

async function waitForBundle(port) {
  for (let i = 0; i < 30; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/v1/bundle`)).ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error("runtime did not start");
}

function run(cmd, args, opts = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd: root, stdio: "inherit", ...opts });
    child.on("exit", (code) => code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`)));
  });
}

function childEnv(extra = {}) {
  const keep = ["PATH", "HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "TEMP", "TMP"];
  return { ...Object.fromEntries(keep.filter((k) => process.env[k]).map((k) => [k, process.env[k]])), ...extra };
}
