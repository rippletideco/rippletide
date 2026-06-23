// The decision engine: pure and deterministic. Given a compiled bundle, a proposed
// action, and the session's prior (allowed) actions, produce a verdict.
// No I/O here — resolvers are injected as plain functions.

import { Env, Value, ExprError, parse, evaluate, interpolate, Node } from "./expr.js";

export type Verdict = "allow" | "deny" | "escalate";
export type RuleMode = "enforce" | "shadow";

export interface ActionDef {
  description?: string;
  params?: Record<string, string>; // name -> type label (informational in v0)
}

export interface Rule {
  id: string;
  action: string; // action name or "*"
  when?: string; // TideExpr predicate
  requires?: string[]; // sequencing: actions that must have occurred earlier this session
  verdict: Verdict;
  mode: RuleMode;
  reason?: string; // template, interpolated with {expr}
  to?: string; // escalation group
}

export interface Bundle {
  name: string;
  version: string;
  actions: Record<string, ActionDef>;
  rules: Rule[];
}

export interface DecisionInput {
  action: string;
  params: Record<string, Value>;
  context: Record<string, Value>;
  history: string[]; // actions already allowed/approved in this session, in order
}

export interface FiredRule {
  id: string;
  verdict: Verdict;
  mode: RuleMode;
  reason: string;
  to?: string;
}

export interface Decision {
  verdict: Verdict;
  reasons: string[];
  to?: string; // escalation group when verdict is escalate
  fired: FiredRule[]; // enforce-mode rules that fired (drive the verdict)
  shadow: FiredRule[]; // shadow-mode rules that fired (recorded, never applied)
  errors: string[]; // expression errors (enforce-mode errors fail closed)
}

export type Resolvers = Record<string, Value>;

// Compiled rule cache: parse each `when` once.
const PRECEDENCE: Record<Verdict, number> = { allow: 0, escalate: 1, deny: 2 };

export class Engine {
  private parsed = new Map<string, Node>();

  constructor(public bundle: Bundle, private resolvers: Resolvers = {}) {
    for (const rule of bundle.rules) {
      if (rule.when) this.parsed.set(rule.id, parse(rule.when));
    }
  }

  decide(input: DecisionInput): Decision {
    const fired: FiredRule[] = [];
    const shadow: FiredRule[] = [];
    const errors: string[] = [];

    if (!(input.action in this.bundle.actions)) {
      return {
        verdict: "deny",
        reasons: [`action '${input.action}' is not in the agent's declared action space`],
        fired: [{ id: "__unknown_action__", verdict: "deny", mode: "enforce", reason: "unknown action" }],
        shadow: [],
        errors: [],
      };
    }

    const env: Env = {
      params: input.params,
      context: input.context,
      session: {
        actions: input.history,
        has: ((name: Value) => typeof name === "string" && input.history.includes(name)) as Value,
        count: ((name: Value) => input.history.filter((a) => a === name).length) as Value,
      },
      resolvers: this.resolvers,
    };

    let failClosed = false;

    for (const rule of this.bundle.rules) {
      if (rule.action !== "*" && rule.action !== input.action) continue;

      let hit = false;
      let reason = rule.reason ?? `rule ${rule.id} fired`;

      if (rule.requires && rule.requires.length > 0) {
        const missing = rule.requires.filter((a) => !input.history.includes(a));
        if (missing.length > 0) {
          hit = true;
          reason = rule.reason ?? `requires [${missing.join(", ")}] earlier in this session`;
        }
      } else if (rule.when) {
        try {
          const v = evaluate(this.parsed.get(rule.id)!, env);
          if (v === true) hit = true;
          else if (v !== false && v !== null) throw new ExprError(`'when' must evaluate to a boolean, got ${JSON.stringify(v)}`);
        } catch (e) {
          const msg = `rule '${rule.id}': ${(e as Error).message}`;
          errors.push(msg);
          // fail closed: an unevaluable enforce-mode rule denies rather than silently passing
          if (rule.mode === "enforce") failClosed = true;
          continue;
        }
      }

      if (!hit) continue;
      const entry: FiredRule = {
        id: rule.id,
        verdict: rule.verdict,
        mode: rule.mode,
        reason: interpolate(reason, env),
        ...(rule.to ? { to: rule.to } : {}),
      };
      (rule.mode === "shadow" ? shadow : fired).push(entry);
    }

    if (failClosed) {
      return {
        verdict: "deny",
        reasons: ["a rule could not be evaluated; failing closed", ...errors],
        fired,
        shadow,
        errors,
      };
    }

    let verdict: Verdict = "allow";
    let to: string | undefined;
    for (const f of fired) {
      if (PRECEDENCE[f.verdict] > PRECEDENCE[verdict]) {
        verdict = f.verdict;
        to = f.to;
      }
    }
    const reasons = fired.filter((f) => f.verdict === verdict && verdict !== "allow").map((f) => f.reason);
    return { verdict, reasons, ...(to ? { to } : {}), fired, shadow, errors };
  }
}

// ---------- Bundle validation (compile-time) ----------

export interface Diagnostic {
  level: "error" | "warning";
  where: string;
  message: string;
}

const VERDICTS: Verdict[] = ["allow", "deny", "escalate"];
const MODES: RuleMode[] = ["enforce", "shadow"];

export function validateBundle(bundle: Bundle): Diagnostic[] {
  const diags: Diagnostic[] = [];
  const ids = new Set<string>();
  for (const rule of bundle.rules) {
    const where = `rule '${rule.id ?? "?"}'`;
    if (!rule.id) diags.push({ level: "error", where, message: "missing id" });
    else if (ids.has(rule.id)) diags.push({ level: "error", where, message: "duplicate rule id" });
    else ids.add(rule.id);

    if (!rule.action) diags.push({ level: "error", where, message: "missing action" });
    else if (rule.action !== "*" && !(rule.action in bundle.actions))
      diags.push({ level: "error", where, message: `action '${rule.action}' not declared in ontology/actions.yaml` });

    if (!VERDICTS.includes(rule.verdict)) diags.push({ level: "error", where, message: `invalid verdict '${rule.verdict}'` });
    if (!MODES.includes(rule.mode)) diags.push({ level: "error", where, message: `invalid mode '${rule.mode}'` });
    if (rule.verdict === "escalate" && !rule.to)
      diags.push({ level: "warning", where, message: "escalate rule has no 'to' group; approvals will be unrouted" });

    if (rule.when && rule.requires)
      diags.push({ level: "error", where, message: "a rule may have 'when' or 'requires', not both" });
    if (!rule.when && !rule.requires)
      diags.push({ level: "error", where, message: "a rule needs a 'when' predicate or a 'requires' precondition" });

    if (rule.requires)
      for (const a of rule.requires)
        if (!(a in bundle.actions))
          diags.push({ level: "error", where, message: `requires unknown action '${a}'` });

    if (rule.when) {
      try {
        parse(rule.when);
      } catch (e) {
        diags.push({ level: "error", where, message: `invalid expression: ${(e as Error).message}` });
      }
    }
  }
  return diags;
}
