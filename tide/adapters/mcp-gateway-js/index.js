#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { randomUUID } from "node:crypto";
import { defaultMcpConfigs, mcpServerToUpstream, readMcpConfig } from "./mcp-config.js";

const SESSION = `mcp-${randomUUID().slice(0, 8)}`;
const DEFAULT_MODEL = "gpt-4.1";

main().catch((error) => {
  console.error(`[tide-gateway] ${error.message}`);
  process.exit(1);
});

async function main() {
  loadEnv();
  const { command, opts } = parseArgs(process.argv.slice(2));
  if (command === "setup") return setup(opts);
  if (command !== "run") opts.upstream ??= command;
  return run(opts);
}

function parseArgs(argv) {
  let command = "run";
  const opts = {
    url: process.env.TIDE_URL ?? "http://localhost:8787",
    mode: process.env.TIDE_MODE ?? "observe",
    onUnavailable: process.env.TIDE_ON_UNAVAILABLE,
    waitSeconds: process.env.TIDE_WAIT_SECONDS ?? 55,
    upstreams: [],
    mcpConfig: process.env.TIDE_MCP_CONFIG,
    dir: "tide",
    yes: false,
    from: [],
    prompt: false,
  };
  if (argv[0] && !argv[0].startsWith("--")) command = argv.shift();
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2).replaceAll("-", "_");
    const value = argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : "true";
    if (key === "from") {
      opts.from.push(value);
      continue;
    }
    if (key === "upstream") {
      opts.upstreams.push(value);
      opts.upstream ??= value;
      continue;
    }
    opts[key === "project" ? "dir" : key === "wait_seconds" ? "waitSeconds" : key === "mcp_config" ? "mcpConfig" : key] = value;
  }
  opts.yes = opts.yes === true || opts.yes === "true";
  opts.prompt = opts.prompt === true || opts.prompt === "true";
  opts.mode = opts.mode === "enforce" ? "enforce" : "observe";
  opts.onUnavailable ??= opts.mode === "enforce" ? "deny" : "allow";
  opts.waitSeconds = Number(opts.waitSeconds);
  if (!Number.isFinite(opts.waitSeconds) || opts.waitSeconds < 0) opts.waitSeconds = 55;
  return { command, opts };
}

function initProject(dirName) {
  const dir = path.resolve(dirName);
  fs.mkdirSync(path.join(dir, "ontology"), { recursive: true });
  fs.mkdirSync(path.join(dir, "policies"), { recursive: true });
  fs.mkdirSync(path.join(dir, "tests"), { recursive: true });
  writeNew(path.join(dir, "tide.yaml"), `name: ${path.basename(dir)}\nversion: "1"\nruntime:\n  url: http://localhost:8787\n`);
  writeNew(path.join(dir, "ontology", "actions.yaml"), "actions: {}\n");
  writeNew(path.join(dir, "policies", "rules.policy.yaml"), "[]\n");
  writeNew(path.join(dir, "tests", "golden.scenarios.yaml"), "[]\n");
  writeNew(path.join(dir, "fixtures.json"), "{}\n");
  writeNew(path.join(dir, "resolvers.js"), "export default function resolvers(_fixtures) {\n  return {};\n}\n");
}

function writeNew(file, text) {
  if (!fs.existsSync(file)) fs.writeFileSync(file, text);
}

async function setup(opts) {
  const upstreams = setupUpstreams(opts);
  if (!upstreams.length) throw new Error("usage: node adapters/mcp-gateway-js/index.js setup [--mcp-config <file>] or --upstream \"<mcp server command>\" [--project ./tide]");
  const tools = [];
  for (const upstream of upstreams) {
    try {
      tools.push(...prefixTools(await discoverTools(upstream.command), upstream.prefix));
    } catch (error) {
      if (!upstream.optional) throw error;
      console.error(`[tide-gateway] skipped ${upstream.prefix}: ${error.message}`);
    }
  }
  if (!tools.length) throw new Error("no MCP tools discovered");
  const policyText = await readPolicyText(opts);
  const suggestions = await generateRules(tools, policyText);
  console.log(`Found ${tools.length} tools from ${upstreams.length} upstream${upstreams.length === 1 ? "" : "s"}.`);
  const selected = suggestions.length ? await selectRules(suggestions, opts) : [];
  writeProject(opts.dir, tools, selected);
  const report = writeFindings(opts.dir, tools, selected, { upstreams: upstreams.length, policyDocs: opts.from.length, promptedPolicy: opts.prompt });
  console.log(`wrote ${opts.dir}/ontology/actions.yaml`);
  console.log(`wrote ${opts.dir}/policies/rules.policy.yaml`);
  console.log(`wrote ${report}`);
  console.log(findingsSummary(tools, selected, { policyDocs: opts.from.length, promptedPolicy: opts.prompt }));
  console.log(`next: node apps/runtime/dist/src/server.js --project ${opts.dir}`);
}

