// TideExpr — a small, deterministic expression language (CEL subset).
// No I/O, no clock, no randomness: same input → same verdict, always.
//
// Grammar (precedence low → high):
//   or:          and ("||" and)*
//   and:         equality ("&&" equality)*
//   equality:    comparison (("==" | "!=") comparison)*
//   comparison:  additive (("<" | "<=" | ">" | ">=" | "in") additive)*
//   additive:    multiplicative (("+" | "-") multiplicative)*
//   multiplicative: unary (("*" | "/" | "%") unary)*
//   unary:       ("!" | "-") unary | postfix
//   postfix:     primary ("." ident | "[" expr "]" | "(" args ")")*
//   primary:     number | string | true | false | null | ident | "(" expr ")" | "[" args "]"

export type Value =
  | number
  | string
  | boolean
  | null
  | Value[]
  | { [key: string]: Value }
  | ExprFn;

export type ExprFn = (...args: Value[]) => Value;

export class ExprError extends Error {}

// ---------- Tokenizer ----------

type Token =
  | { kind: "num"; value: number }
  | { kind: "str"; value: string }
  | { kind: "ident"; value: string }
  | { kind: "op"; value: string };

const OPS = ["||", "&&", "==", "!=", "<=", ">=", "<", ">", "+", "-", "*", "/", "%", "!", "(", ")", "[", "]", ",", "."];

function tokenize(src: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === " " || c === "\t" || c === "\n" || c === "\r") { i++; continue; }
    if (c >= "0" && c <= "9") {
      let j = i;
      while (j < src.length && /[0-9._]/.test(src[j])) {
        // a dot is part of the number only if followed by a digit (else it's member access)
        if (src[j] === "." && !/[0-9]/.test(src[j + 1] ?? "")) break;
        j++;
      }
      const raw = src.slice(i, j).replace(/_/g, "");
      const n = Number(raw);
      if (Number.isNaN(n)) throw new ExprError(`invalid number '${raw}'`);
      tokens.push({ kind: "num", value: n });
      i = j;
      continue;
    }
    if (c === '"' || c === "'") {
      let j = i + 1;
      let out = "";
      while (j < src.length && src[j] !== c) {
        if (src[j] === "\\" && j + 1 < src.length) { out += src[j + 1]; j += 2; }
        else { out += src[j]; j++; }
      }
      if (j >= src.length) throw new ExprError("unterminated string");
      tokens.push({ kind: "str", value: out });
      i = j + 1;
      continue;
    }
    if (/[A-Za-z_]/.test(c)) {
      let j = i;
      while (j < src.length && /[A-Za-z0-9_]/.test(src[j])) j++;
      tokens.push({ kind: "ident", value: src.slice(i, j) });
      i = j;
      continue;
    }
    const op = OPS.find((o) => src.startsWith(o, i));
    if (!op) throw new ExprError(`unexpected character '${c}' at ${i}`);
    tokens.push({ kind: "op", value: op });
    i += op.length;
  }
  return tokens;
}

// ---------- Parser (AST) ----------

export type Node =
  | { t: "lit"; v: Value }
  | { t: "ident"; name: string }
  | { t: "member"; obj: Node; name: string }
  | { t: "index"; obj: Node; idx: Node }
  | { t: "call"; fn: Node; args: Node[] }
  | { t: "list"; items: Node[] }
  | { t: "unary"; op: string; e: Node }
  | { t: "bin"; op: string; l: Node; r: Node };

class Parser {
  pos = 0;
  constructor(private tokens: Token[]) {}

  peek(): Token | undefined { return this.tokens[this.pos]; }
  isOp(v: string): boolean { const t = this.peek(); return !!t && t.kind === "op" && t.value === v; }
  isIdent(v: string): boolean { const t = this.peek(); return !!t && t.kind === "ident" && t.value === v; }
  expectOp(v: string): void {
    if (!this.isOp(v)) throw new ExprError(`expected '${v}'`);
    this.pos++;
  }

  parse(): Node {
    const n = this.or();
    if (this.pos < this.tokens.length) throw new ExprError("unexpected trailing tokens");
    return n;
  }

