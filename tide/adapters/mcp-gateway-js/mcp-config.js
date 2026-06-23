import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export function defaultMcpConfigs() {
  return [
    path.resolve(".mcp.json"),
    path.join(os.homedir(), ".codex", "config.toml"),
    path.join(os.homedir(), "Library", "Application Support", "Claude", "claude_desktop_config.json"),
  ];
}

export function readMcpConfig(file) {
  if (!fs.existsSync(file)) return [];
  const text = fs.readFileSync(file, "utf8");
  return file.endsWith(".json") ? readJsonMcpConfig(text) : readCodexMcpConfig(text);
}

export function mcpServerToUpstream(server) {
  if (!server) return null;
  const unwrapped = unwrapGatewayUpstream(server);
  if (unwrapped) return { prefix: server.name, command: unwrapped, optional: true };
  if (server.url) return { prefix: server.name, command: `npx -y mcp-remote ${shellQuote(server.url)}`, optional: true };
  if (!server.command) return null;
  return { prefix: server.name, command: [server.command, ...(server.args ?? [])].map(shellQuote).join(" "), optional: true };
}

function readJsonMcpConfig(text) {
  const servers = JSON.parse(text).mcpServers ?? {};
  return Object.entries(servers).map(([name, s]) => ({ name, ...s })).filter((s) => s.enabled !== false);
}

function readCodexMcpConfig(text) {
  const servers = [];
  let current = null;
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const section = lines[i].match(/^\s*\[mcp_servers\.([^\].]+)\]\s*$/);
    if (section) {
      current = { name: section[1], args: [] };
      servers.push(current);
      continue;
    }
    if (/^\s*\[/.test(lines[i])) current = null;
    if (!current) continue;
    const m = lines[i].match(/^\s*([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*(.+)\s*$/);
    if (!m) continue;
    let value = m[2].trim();
    if (value === "[") {
      const chunk = [];
      while (++i < lines.length && lines[i].trim() !== "]") chunk.push(lines[i]);
      value = `[${chunk.join("\n")}]`;
    }
    current[m[1]] = tomlValue(value);
  }
  return servers.filter((s) => s.enabled !== false);
}

function tomlValue(value) {
  if (value === "true" || value === "false") return value === "true";
  if (value.startsWith("[")) return [...value.matchAll(/"((?:\\.|[^"\\])*)"|'([^']*)'/g)].map((m) => m[1] === undefined ? m[2] : JSON.parse(`"${m[1]}"`));
  if (value.startsWith('"')) return JSON.parse(value);
  if (value.startsWith("'")) return value.slice(1, value.lastIndexOf("'"));
  return value;
}

function unwrapGatewayUpstream(server) {
  const args = server.args ?? [];
  if (![server.command, ...args].some((part) => String(part).includes("mcp-gateway-js"))) return null;
  const i = args.indexOf("--upstream");
  if (i >= 0) return args[i + 1];
  const prefixed = args.find((arg) => String(arg).startsWith("--upstream="));
  return prefixed ? prefixed.slice("--upstream=".length) : null;
}

function shellQuote(s) {
  return `'${String(s).replaceAll("'", "'\\''")}'`;
}