function setupUpstreams(opts) {
  const raw = opts.upstreams.length ? opts.upstreams : (opts.upstream ? [opts.upstream] : []);
  if (raw.length) return raw.map((value) => parseUpstream(value, raw.length > 1));
  return autoMcpUpstreams(opts);
}

function parseUpstream(value, needsLabel) {
  const m = String(value).match(/^([A-Za-z0-9_-]+)=(.+)$/);
  if (m) return { prefix: m[1], command: m[2] };
  if (needsLabel) throw new Error("multiple --upstream values must use label=command");
  return { prefix: "", command: value };
}

function prefixTools(tools, prefix) {
  return tools.map((tool) => ({
    ...tool,
    ...(prefix ? { name: `${prefix}.${tool.name}`, upstreamName: tool.name, upstreamPrefix: prefix } : {}),
  }));
}

function autoMcpUpstreams(opts) {
  const files = opts.mcpConfig ? [opts.mcpConfig] : defaultMcpConfigs();
  const upstreams = files.flatMap((file) => readMcpConfig(file).map(mcpServerToUpstream).filter(Boolean));
  if (upstreams.length) console.error(`[tide-gateway] auto-discovered ${upstreams.length} MCP upstreams`);
  return upstreams;
}

function loadEnv(file = ".env") {
  if (!fs.existsSync(file)) return;
  for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    const m = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)=(.*)\s*$/);
    if (!m || Object.prototype.hasOwnProperty.call(process.env, m[1])) continue;
    process.env[m[1]] = m[2].replace(/^['"]|['"]$/g, "");
  }
}

async function discoverTools(upstream) {
  const proc = spawn(upstream, { shell: true, detached: true, stdio: ["pipe", "pipe", "inherit"], env: upstreamEnv() });
  const lines = readline.createInterface({ input: proc.stdout });
  const wait = new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("timed out waiting for tools/list")), 20000);
    lines.on("line", (line) => {
      let msg;
      try { msg = JSON.parse(line); } catch { return; }
      if (msg.id === 1) {
        proc.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized", params: {} }) + "\n");
        proc.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} }) + "\n");
      }
      if (msg.id === 2) {
        clearTimeout(timer);
        resolve(msg.result?.tools ?? []);
      }
    });
  });
  proc.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "tide-setup", version: "0.1.0" } } }) + "\n");
  try {
    return await wait;
  } finally {
    lines.close();
    proc.stdin.destroy();
    proc.stdout.destroy();
    try { process.kill(-proc.pid); } catch { proc.kill(); }
  }
}

function suggestRules(tools) {
  const out = [];
  for (const tool of tools) {
    const props = tool.inputSchema?.properties ?? {};
    const raw = baseName(tool.name);
    const textField = ["body", "text", "message", "content", "description"].find((k) => k in props);
    if (textField) {
      out.push(rule(tool.name, `${tool.name}-smoke-deny`, `when: text_matches(params.${textField}, "rippletide-deny-test")`, "Smoke test marker detected; Tide blocked this call before execution", { ...sampleParams(props), [textField]: "rippletide-deny-test" }));
    }
    if (/delete|remove|destroy|archive/i.test(raw)) {
      out.push(rule(tool.name, `${tool.name}-deny`, "when: \"true\"", `${tool.name} is destructive; blocked until explicitly allowed`, sampleParams(props)));
    }
  }
  return out;
}

async function generateRules(tools, policyText) {
  const key = process.env.OPENAI_API_KEY;
  if (!key) {
    console.error("[tide-gateway] OPENAI_API_KEY missing; using local schema heuristics");
    return suggestRules(tools);
  }
  try {
    const rules = await openaiRules(tools, policyText, key);
    if (rules.length) {
      console.error(`[tide-gateway] generated ${rules.length} rules with OpenAI`);
      return mergePolicyHeuristics(rules, tools, policyText);
    }
    console.error("[tide-gateway] OpenAI returned no usable rules; using local schema heuristics");
    return suggestRules(tools);
  } catch (error) {
    console.error(`[tide-gateway] LLM rule generation failed (${error.message}); using local schema heuristics`);
    return suggestRules(tools);
  }
}