  or(): Node {
    let l = this.and();
    while (this.isOp("||")) { this.pos++; l = { t: "bin", op: "||", l, r: this.and() }; }
    return l;
  }
  and(): Node {
    let l = this.equality();
    while (this.isOp("&&")) { this.pos++; l = { t: "bin", op: "&&", l, r: this.equality() }; }
    return l;
  }
  equality(): Node {
    let l = this.comparison();
    while (this.isOp("==") || this.isOp("!=")) {
      const op = (this.peek() as { value: string }).value;
      this.pos++;
      l = { t: "bin", op, l, r: this.comparison() };
    }
    return l;
  }
  comparison(): Node {
    let l = this.additive();
    while (this.isOp("<") || this.isOp("<=") || this.isOp(">") || this.isOp(">=") || this.isIdent("in")) {
      const t = this.peek()!;
      const op = t.kind === "ident" ? "in" : String(t.value);
      this.pos++;
      l = { t: "bin", op, l, r: this.additive() };
    }
    return l;
  }
  additive(): Node {
    let l = this.multiplicative();
    while (this.isOp("+") || this.isOp("-")) {
      const op = (this.peek() as { value: string }).value;
      this.pos++;
      l = { t: "bin", op, l, r: this.multiplicative() };
    }
    return l;
  }
  multiplicative(): Node {
    let l = this.unary();
    while (this.isOp("*") || this.isOp("/") || this.isOp("%")) {
      const op = (this.peek() as { value: string }).value;
      this.pos++;
      l = { t: "bin", op, l, r: this.unary() };
    }
    return l;
  }
  unary(): Node {
    if (this.isOp("!")) { this.pos++; return { t: "unary", op: "!", e: this.unary() }; }
    if (this.isOp("-")) { this.pos++; return { t: "unary", op: "-", e: this.unary() }; }
    return this.postfix();
  }
  postfix(): Node {
    let n = this.primary();
    for (;;) {
      if (this.isOp(".")) {
        this.pos++;
        const t = this.peek();
        if (!t || t.kind !== "ident") throw new ExprError("expected identifier after '.'");
        this.pos++;
        n = { t: "member", obj: n, name: t.value };
      } else if (this.isOp("[")) {
        this.pos++;
        const idx = this.or();
        this.expectOp("]");
        n = { t: "index", obj: n, idx };
      } else if (this.isOp("(")) {
        this.pos++;
        const args: Node[] = [];
        if (!this.isOp(")")) {
          args.push(this.or());
          while (this.isOp(",")) { this.pos++; args.push(this.or()); }
        }
        this.expectOp(")");
        n = { t: "call", fn: n, args };
      } else break;
    }
    return n;
  }
  primary(): Node {
    const t = this.peek();
    if (!t) throw new ExprError("unexpected end of expression");
    if (t.kind === "num") { this.pos++; return { t: "lit", v: t.value }; }
    if (t.kind === "str") { this.pos++; return { t: "lit", v: t.value }; }
    if (t.kind === "ident") {
      this.pos++;
      if (t.value === "true") return { t: "lit", v: true };
      if (t.value === "false") return { t: "lit", v: false };
      if (t.value === "null") return { t: "lit", v: null };
      return { t: "ident", name: t.value };
    }
    if (t.kind === "op" && t.value === "(") {
      this.pos++;
      const n = this.or();
      this.expectOp(")");
      return n;
    }
    if (t.kind === "op" && t.value === "[") {
      this.pos++;
      const items: Node[] = [];
      if (!this.isOp("]")) {
        items.push(this.or());
        while (this.isOp(",")) { this.pos++; items.push(this.or()); }
      }
      this.expectOp("]");
      return { t: "list", items };
    }
    throw new ExprError(`unexpected token '${"value" in t ? t.value : "?"}'`);
  }
}

export function parse(src: string): Node {
  return new Parser(tokenize(src)).parse();
}

// ---------- Evaluator ----------

export type Env = { [key: string]: Value };

// Deterministic builtins. Anything time-, network-, or randomness-dependent is banned by design.
export const BUILTINS: Env = {
  len: (v: Value) => {
    if (typeof v === "string") return v.length;
    if (Array.isArray(v)) return v.length;
    throw new ExprError("len() expects string or list");
  },
  lower: (v: Value) => {
    if (typeof v !== "string") throw new ExprError("lower() expects string");
    return v.toLowerCase();
  },
  upper: (v: Value) => {
    if (typeof v !== "string") throw new ExprError("upper() expects string");
    return v.toUpperCase();
  },
  abs: (v: Value) => {
    if (typeof v !== "number") throw new ExprError("abs() expects number");
    return Math.abs(v);
  },
  min: (...args: Value[]) => {
    if (!args.length || args.some((a) => typeof a !== "number")) throw new ExprError("min() expects numbers");
    return Math.min(...(args as number[]));
  },
  max: (...args: Value[]) => {
    if (!args.length || args.some((a) => typeof a !== "number")) throw new ExprError("max() expects numbers");
    return Math.max(...(args as number[]));
  },
  // case-insensitive regex match; reject patterns that are too broad for a tiny local runtime
  text_matches: (s: Value, pattern: Value) => {
    if (typeof s !== "string" || typeof pattern !== "string") throw new ExprError("text_matches(s, pattern) expects strings");
    if (pattern.length > 256 || /\([^)]*[+*][^)]*\)[+*?{]/.test(pattern)) throw new ExprError("text_matches pattern is too complex");
    return new RegExp(pattern, "i").test(s);
  },
  contains: (hay: Value, needle: Value) => {
    if (typeof hay === "string" && typeof needle === "string") return hay.toLowerCase().includes(needle.toLowerCase());
    if (Array.isArray(hay)) return hay.some((v) => deepEq(v, needle));
    throw new ExprError("contains() expects (string,string) or (list,value)");
  },
  round: (v: Value) => {
    if (typeof v !== "number") throw new ExprError("round() expects number");
    return Math.round(v);
  },
};

