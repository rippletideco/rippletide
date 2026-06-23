// Loads a tide project directory (the output of `tide init`) into a compiled Bundle.
// Layout: tide.yaml, ontology/actions.yaml, policies/*.policy.yaml, fixtures.json,
// resolvers.js (optional), tests/*.scenarios.yaml, traces/*.jsonl

import * as fs from "node:fs";
import * as path from "node:path";
import { pathToFileURL } from "node:url";
import YAML from "yaml";
import { Bundle, Rule, Resolvers, Diagnostic, validateBundle } from "./engine.js";
import { Value } from "./expr.js";

export interface Project {
  dir: string;
  config: Record<string, unknown>;
  bundle: Bundle;
  fixtures: Record<string, Value>;
  resolvers: Resolvers;
  diagnostics: Diagnostic[];
}

function readYaml(file: string): unknown {
  return YAML.parse(fs.readFileSync(file, "utf8"));
}

export async function loadProject(dir: string): Promise<Project> {
  const configPath = path.join(dir, "tide.yaml");
  if (!fs.existsSync(configPath)) throw new Error(`not a tide project: ${configPath} not found`);
  const config = (readYaml(configPath) ?? {}) as Record<string, unknown>;

  const actionsPath = path.join(dir, "ontology", "actions.yaml");
  const actionsDoc = fs.existsSync(actionsPath) ? (readYaml(actionsPath) as { actions?: Record<string, unknown> }) : {};
  const actions = (actionsDoc?.actions ?? {}) as Bundle["actions"];

  const rules: Rule[] = [];
  const policiesDir = path.join(dir, "policies");
  if (fs.existsSync(policiesDir)) {
    for (const f of fs.readdirSync(policiesDir).sort()) {
      if (!f.endsWith(".yaml") && !f.endsWith(".yml")) continue;
      const doc = readYaml(path.join(policiesDir, f));
      if (!Array.isArray(doc)) throw new Error(`${f}: expected a YAML list of rules`);
      for (const raw of doc) {
        const r = raw as Rule;
        rules.push({ ...r, mode: r.mode ?? "enforce" });
      }
    }
  }

  const bundle: Bundle = {
    name: String(config.name ?? path.basename(dir)),
    version: String(config.version ?? "0"),
    actions,
    rules,
  };

  const fixturesPath = path.join(dir, "fixtures.json");
  const fixtures = fs.existsSync(fixturesPath)
    ? (JSON.parse(fs.readFileSync(fixturesPath, "utf8")) as Record<string, Value>)
    : {};

  let resolvers: Resolvers = {};
  const resolversPath = path.join(dir, "resolvers.js");
  if (fs.existsSync(resolversPath)) {
    const mod = await import(pathToFileURL(resolversPath).href);
    if (typeof mod.default !== "function") throw new Error("resolvers.js must default-export a factory(fixtures) => resolver namespaces");
    resolvers = mod.default(fixtures) as Resolvers;
  }

  return { dir, config, bundle, fixtures, resolvers, diagnostics: validateBundle(bundle) };
}

// ---------- Scenarios (tide test) ----------

export interface Scenario {
  name: string;
  session?: string[]; // pre-seeded allowed-action history
  action: string;
  params?: Record<string, Value>;
  context?: Record<string, Value>;
  expect: "allow" | "deny" | "escalate";
  expect_rules?: string[]; // rule ids that must be among the fired (enforce) rules
}

export function loadScenarios(dir: string): Scenario[] {
  const testsDir = path.join(dir, "tests");
  if (!fs.existsSync(testsDir)) return [];
  const out: Scenario[] = [];
  for (const f of fs.readdirSync(testsDir).sort()) {
    if (!f.endsWith(".yaml") && !f.endsWith(".yml")) continue;
    const doc = readYaml(path.join(testsDir, f));
    if (Array.isArray(doc)) out.push(...(doc as Scenario[]));
  }
  return out;
}