async function readPolicyText(opts) {
  const chunks = [];
  for (const file of opts.from) chunks.push(fs.readFileSync(file, "utf8"));
  if (opts.prompt) chunks.push(await readPromptText());
  return chunks.join("\n\n").trim();
}

async function readPromptText() {
  if (!process.stdin.isTTY) return await new Promise((resolve) => {
    let s = "";
    process.stdin.on("data", (c) => s += c);
    process.stdin.on("end", () => resolve(s));
  });
  console.log("Paste policy text. End with an empty line.");
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  const lines = [];
  for (;;) {
    const line = await new Promise((resolve) => rl.question("> ", resolve));
    if (!line.trim()) break;
    lines.push(line);
  }
  rl.close();
  return lines.join("\n");
}

async function openaiRules(tools, policyText, key) {
  const body = {
    model: process.env.OPENAI_MODEL ?? DEFAULT_MODEL,
    input: [
      {
        role: "system",
        content: "You generate deterministic Tide policy rules for MCP tools. Output JSON only. No markdown. Rules must be reviewable and safe. Prefer shadow only when unsure.",
      },
      {
        role: "user",
        content: JSON.stringify({
          instructions: "Return {rules:[{id,action,when?,requires?,verdict,mode,to?,reason,test_params}]}. when MUST be a TideExpr string, never an object. Use only provided tool names. Use params.field, not input.field. Do not use JavaScript .match, regex literals, or ?. Use text_matches(params.field, 'pattern') for text checks. requires is an array of prior action names and every value must be a provided tool name. Prefer requires over when:true for sequencing rules. Explicit must/must-not policy requirements should be mode enforce. Use shadow only for weak suggestions or ambiguous policy. verdict is deny or escalate; escalate must include to. test_params must trigger the rule.",
          policy: policyText || "No explicit policy text. Suggest basic safety guardrails from tool names and schemas.",
          tools: tools.map((t) => ({ name: t.name, description: t.description ?? t.title ?? "", inputSchema: t.inputSchema ?? {} })),
        }),
      },
    ],
    text: { format: { type: "json_object" } },
  };
  const base = (process.env.OPENAI_BASE_URL ?? "https://api.openai.com/v1").replace(/\/+$/, "");
  const res = await fetch(`${base}/responses`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${key}` },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`OpenAI ${res.status}`);
  const data = await res.json();
  return normalizeLlmRules(JSON.parse(extractText(data)), tools);
}

function extractText(data) {
  if (data.output_text) return data.output_text;
  for (const item of data.output ?? []) for (const c of item.content ?? []) if (c.text) return c.text;
  const chat = data.choices?.[0]?.message?.content;
  if (chat) return chat;
  throw new Error("no text in model response");
}

function normalizeLlmRules(data, tools) {
  const actions = new Set(tools.map((t) => t.name));
  const rules = [];
  for (const r of data.rules ?? []) {
    const action = resolveAction(r.action, actions);
    if (!action || !r.id || (!r.when && !r.requires)) continue;
    const id = String(r.id).replace(/[^A-Za-z0-9_-]/g, "-").replace(/-+/g, "-").slice(0, 80);
    const requires = repairRequires({ ...r, action }, actions);
    const when = requires.length ? null : toTideExpr(r.when);
    if (!requires.length && !when) continue;
    const condition = requires.length ? `requires: [${requires.map(yamlKey).join(", ")}]` : `when: ${yamlString(when)}`;
    rules.push(rule(
      action,
      id,
      condition,
      r.reason || `Policy rule ${id} fired`,
      r.test_params && typeof r.test_params === "object" ? r.test_params : {},
      r.verdict === "escalate" ? "escalate" : "deny",
      r.mode === "shadow" ? "shadow" : "enforce",
      r.verdict === "escalate" ? escalationTarget(r.to) : undefined,
    ));
  }
  return rules;
}

function escalationTarget(to) {
  const value = typeof to === "string" ? to.trim() : "";
  return value && !/^(allow|deny|block)$/i.test(value) ? value : "human-review";
}

function toTideExpr(expr) {
  if (expr && typeof expr === "object") expr = expr.expr ?? expr.expression ?? expr.condition ?? objectCondition(expr);
  if (typeof expr !== "string" || !expr.trim()) return null;
  let out = expr.replaceAll("input.", "params.");
  out = out.replaceAll("!==", "!=").replaceAll("===", "==");
  out = out.replace(/params\.([A-Za-z0-9_]+)\.match\(\/\(([^/)]+)\)\/i\)/g, (_m, field, words) =>
    words.split("|").map((w) => `text_matches(params.${field}, ${JSON.stringify(w)})`).join(" || "));
  out = out.replace(/params\.([A-Za-z0-9_]+)\.match\(\/([^/]+)\/i\)/g, (_m, field, pattern) =>
    `text_matches(params.${field}, ${JSON.stringify(pattern)})`);
  out = out.replace(/\s+or\s+/gi, " || ").replace(/\s+and\s+/gi, " && ");
  return out;
}

function objectCondition(obj) {
  const field = obj.field || obj.param || obj.path;
  const op = obj.op || obj.operator;
  const value = obj.value;
  if (!field || !op) return null;
  const left = String(field).startsWith("params.") ? field : `params.${field}`;
  const right = typeof value === "number" || typeof value === "boolean" ? String(value) : JSON.stringify(String(value));
  return `${left} ${op} ${right}`;
}

function repairRequires(r, actions) {
  return Array.isArray(r.requires) ? r.requires.map((a) => resolveRequired(a, r.action, actions)).filter(Boolean) : [];
}

function resolveAction(name, actions) {
  if (!name) return null;
  if (actions.has(name)) return name;
  const matches = [...actions].filter((a) => baseName(a) === name);
  return matches.length === 1 ? matches[0] : null;
}

function resolveRequired(name, action, actions) {
  if (!name) return null;
  if (actions.has(name)) return name;
  const sibling = siblingAction(action, name);
  if (actions.has(sibling)) return sibling;
  return resolveAction(name, actions);
}

function siblingAction(action, raw) {
  const i = action.lastIndexOf(".");
  return i === -1 ? raw : `${action.slice(0, i + 1)}${raw}`;
}

function baseName(action) {
  const i = action.lastIndexOf(".");
  return i === -1 ? action : action.slice(i + 1);
}

function rule(action, id, condition, reason, params = {}, verdict = "deny", mode = "enforce", to = undefined) {
  return { action, id, condition, reason, params, verdict, mode, to };
}

function sampleParams(props) {
  const out = {};
  for (const [name, spec] of Object.entries(props)) {
    if (spec.type === "number" || spec.type === "integer") out[name] = 1;
    else if (spec.type === "boolean") out[name] = true;
    else out[name] = `${name}-1`;
  }
  return out;
}

function mergePolicyHeuristics(rules, tools, policyText) {
  const merged = [...rules];
  for (const rule of destructivePolicyRules(tools, policyText)) {
    if (!merged.some((r) => r.action === rule.action)) merged.push(rule);
  }
  return merged;
}

function destructivePolicyRules(tools, policyText) {
  const text = String(policyText ?? "").toLowerCase();
  if (!/\b(must not|may not|cannot|can't|never|do not)\b[\s\S]{0,120}\b(delete|remove|archive|destroy)\b/.test(text)) return [];
  return tools
    .filter((tool) => /delete|remove|archive|destroy/i.test(baseName(tool.name)))
    .map((tool) => rule(
      tool.name,
      `${tool.name.replace(/[^A-Za-z0-9_-]/g, "-")}-deny-destructive`,
      `when: "true"`,
      `${tool.name} is destructive and blocked by business policy`,
      sampleParams(tool.inputSchema?.properties ?? {}),
    ));
}

async function selectRules(rules, opts) {
  if (opts.yes || !process.stdin.isTTY) return rules;
  let cursor = 0;
  const selected = new Set(rules.map((_, i) => i));
  process.stdin.setRawMode(true);
  process.stdin.resume();
  return new Promise((resolve) => {
    const render = () => {
      process.stdout.write("\x1Bc");
      console.log("Select rules to write. space toggles, enter confirms.\n");
      rules.forEach((r, i) => console.log(`${i === cursor ? ">" : " "} [${selected.has(i) ? "x" : " "}] ${r.action}: ${r.reason}`));
    };
    const done = () => {
      process.stdin.setRawMode(false);
      process.stdin.off("data", onData);
      process.stdin.pause();
      process.stdout.write("\n");
      resolve(rules.filter((_, i) => selected.has(i)));
    };
    const onData = (buf) => {
      const s = buf.toString();
      if (s === "\u0003") process.exit(130);
      if (s === "\r" || s === "\n") return done();
      if (s === " ") selected.has(cursor) ? selected.delete(cursor) : selected.add(cursor);
      if (s === "\u001b[A") cursor = Math.max(0, cursor - 1);
      if (s === "\u001b[B") cursor = Math.min(rules.length - 1, cursor + 1);
      render();
    };
    process.stdin.on("data", onData);
    render();
  });
}

function writeProject(dirName, tools, rules) {
  const dir = path.resolve(dirName);
  initProject(dir);
  fs.writeFileSync(path.join(dir, "ontology", "actions.yaml"), actionsYaml(tools));
  fs.writeFileSync(path.join(dir, "policies", "rules.policy.yaml"), rulesYaml(rules));
  fs.writeFileSync(path.join(dir, "tests", "golden.scenarios.yaml"), testsYaml(rules));
}

function writeFindings(dirName, tools, rules, meta) {
  const dir = path.resolve(dirName);
  const report = path.join(dir, "findings.md");
  fs.writeFileSync(report, findingsReport(tools, rules, meta));
  return path.join(dirName, "findings.md");
}

function findingsReport(tools, rules, meta) {
  const risks = riskSummary(tools, rules);
  const policyInputs = meta.policyDocs + (meta.promptedPolicy ? 1 : 0);
  const businessGaps = policyInputs ? risks.uncovered : risks.risky;
  const concrete = rules.filter((r) => r.mode !== "shadow");
  const lines = [
    "# Tide Findings",
    "",
    "## Bottom Line",
    "",
    policyInputs
      ? `Tide found ${plural(risks.risky.length, "risky MCP tool")}. It can block or route ${plural(concrete.length, "concrete tool call")} before the MCP server executes anything.`
      : `Tide found ${plural(risks.risky.length, "risky MCP tool")}, but no business policy was provided. This is only a capability scan, not a safe enforce plan.`,
    "",
    "## What Your Agent Can Do",
    "",
    ...(risks.reads.length ? [`- Low-risk read path: ${risks.reads.map((t) => t.name).join(", ")}`] : ["- No obvious read-only tools detected."]),
    ...(risks.risky.length ? [`- Risky action path: ${risks.risky.map((t) => t.name).join(", ")}`] : ["- No obvious risky action tools detected."]),
    "",
    "## What Tide Would Stop",
    "",
    ...(concrete.length ? concrete.map(blockingFinding) : ["- Nothing concrete yet. Add business policy docs or select rules during setup."]),
    "",
    "## What Still Needs A Decision",
    "",
    ...(policyInputs ? policyGapLines(risks.uncovered) : [
      "- No business policy input was provided, so Tide cannot tell which risky actions are actually allowed for your business.",
      ...businessGaps.map((t) => undecidedFinding(t)),
    ]),
    "",
    "## Why This Matters",
    "",
    "- The agent can keep using safe read tools normally.",
    "- Risky write/delete/send/execute calls get a deterministic decision before execution.",
    "- The remaining gaps are explicit product decisions, not hidden prompt hopes.",
    "",
    "## Next Step",
    "",
    "Review `policies/rules.policy.yaml`, then run `.venv/bin/tide test --project <this-dir>`.",
    "",
  ];
  return lines.join("\n");
}

function findingsSummary(tools, rules, meta) {
  const risks = riskSummary(tools, rules);
  const policyInputs = meta.policyDocs + (meta.promptedPolicy ? 1 : 0);
  const businessGaps = policyInputs ? risks.uncovered : risks.risky;
  const concrete = rules.filter((r) => r.mode !== "shadow");
  return [
    "Findings:",
    `  ${risks.risky.length} risky tools found out of ${tools.length}`,
    `  ${concrete.length} concrete calls would be blocked or routed before execution`,
    `  ${businessGaps.length} decisions still needed before enforce`,
  ].join("\n");
}

function policyGapLines(uncovered) {
  return uncovered.length ? uncovered.map(undecidedFinding) : ["- No undecided risky tools detected from the supplied business policy."];
}

function blockingFinding(rule) {
  const verdict = rule.verdict === "escalate" ? `route to ${rule.to ?? "human review"}` : "block";
  return [
    `- If the agent calls \`${rule.action}\` with \`${JSON.stringify(rule.params ?? {})}\`, Tide will ${verdict} it before execution.`,
    `  Reason shown to the agent: ${rule.reason}`,
  ].join("\n");
}