function deepEq(a: Value, b: Value): boolean {
  if (a === b) return true;
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((v, i) => deepEq(v, b[i]));
  if (a && b && typeof a === "object" && typeof b === "object" && !Array.isArray(a) && !Array.isArray(b)) {
    const ka = Object.keys(a), kb = Object.keys(b);
    return ka.length === kb.length && ka.every((k) => deepEq((a as any)[k], (b as any)[k]));
  }
  return false;
}

function truthy(v: Value): boolean {
  if (v === null) return false;
  if (typeof v === "boolean") return v;
  throw new ExprError(`expected boolean, got ${JSON.stringify(v)}`);
}

function num(v: Value, op: string): number {
  if (typeof v !== "number") throw new ExprError(`'${op}' expects numbers, got ${JSON.stringify(v)}`);
  return v;
}

export function evaluate(node: Node, env: Env): Value {
  switch (node.t) {
    case "lit":
      return node.v;
    case "ident": {
      if (node.name in env) return env[node.name];
      if (node.name in BUILTINS) return BUILTINS[node.name];
      throw new ExprError(`unknown identifier '${node.name}'`);
    }
    case "member": {
      const obj = evaluate(node.obj, env);
      // missing member resolves to null (CEL-style leniency for sparse context);
      // member access on a non-object is a hard error (likely a typo in the rule)
      if (obj === null) return null;
      if (typeof obj !== "object" || Array.isArray(obj)) throw new ExprError(`cannot access '.${node.name}' on ${JSON.stringify(obj)}`);
      const v = (obj as { [k: string]: Value })[node.name];
      return v === undefined ? null : v;
    }
    case "index": {
      const obj = evaluate(node.obj, env);
      const idx = evaluate(node.idx, env);
      if (Array.isArray(obj)) {
        if (typeof idx !== "number") throw new ExprError("list index must be a number");
        return obj[idx] ?? null;
      }
      if (obj && typeof obj === "object") {
        if (typeof idx !== "string") throw new ExprError("object index must be a string");
        const v = (obj as { [k: string]: Value })[idx];
        return v === undefined ? null : v;
      }
      throw new ExprError("cannot index into " + JSON.stringify(obj));
    }
    case "call": {
      const fn = evaluate(node.fn, env);
      if (typeof fn !== "function") throw new ExprError("attempted to call a non-function");
      const args = node.args.map((a) => evaluate(a, env));
      return (fn as ExprFn)(...args);
    }
    case "list":
      return node.items.map((i) => evaluate(i, env));
    case "unary": {
      const v = evaluate(node.e, env);
      if (node.op === "!") return !truthy(v);
      return -num(v, "-");
    }
    case "bin": {
      if (node.op === "&&") return truthy(evaluate(node.l, env)) && truthy(evaluate(node.r, env));
      if (node.op === "||") return truthy(evaluate(node.l, env)) || truthy(evaluate(node.r, env));
      const l = evaluate(node.l, env);
      const r = evaluate(node.r, env);
      switch (node.op) {
        case "==": return deepEq(l, r);
        case "!=": return !deepEq(l, r);
        case "<": return num(l, "<") < num(r, "<");
        case "<=": return num(l, "<=") <= num(r, "<=");
        case ">": return num(l, ">") > num(r, ">");
        case ">=": return num(l, ">=") >= num(r, ">=");
        case "+": {
          if (typeof l === "string" && typeof r === "string") return l + r;
          return num(l, "+") + num(r, "+");
        }
        case "-": return num(l, "-") - num(r, "-");
        case "*": return num(l, "*") * num(r, "*");
        case "/": {
          const d = num(r, "/");
          if (d === 0) throw new ExprError("division by zero");
          return num(l, "/") / d;
        }
        case "%": {
          const d = num(r, "%");
          if (d === 0) throw new ExprError("modulo by zero");
          return num(l, "%") % d;
        }
        case "in": {
          if (Array.isArray(r)) return r.some((v) => deepEq(v, l));
          if (typeof r === "string" && typeof l === "string") return r.includes(l);
          if (r && typeof r === "object") return typeof l === "string" && l in (r as object);
          throw new ExprError("'in' expects list, string, or object on the right");
        }
      }
      throw new ExprError(`unknown operator '${node.op}'`);
    }
  }
}

export function evalString(src: string, env: Env): Value {
  return evaluate(parse(src), env);
}

// Interpolate "{expr}" segments inside reason templates, e.g.
// "Refund {params.amount} exceeds {context.customer.tier} cap"
export function interpolate(template: string, env: Env): string {
  return template.replace(/\{([^{}]+)\}/g, (_, expr) => {
    try {
      const v = evalString(expr.trim(), env);
      return typeof v === "string" ? v : JSON.stringify(v);
    } catch {
      return `{${expr}}`;
    }
  });
}
