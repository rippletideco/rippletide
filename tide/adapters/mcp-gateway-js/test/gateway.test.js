import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const DIR = path.dirname(fileURLToPath(import.meta.url));

test("enforce mode blocks denied tool calls", async () => {
  const server = http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    if (req.url === "/v1/bundle") return res.end(JSON.stringify({ actions: ["create_record"] }));
    res.end(JSON.stringify({ trace_id: "tr_1", verdict: "deny", reasons: ["no writes"] }));
  });
  await listen(server);
  const port = server.address().port;
  const child = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "run",
    "--mode", "enforce",
    "--url", `http://127.0.0.1:${port}`,
    "--upstream", `${process.execPath} ${path.join(DIR, "dummy-upstream.js")}`,
  ], { stdio: ["pipe", "pipe", "ignore"] });
  try {
    const line = nextLine(child.stdout);
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "create_record", arguments: {} } }) + "\n");
    const out = JSON.parse(await line);
    assert.equal(out.result.isError, true);
    assert.match(out.result.content[0].text, /no writes/);
  } finally {
    child.kill();
    server.close();
  }
});

test("setup discovers tools and writes a testable tide project", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tide-setup-"));
  const setup = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "setup",
    "--yes",
    "--project", dir,
    "--upstream", `${process.execPath} ${path.join(DIR, "dummy-upstream.js")}`,
  ], { stdio: ["ignore", "pipe", "ignore"], env: { ...process.env, OPENAI_API_KEY: "" } });
  await exitOk(setup);
  assert.match(fs.readFileSync(path.join(dir, "ontology", "actions.yaml"), "utf8"), /create_record/);
  assert.match(fs.readFileSync(path.join(dir, "policies", "rules.policy.yaml"), "utf8"), /add_note-smoke-deny/);
  const report = fs.readFileSync(path.join(dir, "findings.md"), "utf8");
  assert.match(report, /# Tide Findings/);
  assert.match(report, /This is only a capability scan/);
  assert.match(report, /If the agent calls `add_note`/);
  assert.match(report, /create_record: risky tool with no business rule yet/);
});

test("setup discovers multiple upstreams into one prefixed project", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tide-multi-"));
  const cmd = `${process.execPath} ${path.join(DIR, "dummy-upstream.js")}`;
  const setup = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "setup",
    "--yes",
    "--project", dir,
    "--upstream", `crm=${cmd}`,
    "--upstream", `docs=${cmd}`,
  ], { stdio: ["ignore", "pipe", "ignore"], env: { ...process.env, OPENAI_API_KEY: "" } });
  await exitOk(setup);
  const actions = fs.readFileSync(path.join(dir, "ontology", "actions.yaml"), "utf8");
  const rules = fs.readFileSync(path.join(dir, "policies", "rules.policy.yaml"), "utf8");
  assert.match(actions, /crm\.create_record/);
  assert.match(actions, /docs\.create_record/);
  assert.match(rules, /crm\.add_note-smoke-deny/);
  assert.match(rules, /docs\.add_note-smoke-deny/);
});

test("setup auto-discovers MCP servers from JSON config", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tide-json-config-"));
  const config = path.join(dir, "mcp.json");
  fs.writeFileSync(config, JSON.stringify({
    mcpServers: {
      business: { command: process.execPath, args: [path.join(DIR, "dummy-upstream.js")] },
      disabled: { command: process.execPath, args: [path.join(DIR, "dummy-upstream.js")], enabled: false },
    },
  }));
  const setup = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "setup",
    "--yes",
    "--project", dir,
    "--mcp-config", config,
  ], { stdio: ["ignore", "pipe", "ignore"], env: { ...process.env, OPENAI_API_KEY: "" } });
  await exitOk(setup);
  const actions = fs.readFileSync(path.join(dir, "ontology", "actions.yaml"), "utf8");
  assert.match(actions, /business\.create_record/);
  assert.doesNotMatch(actions, /disabled\.create_record/);
});