function undecidedFinding(tool) {
  return `- ${tool.name}: risky tool with no business rule yet. Decide allow, deny, or escalate before enforce.`;
}

function riskSummary(tools, rules) {
  const risky = tools.filter(isRiskyTool);
  const reads = tools.filter((t) => !isRiskyTool(t));
  const covered = risky.filter((t) => rules.some((r) => r.action === t.name));
  const uncovered = risky.filter((t) => !rules.some((r) => r.action === t.name));
  return { risky, reads, covered, uncovered };
}

function isRiskyTool(tool) {
  const raw = baseName(tool.name);
  if (tool.annotations?.readOnlyHint) return false;
  if (/^(get|list|read|search|fetch|find)_?/i.test(raw)) return false;
  return /create|update|write|save|add|send|post|delete|remove|destroy|archive|apply|run|execute|deploy|publish|grant|revoke|refund|charge|transfer|invite|message/i.test(raw);
}

function plural(count, label) {
  return `${count} ${label}${count === 1 ? "" : "s"}`;
}

function actionsYaml(tools) {
  const lines = ["actions:"];
  for (const t of tools) {
    lines.push(`  ${yamlKey(t.name)}:`);
    lines.push(`    description: ${yamlString(t.description ?? t.title ?? t.name)}`);
    const props = t.inputSchema?.properties ?? {};
    if (Object.keys(props).length === 0) lines.push("    params: {}");
    else {
      lines.push("    params:");
      for (const [name, spec] of Object.entries(props)) lines.push(`      ${yamlKey(name)}: ${yamlString(spec.type ?? "any")}`);
    }
  }
  return lines.join("\n") + "\n";
}

