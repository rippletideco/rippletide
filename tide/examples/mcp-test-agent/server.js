#!/usr/bin/env node
import readline from "node:readline";

const tools = [
  { name: "read_records", description: "Search business records", inputSchema: { type: "object", properties: { query: { type: "string" } } }, annotations: { readOnlyHint: true } },
  { name: "create_record", description: "Create a business record", inputSchema: { type: "object", properties: { title: { type: "string" }, priority: { type: "number" }, body: { type: "string" } } } },
  { name: "add_note", description: "Add a note to a business record", inputSchema: { type: "object", properties: { recordId: { type: "string" }, body: { type: "string" } } } },
  { name: "delete_record", description: "Delete a business record", inputSchema: { type: "object", properties: { id: { type: "string" } } } },
];

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const msg = JSON.parse(line);
  if (!("id" in msg)) return;
  if (msg.method === "initialize") return send(msg.id, { protocolVersion: "2024-11-05", capabilities: { tools: {} }, serverInfo: { name: "tide-demo-agent", version: "0.1.0" } });
  if (msg.method === "tools/list") return send(msg.id, { tools });
  if (msg.method !== "tools/call") return send(msg.id, { content: [{ type: "text", text: `unsupported ${msg.method}` }] });
  const { name, arguments: args = {} } = msg.params ?? {};
  if (name === "read_records") {
    return send(msg.id, { content: [{ type: "text", text: [
      `Search results for "${args.query ?? ""}":`,
      "- REC-102: Customer sync is healthy",
      "- REC-118: Add policy guardrails before agent writes",
    ].join("\n") }] });
  }
  if (name === "add_note") {
    return send(msg.id, { content: [{ type: "text", text: `Added note on ${args.recordId}: ${args.body}` }] });
  }
  if (name === "create_record") return send(msg.id, { content: [{ type: "text", text: `Created record: ${args.title}` }] });
  if (name === "delete_record") return send(msg.id, { content: [{ type: "text", text: `Deleted record: ${args.id}` }] });
  send(msg.id, { content: [{ type: "text", text: `unknown tool ${name}` }] });
});

function send(id, result) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, result }) + "\n");
}