test("setup auto-discovers MCP servers from Codex TOML config", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tide-toml-config-"));
  const config = path.join(dir, "config.toml");
  const dummy = path.join(DIR, "dummy-upstream.js");
  fs.writeFileSync(config, [
    "[mcp_servers.crm]",
    `command = ${JSON.stringify(process.execPath)}`,
    `args = [${JSON.stringify(dummy)}]`,
    "",
    "[mcp_servers.docs]",
    `command = ${JSON.stringify(process.execPath)}`,
    "args = [",
    `  ${JSON.stringify(path.join(DIR, "..", "index.js"))},`,
    '  "run",',
    '  "--upstream",',
    `  ${JSON.stringify(`${process.execPath} ${dummy}`)}`,
    "]",
    "",
    "[mcp_servers.off]",
    `command = ${JSON.stringify(process.execPath)}`,
    `args = [${JSON.stringify(dummy)}]`,
    "enabled = false",
  ].join("\n"));
  const setup = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "setup",
    "--yes",
    "--project", dir,
    "--mcp-config", config,
  ], { stdio: ["ignore", "pipe", "ignore"], env: { ...process.env, OPENAI_API_KEY: "" } });
  await exitOk(setup);
  const actions = fs.readFileSync(path.join(dir, "ontology", "actions.yaml"), "utf8");
  assert.match(actions, /crm\.create_record/);
  assert.match(actions, /docs\.create_record/);
  assert.doesNotMatch(actions, /off\.create_record/);
});

test("run with a labeled upstream authorizes prefixed tool names", async () => {
  const calls = [];
  const server = http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    if (req.url === "/v1/bundle") return res.end(JSON.stringify({ actions: ["crm.create_record"] }));
    let body = "";
    req.on("data", (chunk) => body += chunk);
    req.on("end", () => {
      if (body) calls.push(JSON.parse(body));
      res.end(JSON.stringify({ trace_id: "tr_1", verdict: "allow", reasons: [] }));
    });
  });
  await listen(server);
  const port = server.address().port;
  const child = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "run",
    "--mode", "enforce",
    "--url", `http://127.0.0.1:${port}`,
    "--upstream", `crm=${process.execPath} ${path.join(DIR, "dummy-upstream.js")}`,
  ], { stdio: ["pipe", "pipe", "ignore"] });
  try {
    const line = nextLine(child.stdout);
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "create_record", arguments: {} } }) + "\n");
    const out = JSON.parse(await line);
    assert.equal(out.result.executed, true);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].tool, "crm.create_record");
  } finally {
    child.kill();
    server.close();
  }
});

test("enforce mode waits for escalated approval before forwarding", async () => {
  const calls = [];
  let polls = 0;
  const server = http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    if (req.url === "/v1/bundle") return res.end(JSON.stringify({ actions: ["crm.create_record"] }));
    if (req.method === "GET" && req.url === "/v1/decisions/tr_escalate") {
      polls++;
      return res.end(JSON.stringify({ trace_id: "tr_escalate", status: "approved", verdict: "allow" }));
    }
    let body = "";
    req.on("data", (chunk) => body += chunk);
    req.on("end", () => {
      if (body) calls.push(JSON.parse(body));
      res.end(JSON.stringify({ trace_id: "tr_escalate", verdict: "escalate", reasons: ["needs approval"] }));
    });
  });
  await listen(server);
  const port = server.address().port;
  const child = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "run",
    "--mode", "enforce",
    "--wait-seconds", "2",
    "--url", `http://127.0.0.1:${port}`,
    "--upstream", `crm=${process.execPath} ${path.join(DIR, "dummy-upstream.js")}`,
  ], { stdio: ["pipe", "pipe", "ignore"] });
  try {
    const line = nextLine(child.stdout);
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "create_record", arguments: {} } }) + "\n");
    const out = JSON.parse(await line);
    assert.equal(out.result.executed, true);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].tool, "crm.create_record");
    assert.equal(polls, 1);
  } finally {
    child.kill();
    server.close();
  }
});

