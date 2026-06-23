import { test } from "node:test";
import assert from "node:assert/strict";
import { redactSensitive } from "../src/redact.js";

test("redacts secret-looking keys and token-looking values", () => {
  const fakeGithubToken = "ghp_" + "a".repeat(36);
  assert.deepEqual(redactSensitive({
    api_key: "plain",
    nested: { body: `curl -H 'Authorization: token ${fakeGithubToken}'` },
    safe: "hello",
  }), {
    api_key: "[redacted]",
    nested: { body: "[redacted]" },
    safe: "hello",
  });
});