function rulesYaml(rules) {
  if (!rules.length) return "[]\n";
  return rules.map((r) => [
    `- id: ${yamlKey(r.id)}`,
    `  action: ${yamlKey(r.action)}`,
    `  ${r.condition}`,
    `  verdict: ${r.verdict}`,
    ...(r.to ? [`  to: ${yamlKey(r.to)}`] : []),
    `  mode: ${r.mode}`,
    `  reason: ${yamlString(r.reason)}`,
  ].join("\n")).join("\n\n") + "\n";
}

function testsYaml(rules) {
  if (!rules.length) return "[]\n";
  return rules.map((r) => [
    `- name: ${yamlString(`${r.id} blocks`)}`,
    "  session: []",
    `  action: ${yamlKey(r.action)}`,
    `  params: ${JSON.stringify(r.params ?? {})}`,
    "  context: {}",
    `  expect: ${r.mode === "shadow" ? "allow" : r.verdict}`,
    `  expect_rules: [${yamlKey(r.id)}]`,
  ].join("\n")).join("\n\n") + "\n";
}

function yamlKey(s) {
  return /^[A-Za-z0-9_-]+$/.test(s) ? s : yamlString(s);
}

function yamlString(s) {
  return JSON.stringify(String(s));
}

async function run(opts) {
  if (!opts.upstream) throw new Error("usage: node adapters/mcp-gateway-js/index.js run --upstream \"<mcp server command>\" [--mode observe|enforce]");
  if (opts.mode === "enforce") await assertEnforceReady(opts.url);

  const parsed = parseUpstream(opts.upstream, false);
  opts.prefix ||= parsed.prefix;
  opts.upstream = parsed.command;
  const upstream = spawn(opts.upstream, { shell: true, stdio: ["pipe", "pipe", "inherit"], env: upstreamEnv() });
  upstream.on("exit", (code) => process.exit(code ?? 0));
  console.error(`[tide-gateway] session ${SESSION} · mode ${opts.mode} · runtime ${opts.url} · upstream ${opts.upstream}`);

  readline.createInterface({ input: upstream.stdout }).on("line", (line) => {
    process.stdout.write(line + "\n");
  });

  readline.createInterface({ input: process.stdin }).on("line", async (line) => {
    if (!line.trim()) return;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      return forward(upstream, line);
    }
    if (msg.method === "tools/call") await authorizeAndRoute(msg, upstream, opts);
    else forward(upstream, JSON.stringify(msg));
  });
}