test("setup uses LLM rules from policy files when OpenAI is configured", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tide-llm-"));
  const policy = path.join(dir, "policy.txt");
  fs.writeFileSync(policy, "Agents may not create high-priority business records.");
  const server = http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    res.end(JSON.stringify({
      output_text: JSON.stringify({
        rules: [
          {
            id: "llm-no-high-priority",
            action: "create_record",
            when: "params.priority == 1",
            verdict: "deny",
            mode: "enforce",
            reason: "High-priority records must be created by a human",
            test_params: { title: "Outage", priority: 1, body: "..." },
          },
          {
            id: "llm-requires-read",
            action: "create_record",
            requires: ["read_records"],
            verdict: "escalate",
            to: "allow",
            mode: "enforce",
            reason: "Needs a prior read",
            test_params: { title: "Follow-up", priority: 0, body: "..." },
          },
        ],
      }),
    }));
  });
  await listen(server);
  const setup = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "setup",
    "--yes",
    "--project", dir,
    "--from", policy,
    "--upstream", `${process.execPath} ${path.join(DIR, "dummy-upstream.js")}`,
  ], {
    stdio: ["ignore", "pipe", "ignore"],
    env: { ...process.env, OPENAI_API_KEY: "test", OPENAI_BASE_URL: `http://127.0.0.1:${server.address().port}` },
  });
  try {
    await exitOk(setup);
    const rules = fs.readFileSync(path.join(dir, "policies", "rules.policy.yaml"), "utf8");
    assert.match(rules, /llm-no-high-priority/);
    assert.match(rules, /to: human-review/);
    assert.doesNotMatch(rules, /to: allow/);
    assert.doesNotMatch(rules, /smoke-deny/);
    const report = fs.readFileSync(path.join(dir, "findings.md"), "utf8");
    assert.match(report, /Tide found 3 risky MCP tools/);
    assert.match(report, /If the agent calls `create_record`/);
    assert.match(report, /route to human-review/);
    assert.match(report, /High-priority records must be created by a human/);
    assert.match(report, /add_note: risky tool with no business rule yet/);
  } finally {
    server.close();
  }
});

test("setup keeps deterministic destructive denies when LLM omits them", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tide-llm-destructive-"));
  const policy = path.join(dir, "policy.txt");
  fs.writeFileSync(policy, "Agents must not delete, remove, archive, or destroy records.");
  const server = http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    res.end(JSON.stringify({
      output_text: JSON.stringify({
        rules: [{
          id: "llm-note-secret",
          action: "add_note",
          when: "text_matches(params.body, 'secret')",
          verdict: "deny",
          mode: "enforce",
          reason: "Do not write secrets",
          test_params: { recordId: "REC-1", body: "secret" },
        }],
      }),
    }));
  });
  await listen(server);
  const setup = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "setup",
    "--yes",
    "--project", dir,
    "--from", policy,
    "--upstream", `${process.execPath} ${path.join(DIR, "dummy-upstream.js")}`,
  ], {
    stdio: ["ignore", "pipe", "ignore"],
    env: { ...process.env, OPENAI_API_KEY: "test", OPENAI_BASE_URL: `http://127.0.0.1:${server.address().port}` },
  });
  try {
    await exitOk(setup);
    const rules = fs.readFileSync(path.join(dir, "policies", "rules.policy.yaml"), "utf8");
    assert.match(rules, /delete_record-deny-destructive/);
    const report = fs.readFileSync(path.join(dir, "findings.md"), "utf8");
    assert.doesNotMatch(report, /delete_record: risky tool with no business rule yet/);
  } finally {
    server.close();
  }
});

test("setup accepts prompt text from stdin", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tide-prompt-"));
  const server = http.createServer((req, res) => {
    res.setHeader("content-type", "application/json");
    res.end(JSON.stringify({
      output_text: JSON.stringify({
        rules: [{
          id: "prompt-note-block",
          action: "add_note",
          when: "text_matches(params.body, 'secret')",
          verdict: "deny",
          mode: "enforce",
          reason: "Do not post secrets in notes",
          test_params: { recordId: "REC-1", body: "secret" },
        }],
      }),
    }));
  });
  await listen(server);
  const setup = spawn(process.execPath, [
    path.join(DIR, "..", "index.js"),
    "setup",
    "--yes",
    "--prompt",
    "--project", dir,
    "--upstream", `${process.execPath} ${path.join(DIR, "dummy-upstream.js")}`,
  ], {
    stdio: ["pipe", "pipe", "ignore"],
    env: { ...process.env, OPENAI_API_KEY: "test", OPENAI_BASE_URL: `http://127.0.0.1:${server.address().port}` },
  });
  setup.stdin.end("Never post secrets in notes.");
  try {
    await exitOk(setup);
    assert.match(fs.readFileSync(path.join(dir, "policies", "rules.policy.yaml"), "utf8"), /prompt-note-block/);
  } finally {
    server.close();
  }
});

function listen(server) {
  return new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
}

function nextLine(stream) {
  return new Promise((resolve) => {
    let buf = "";
    stream.on("data", (chunk) => {
      buf += chunk;
      const i = buf.indexOf("\n");
      if (i >= 0) resolve(buf.slice(0, i));
    });
  });
}

function exitOk(child) {
  return new Promise((resolve, reject) => {
    child.on("exit", (code) => code === 0 ? resolve() : reject(new Error(`exit ${code}`)));
  });
}
