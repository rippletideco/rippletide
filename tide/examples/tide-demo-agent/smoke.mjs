#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const project = fs.mkdtempSync(path.join(os.tmpdir(), "tide-demo-agent-"));
const mcpConfig = path.join(project, "mcp.toml");
const agent = path.join(root, "examples/tide-demo-agent/agent.mjs");
const openai = http.createServer(fakeOpenAI);

fs.writeFileSync(mcpConfig, [
  "[mcp_servers.business]",
  `command = ${JSON.stringify(process.execPath)}`,
  `args = [${JSON.stringify(path.join(root, "examples/mcp-test-agent/server.js"))}]`,
].join("\n"));

await listen(openai);

try {
  const preview = await text(process.execPath, [agent, "preview", "--project", project, "--mcp-config", mcpConfig]);
  assert(preview.includes("Tide agent UX preview"), preview);
  assert(preview.includes("select suggested rules"), preview);
  assert(preview.includes("BLOCKED by Tide policy"), preview);
  process.stdout.write(preview + "\n\n");

  await run(process.execPath, [agent, "setup", "--project", project, "--mcp-config", mcpConfig], {
    OPENAI_API_KEY: "test",
    OPENAI_BASE_URL: `http://127.0.0.1:${openai.address().port}`,
  });

  const actions = fs.readFileSync(path.join(project, "ontology/actions.yaml"), "utf8");
  assert(actions.includes('"business.add_note"') && actions.includes('"business.read_records"'), "missing namespaced actions");

  await run(path.join(root, ".venv/bin/tide"), ["test", "--project", project]);

  const allowed = await json(process.execPath, [agent, "call", "--project", project, "--mcp-config", mcpConfig, "--server", "business", "--tool", "read_records", "--args", '{"query":"hello"}']);
  assert(allowed.result?.content?.[0]?.text.includes("Search results for"), JSON.stringify(allowed));

  const writeAllowed = await json(process.execPath, [agent, "call", "--project", project, "--mcp-config", mcpConfig, "--server", "business", "--tool", "add_note", "--args", '{"recordId":"SAFE-1","body":"safe sandbox write"}']);
  assert(writeAllowed.result?.content?.[0]?.text.includes("Added note on SAFE-1"), JSON.stringify(writeAllowed));

  const denied = await json(process.execPath, [agent, "call", "--project", project, "--mcp-config", mcpConfig, "--server", "business", "--tool", "add_note", "--args", '{"recordId":"REC-1","body":"rippletide-deny-test"}']);
  assert(denied.result?.isError === true, JSON.stringify(denied));

  console.log("tide demo agent smoke ok");
} finally {
  openai.close();
}

function run(cmd, args, env = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd: root, stdio: "inherit", env: childEnv(env) });
    child.on("exit", (code) => code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`)));
  });
}

function json(cmd, args) {
  return text(cmd, args).then((out) => {
    const start = out.indexOf("{");
    return JSON.parse(out.slice(start));
  });
}

function text(cmd, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd: root, stdio: ["ignore", "pipe", "inherit"], env: childEnv({ OPENAI_API_KEY: "" }) });
    let out = "";
    child.stdout.on("data", (chunk) => out += chunk);
    child.on("exit", (code) => {
      if (code !== 0) return reject(new Error(`${cmd} exited ${code}`));
      resolve(out.trim());
    });
  });
}

function assert(ok, message) {
  if (!ok) throw new Error(message);
}

function listen(server) {
  return new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
}

function childEnv(extra = {}) {
  const keep = ["PATH", "HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "TEMP", "TMP"];
  return { ...Object.fromEntries(keep.filter((k) => process.env[k]).map((k) => [k, process.env[k]])), ...extra };
}

function fakeOpenAI(req, res) {
  let body = "";
  req.on("data", (chunk) => body += chunk);
  req.on("end", () => {
    res.setHeader("content-type", "application/json");
    if (!body.includes("Business policy")) {
      res.statusCode = 400;
      return res.end(JSON.stringify({ error: "policy missing" }));
    }
    res.end(JSON.stringify({
      output_text: JSON.stringify({
        rules: [{
          id: "business-note-deny-marker",
          action: "business.add_note",
          when: "text_matches(params.body, 'rippletide-deny-test')",
          verdict: "deny",
          mode: "enforce",
          reason: "Business policy blocks unsafe notes",
          test_params: { recordId: "REC-1", body: "rippletide-deny-test" },
        }],
      }),
    }));
  });
}