async function assertEnforceReady(url) {
  let bundle;
  try {
    bundle = await api(url, "GET", "/v1/bundle");
  } catch (error) {
    throw new Error(`runtime unavailable in enforce mode: ${error.message}`);
  }
  if (!bundle.actions?.length) {
    throw new Error("refusing enforce mode with empty action space; declare ontology/actions.yaml or run --mode observe");
  }
}

async function authorizeAndRoute(msg, upstream, opts) {
  const params = msg.params ?? {};
  const action = params.name ?? "";
  const governedAction = opts.prefix ? `${opts.prefix}.${action}` : action;
  const args = params.arguments ?? {};
  let decision;
  try {
    decision = await api(opts.url, "POST", "/v1/decisions", {
      agent_id: process.env.TIDE_AGENT_ID ?? "mcp-agent",
      session_id: SESSION,
      tool: governedAction,
      params: args,
      context: {},
      metadata: { gateway: "mcp" },
      idempotency_key: msg.id ? String(msg.id) : undefined,
    });
  } catch (error) {
    if (opts.onUnavailable === "deny") return send(blocked(msg.id, `Policy runtime unavailable; failing closed: ${error.message}`));
    console.error(`[tide-gateway] runtime unavailable; forwarding ${action} ungoverned`);
    return forward(upstream, JSON.stringify(msg));
  }

  if (opts.mode === "observe") {
    console.error(`[tide-gateway] OBSERVE ${governedAction} -> ${decision.verdict}`);
    return forward(upstream, JSON.stringify(msg));
  }
  if (decision.verdict === "allow") {
    console.error(`[tide-gateway] ALLOW ${governedAction}`);
    return forward(upstream, JSON.stringify(msg));
  }
  if (decision.verdict === "deny") {
    const reason = (decision.reasons ?? []).join("; ") || "denied by policy";
    console.error(`[tide-gateway] DENY ${governedAction}: ${reason}`);
    return send(blocked(msg.id, `BLOCKED by Tide policy: ${reason}`));
  }
  const reason = (decision.reasons ?? []).join("; ") || `requires approval (${decision.trace_id})`;
  console.error(`[tide-gateway] ESCALATE ${governedAction}: ${reason}`);
  return waitForApproval(msg, upstream, opts, decision, governedAction);
}

