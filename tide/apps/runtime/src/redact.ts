import { Value } from "./expr.js";

const SENSITIVE_KEY = /(api[_-]?key|authorization|bearer|credential|password|secret|token)/i;
const SENSITIVE_VALUE = /\b(sk-[A-Za-z0-9_-]{20,}|sk-proj-[A-Za-z0-9_-]+|xox[baprs]-[A-Za-z0-9-]+|gh[pousr]_[A-Za-z0-9_]+)\b/;

export function redactSensitive(value: Value, key = ""): Value {
  if (SENSITIVE_KEY.test(key)) return "[redacted]";
  if (typeof value === "string") return SENSITIVE_VALUE.test(value) ? "[redacted]" : value;
  if (Array.isArray(value)) return value.map((v) => redactSensitive(v));
  if (value && typeof value === "object") {
    const out: Record<string, Value> = {};
    for (const [k, v] of Object.entries(value)) out[k] = redactSensitive(v, k);
    return out;
  }
  return value;
}
