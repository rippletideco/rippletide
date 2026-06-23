import { test } from "node:test";
import assert from "node:assert/strict";
import { Engine, Bundle, validateBundle } from "../src/engine.js";

const bundle: Bundle = {
  name: "test",
  version: "1",
  actions: {
    read_record: { params: { record_id: "string" } },
    write_record: { params: { value: "number", record_id: "string" } },
    add_note: { params: { body: "string" } },
  },
  rules: [
    {
      id: "write-requires-read",
      action: "write_record",
      requires: ["read_record"],
      verdict: "deny",
      mode: "enforce",
      reason: "Writes require reading the record first",
    },
    {
      id: "write-limit",
      action: "write_record",
      when: "params.value > resolvers.policy.limit(context.record)",
      verdict: "deny",
      mode: "enforce",
      reason: "Value {params.value} exceeds policy limit",
    },
    {
      id: "write-velocity",
      action: "write_record",
      when: "resolvers.policy.total_30d(context.record) + params.value > 1000",
      verdict: "escalate",
      to: "ops-review",
      mode: "enforce",
    },
    {
      id: "sensitive-note-watch",
      action: "add_note",
      when: "text_matches(params.body, 'restricted')",
      verdict: "escalate",
      to: "ops-review",
      mode: "shadow",
    },
  ],
};

const resolvers = {
  policy: {
    limit: (r: any) => (r?.tier === "A" ? 1000 : 250),
    total_30d: (_r: any) => 700,
  },
} as any;

const engine = new Engine(bundle, resolvers);
const record = { id: "REC-7", tier: "B" };

test("allow when no rule fires", () => {
  const d = engine.decide({ action: "read_record", params: { record_id: "REC-7" }, context: {}, history: [] });
  assert.equal(d.verdict, "allow");
  assert.equal(d.fired.length, 0);
});

test("sequencing rule denies write without prior read", () => {
  const d = engine.decide({ action: "write_record", params: { value: 50 }, context: { record }, history: [] });
  assert.equal(d.verdict, "deny");
  assert.ok(d.fired.some((f) => f.id === "write-requires-read"));
});

test("over-limit write denied; deny outranks escalate", () => {
  const d = engine.decide({
    action: "write_record",
    params: { value: 480 },
    context: { record },
    history: ["read_record"],
  });
  assert.equal(d.verdict, "deny");
  assert.match(d.reasons[0], /480 exceeds policy limit/);
  assert.ok(d.fired.some((f) => f.id === "write-velocity"));
});

test("velocity rule escalates an in-limit write", () => {
  const d = engine.decide({
    action: "write_record",
    params: { value: 400 },
    context: { record: { id: "REC-9", tier: "A" } },
    history: ["read_record"],
  });
  assert.equal(d.verdict, "escalate");
  assert.equal(d.to, "ops-review");
});

test("shadow rule records but never blocks", () => {
  const d = engine.decide({
    action: "add_note",
    params: { body: "This contains restricted wording" },
    context: {},
    history: [],
  });
  assert.equal(d.verdict, "allow");
  assert.equal(d.shadow.length, 1);
  assert.equal(d.shadow[0].id, "sensitive-note-watch");
});

test("unknown action denied (default-deny outside the action space)", () => {
  const d = engine.decide({ action: "delete_database", params: {}, context: {}, history: [] });
  assert.equal(d.verdict, "deny");
  assert.match(d.reasons[0], /not in the agent's declared action space/);
});

test("enforce-mode expression error fails closed", () => {
  const broken = new Engine(
    {
      ...bundle,
      rules: [{ id: "broken", action: "write_record", when: "params.value > resolvers.nope.fn()", verdict: "deny", mode: "enforce" }],
    },
    resolvers,
  );
  const d = broken.decide({ action: "write_record", params: { value: 1 }, context: {}, history: [] });
  assert.equal(d.verdict, "deny");
  assert.ok(d.errors.length > 0);
});

test("validateBundle catches structural problems", () => {
  const diags = validateBundle({
    name: "x",
    version: "1",
    actions: { a: {} },
    rules: [
      { id: "r1", action: "missing_action", when: "true", verdict: "deny", mode: "enforce" },
      { id: "r1", action: "a", when: "1 +", verdict: "deny", mode: "enforce" },
      { id: "r3", action: "a", verdict: "deny", mode: "enforce" },
      { id: "r4", action: "a", when: "true", requires: ["a"], verdict: "deny", mode: "enforce" },
      { id: "r5", action: "a", when: "true", verdict: "escalate", mode: "enforce" },
    ],
  });
  const msgs = diags.map((d) => d.message).join("\n");
  assert.match(msgs, /not declared in ontology/);
  assert.match(msgs, /duplicate rule id/);
  assert.match(msgs, /invalid expression/);
  assert.match(msgs, /needs a 'when' predicate or a 'requires'/);
  assert.match(msgs, /'when' or 'requires', not both/);
  assert.match(msgs, /no 'to' group/);
});