async function waitForApproval(msg, upstream, opts, decision, action) {
  const traceId = decision.trace_id;
  if (!traceId) return send(blocked(msg.id, "Approval required but no trace_id was returned by Tide."));

  console.error(`[tide-gateway] waiting up to ${Math.floor(opts.waitSeconds)}s for ${action} approval (trace ${traceId})`);
  const deadline = Date.now() + opts.waitSeconds * 1000;
  while (Date.now() < deadline) {
    let status;
    try {
      status = await api(opts.url, "GET", `/v1/decisions/${traceId}`);
    } catch (error) {
      console.error(`[tide-gateway] approval poll failed for ${traceId}: ${error.message}`);
      break;
    }

    if (status.status === "approved" || status.verdict === "allow") {
      console.error(`[tide-gateway] APPROVED ${action}; forwarding`);
      return forward(upstream, JSON.stringify(msg));
    }
    if (status.status === "rejected" || status.verdict === "deny") {
      console.error(`[tide-gateway] REJECTED ${action}`);
      return send(blocked(msg.id, "A human rejected this action. Do not retry it; inform the user and continue with what you can."));
    }
    await sleep(1500);
  }

  return send(blocked(
    msg.id,
    `This action requires human approval (still pending, trace ${traceId}). It was NOT executed. Tell the user it awaits approval; it can be retried after approval.`,
  ));
}

async function api(baseUrl, method, path, body) {
  const res = await fetch(baseUrl.replace(/\/+$/, "") + path, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`${method} ${path} -> ${res.status}`);
  return res.json();
}

function forward(upstream, line) {
  upstream.stdin.write(line + "\n");
}

function send(obj) {
  process.stdout.write(JSON.stringify(obj) + "\n");
}

function blocked(id, text) {
  return { jsonrpc: "2.0", id, result: { content: [{ type: "text", text }], isError: true } };
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function upstreamEnv() {
  if (process.env.TIDE_INHERIT_ENV === "1") return process.env;
  const keep = ["PATH", "HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "TEMP", "TMP"];
  return Object.fromEntries(keep.filter((k) => process.env[k]).map((k) => [k, process.env[k]]));
}
