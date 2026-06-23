import { test } from "node:test";
import assert from "node:assert/strict";
import { evalString, interpolate, ExprError } from "../src/expr.js";

const env = {
  params: { value: 480, body: "This contains restricted wording", tags: ["vip", "eu"] },
  context: { record: { id: "REC-7", tier: "B", spend: 1200.5 } },
  resolvers: {
    policy: {
      limit: (r: any) => (r.tier === "A" ? 1000 : 250),
      total_30d: (_r: any) => 700,
    },
  } as any,
  session: { actions: ["read_record"], has: ((n: any) => n === "read_record") as any, count: ((_n: any) => 1) as any },
};

test("arithmetic and comparison", () => {
  assert.equal(evalString("1 + 2 * 3", env), 7);
  assert.equal(evalString("(1 + 2) * 3", env), 9);
  assert.equal(evalString("10 % 3", env), 1);
  assert.equal(evalString("params.value > 250", env), true);
  assert.equal(evalString("params.value <= 250", env), false);
});

test("member access, missing members resolve to null", () => {
  assert.equal(evalString("context.record.tier", env), "B");
  assert.equal(evalString("context.record.missing", env), null);
  assert.equal(evalString("context.record.missing == null", env), true);
  assert.equal(evalString("context.nope.deep.path == null", env), true);
});

test("indexing objects and lists", () => {
  assert.equal(evalString("params.tags[0]", env), "vip");
  assert.equal(evalString("context['record']['tier']", env), "B");
});

test("resolver calls", () => {
  assert.equal(evalString("resolvers.policy.limit(context.record)", env), 250);
  assert.equal(evalString("params.value > resolvers.policy.limit(context.record)", env), true);
  assert.equal(evalString("resolvers.policy.total_30d(context.record) + params.value > 1000", env), true);
});

test("boolean logic short-circuits and requires booleans", () => {
  assert.equal(evalString("true && params.value > 100", env), true);
  assert.equal(evalString("false || !false", env), true);
  assert.throws(() => evalString("1 && true", env), ExprError);
});

test("in operator", () => {
  assert.equal(evalString("'vip' in params.tags", env), true);
  assert.equal(evalString("'gold' in params.tags", env), false);
  assert.equal(evalString("context.record.tier in ['A', 'B']", env), true);
});

test("builtins", () => {
  assert.equal(evalString("len(params.tags)", env), 2);
  assert.equal(evalString("lower('ABC')", env), "abc");
  assert.equal(evalString("text_matches(params.body, 'restricted')", env), true);
  assert.equal(evalString("text_matches('all good', 'restricted')", env), false);
  assert.throws(() => evalString("text_matches(params.body, '(a+)+')", env), ExprError);
  assert.equal(evalString("min(3, 1, 2)", env), 1);
  assert.equal(evalString("contains(params.body, 'RESTRICTED')", env), true);
});

test("session helpers", () => {
  assert.equal(evalString("session.has('read_record')", env), true);
  assert.equal(evalString("'read_record' in session.actions", env), true);
});

test("strings: concat and comparison", () => {
  assert.equal(evalString("'a' + 'b'", env), "ab");
  assert.equal(evalString("context.record.tier == 'B'", env), true);
});

test("errors: unknown identifier, division by zero, calling non-function", () => {
  assert.throws(() => evalString("nonexistent + 1", env), ExprError);
  assert.throws(() => evalString("1 / 0", env), ExprError);
  assert.throws(() => evalString("params.amount(1)", env), ExprError);
});

test("interpolation", () => {
  assert.equal(
    interpolate("Value {params.value} exceeds {context.record.tier} limit {resolvers.policy.limit(context.record)}", env),
    "Value 480 exceeds B limit 250",
  );
  assert.equal(interpolate("bad {not.an.expr +} stays", env), "bad {not.an.expr +} stays");
});
