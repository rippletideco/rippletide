#!/usr/bin/env node

const { execFileSync } = require("child_process");
const path = require("path");

const PLATFORMS = {
  "darwin-arm64": "rippletide-mcp-darwin-arm64",
  "darwin-x64": "rippletide-mcp-darwin-x64",
  "linux-x64": "rippletide-mcp-linux-x64",
  "win32-x64": "rippletide-mcp-win32-x64",
  "win32-arm64": "rippletide-mcp-win32-arm64",
  "linux-arm64": "rippletide-mcp-linux-arm64",
};

const platformKey = `${process.platform}-${process.arch}`;
const pkg = PLATFORMS[platformKey];

if (!pkg) {
  console.error(
    `Unsupported platform: ${platformKey}. Supported: ${Object.keys(PLATFORMS).join(", ")}`
  );
  process.exit(1);
}

let binPath;
try {
  const binName = process.platform === "win32" ? "rippletide-mcp.exe" : "rippletide-mcp";
  binPath = path.join(require.resolve(`${pkg}/package.json`), "..", binName);
} catch {
  console.error(
    `Could not find the binary package "${pkg}" for your platform.\n` +
      `Make sure it was installed (it should be an optionalDependency).`
  );
  process.exit(1);
}

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  process.exit(e.status ?? 1);
}
