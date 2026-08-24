#!/usr/bin/env node

const { execFileSync, execSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const PLATFORMS = {
  "darwin-arm64": "rippletide-code-darwin-arm64",
  "darwin-x64": "rippletide-code-darwin-x64",
  "linux-x64": "rippletide-code-linux-x64",
  "win32-x64": "rippletide-code-win32-x64",
  "win32-arm64": "rippletide-code-win32-arm64",
  "linux-arm64": "rippletide-code-linux-arm64",
};

const platformKey = `${process.platform}-${process.arch}`;
const pkg = PLATFORMS[platformKey];

if (!pkg) {
  console.error(
    `Unsupported platform: ${platformKey}. Supported: ${Object.keys(PLATFORMS).join(", ")}`
  );
  process.exit(1);
}

const binName = process.platform === "win32" ? "rippletide-code.exe" : "rippletide-code";

function resolveBinPath() {
  const pkgDir = path.dirname(require.resolve(`${pkg}/package.json`));
  return path.join(pkgDir, binName);
}

let binPath;
try {
  binPath = resolveBinPath();
} catch {
  // optionalDependency not installed (common with npx on Windows) — install it on the fly
  const version = require(path.join(__dirname, "..", "package.json")).version;
  const installTarget = `${pkg}@${version}`;
  console.error(`Platform package "${pkg}" not found — installing ${installTarget}...`);
  try {
    execSync(`npm install --no-save ${installTarget}`, {
      stdio: "inherit",
      cwd: path.join(__dirname, ".."),
    });
    binPath = resolveBinPath();
  } catch (installErr) {
    console.error(`Failed to install "${installTarget}": ${installErr.message}`);
    process.exit(1);
  }
}

if (!fs.existsSync(binPath)) {
  console.error(`Binary not found at ${binPath} — the package may be incomplete.`);
  process.exit(1);
}

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  if (e.status != null) {
    process.exit(e.status);
  }
  console.error(`Failed to run rippletide-code: ${e.message}`);
  process.exit(1);
}
