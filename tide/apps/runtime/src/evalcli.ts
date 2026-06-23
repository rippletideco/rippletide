// Offline evaluator CLI — the same engine the runtime serves, callable by `tide`
// for compile / test. Emits JSON on stdout; exit code 1 on failure.
//
//   node dist/src/evalcli.js compile --project <dir> [--out <file>]
//   node dist/src/evalcli.js test    --project <dir>

import * as fs from "node:fs";
import * as path from "node:path";
import { Engine } from "./engine.js";
import { loadProject, loadScenarios } from "./project.js";

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

function out(obj: unknown, failed: boolean): never {
  process.stdout.write(JSON.stringify(obj, null, 2) + "\n");
  process.exit(failed ? 1 : 0);
}

const cmd = process.argv[2];
const projectDir = arg("project") ?? ".";

const project = await loadProject(path.resolve(projectDir));
const errors = project.diagnostics.filter((d) => d.level === "error");

if (cmd === "compile") {
  const failed = errors.length > 0;
  if (!failed) {
    const outFile = arg("out") ?? path.join(project.dir, ".tide", "bundle.json");
    fs.mkdirSync(path.dirname(outFile), { recursive: true });
    fs.writeFileSync(outFile, JSON.stringify(project.bundle, null, 2));
  }
  out(
    {
      ok: !failed,
      bundle: { name: project.bundle.name, version: project.bundle.version, actions: Object.keys(project.bundle.actions).length, rules: project.bundle.rules.length },
      diagnostics: project.diagnostics,
    },
    failed,
  );
}

if (errors.length > 0) out({ ok: false, diagnostics: project.diagnostics }, true);
const engine = new Engine(project.bundle, project.resolvers);

if (cmd === "test") {
  const scenarios = loadScenarios(project.dir);
  const firedIds = new Set<string>();
  const results = scenarios.map((s) => {
    const decision = engine.decide({
      action: s.action,
      params: s.params ?? {},
      context: s.context ?? {},
      history: s.session ?? [],
    });
    decision.fired.forEach((f) => firedIds.add(f.id));
    decision.shadow.forEach((f) => firedIds.add(f.id));
    const missingRules = (s.expect_rules ?? []).filter(
      (id) => !decision.fired.some((f) => f.id === id) && !decision.shadow.some((f) => f.id === id),
    );
    const pass = decision.verdict === s.expect && missingRules.length === 0;
    return { name: s.name, expect: s.expect, got: decision.verdict, pass, missing_rules: missingRules, reasons: decision.reasons };
  });
  const ruleIds = project.bundle.rules.map((r) => r.id);
  const covered = ruleIds.filter((id) => firedIds.has(id));
  const failed = results.some((r) => !r.pass) || results.length === 0;
  out(
    {
      ok: !failed,
      total: results.length,
      passed: results.filter((r) => r.pass).length,
      results,
      coverage: { rules: ruleIds.length, covered: covered.length, uncovered: ruleIds.filter((id) => !firedIds.has(id)) },
    },
    failed,
  );
}

out({ ok: false, error: `unknown command '${cmd}' (expected compile|test)` }, true);
