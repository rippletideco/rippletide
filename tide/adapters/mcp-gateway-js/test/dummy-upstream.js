#!/usr/bin/env node
import readline from "node:readline";

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const msg = JSON.parse(line);
  if (!("id" in msg)) return;
  if (msg.method === "initialize") return send(msg.id, { protocolVersion: "2024-11-05", capabilities: { tools: {} }, serverInfo: { name: "dummy" } });
  if (msg.method === "tools/list") return send(msg.id, { tools });
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { executed: true } }) + "\n");
});

function send(id, result) {
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, result }) + "\n");
}

const tools = [
  {
    name: "read_records",
    description: "Search business records",
    inputSchema: { type: "object", properties: { query: { type: "string" } } },
    annotations: { readOnlyHint: true },
  },
  {
    name: "create_record",
    description: "Create a business record",
    inputSchema: { type: "object", properties: { title: { type: "string" }, priority: { type: "number" }, body: { type: "string" } } },
  },
  {
    name: "add_note",
    description: "Add a note to a business record",
    inputSchema: { type: "object", properties: { recordId: { type: "string" }, body: { type: "string" } } },
  },
  {
    name: "delete_record",
    description: "Delete a business record",
    inputSchema: { type: "object", properties: { id: { type: "string" } } },
  },
];
