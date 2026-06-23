#!/usr/bin/env node
// Tide Decision Runtime — the enforcement point agents call before acting.
//
//   node dist/src/server.js --project <dir> [--port 8787] [--host 127.0.0.1]
//
// POST /v1/decisions             propose an action → allow | deny | escalate
// GET  /v1/decisions/:trace_id   poll an escalated decision's status
// GET  /v1/approvals             pending approvals (the Console queue)
// POST /v1/approvals/:id         {decision: "approve"|"reject", approver}
// GET  /v1/traces                decision feed (most recent first)
// GET  /v1/bundle                active bundle summary
// GET  /                         Console (static)

import Fastify from "fastify";
import fastifyStatic from "@fastify/static";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { Engine } from "./engine.js";
import { loadProject } from "./project.js";
import { Value } from "./expr.js";
import { redactSensitive } from "./redact.js";

function arg(name: string, fallback?: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : fallback;
}

const projectDir = path.resolve(arg("project", ".")!);
const port = Number(arg("port", "8787"));
const host = arg("host", process.env.TIDE_HOST ?? "127.0.0.1")!;
const unsafeBindPublic = process.argv.includes("--unsafe-bind-public") || process.env.TIDE_UNSAFE_BIND_PUBLIC === "1";

function isLoopbackHost(value: string): boolean {
  return value === "localhost" || value === "127.0.0.1" || value === "::1";
}

if (!isLoopbackHost(host) && !unsafeBindPublic) {
  console.error("refusing to bind unauthenticated runtime to a non-loopback host; pass --unsafe-bind-public only on a trusted network");
  process.exit(1);
}

const project = await loadProject(projectDir);
const errors = project.diagnostics.filter((d) => d.level === "error");
if (errors.length > 0) {
  console.error("bundle has errors; refusing to serve:");
  for (const d of errors) console.error(`  [${d.level}] ${d.where}: ${d.message}`);
  process.exit(1);
}
const engine = new Engine(project.bundle, project.resolvers);

interface TraceRecord {
  trace_id: string;
  ts: string;
  actor_id?: string;
  agent_id: string;
  session_id: string;
  action: string;
  params: Record<string, Value>;
  context: Record<string, Value>;
  metadata?: Record<string, Value>;
  idempotency_key?: string;
  verdict: string;
  status: "final" | "pending" | "approved" | "rejected";
  reasons: string[];
  fired: unknown[];
  shadow: unknown[];
  latency_ms: number;
  approval_id?: string;
}

interface Approval {
  approval_id: string;
  trace_id: string;
  ts: string;
  session_id: string;
  agent_id: string;
  action: string;
  params: Record<string, Value>;
  to: string;
  status: "pending" | "approved" | "rejected";
  approver?: string;
  decided_ts?: string;
}

const sessions = new Map<string, string[]>(); // session_id -> allowed action history
const traces: TraceRecord[] = [];
const tracesById = new Map<string, TraceRecord>();
const approvals = new Map<string, Approval>();
// ponytail: in-memory idempotency; persist it with traces before multi-instance runtime.
const idempotency = new Map<string, string>(); // session_id:idempotency_key -> trace_id

const varDir = path.join(projectDir, "var");
fs.mkdirSync(varDir, { recursive: true });
const traceLog = fs.createWriteStream(path.join(varDir, "decisions.jsonl"), { flags: "a" });

const app = Fastify({ logger: false });

const publicDir = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "public");
await app.register(fastifyStatic, { root: publicDir });

function publicDecision(record: TraceRecord) {
  const route = (record.fired as Array<{ to?: string }>).find((f) => f.to)?.to;
  return {
    trace_id: record.trace_id,
    verdict: record.status === "approved" ? "allow" : record.status === "rejected" ? "deny" : record.verdict,
    reasons: record.reasons,
    ...(record.approval_id ? { approval_id: record.approval_id, to: route ?? "unrouted" } : {}),
    latency_ms: record.latency_ms,
  };
}

app.post("/v1/decisions", async (req, reply) => {
  const body = (req.body ?? {}) as {
    actor_id?: string;
    agent_id?: string;
    session_id?: string;
    action?: string;
    tool?: string;
    params?: Record<string, Value>;
    context?: Record<string, Value>;
    metadata?: Record<string, Value>;
    idempotency_key?: string;
  };
  const action = body.action ?? body.tool;
  if (!action) return reply.code(400).send({ error: "action is required" });
  const sessionId = body.session_id ?? "default";
  const idempotencyKey = body.idempotency_key ? `${sessionId}:${body.idempotency_key}` : undefined;
  if (idempotencyKey) {
    const prior = tracesById.get(idempotency.get(idempotencyKey) ?? "");
    if (prior) return publicDecision(prior);
  }
  const history = sessions.get(sessionId) ?? [];

  const t0 = process.hrtime.bigint();
  const decision = engine.decide({
    action,
    params: body.params ?? {},
    context: body.context ?? {},
    history,
  });
  const latencyMs = Number(process.hrtime.bigint() - t0) / 1e6;
  const safeParams = redactSensitive(body.params ?? {}) as Record<string, Value>;
  const safeContext = redactSensitive(body.context ?? {}) as Record<string, Value>;
  const safeMetadata = body.metadata ? redactSensitive(body.metadata) as Record<string, Value> : undefined;

  const traceId = "tr_" + randomUUID().slice(0, 12);
  const record: TraceRecord = {
    trace_id: traceId,
    ts: new Date().toISOString(),
    ...(body.actor_id ? { actor_id: body.actor_id } : {}),
    agent_id: body.agent_id ?? "unknown",
    session_id: sessionId,
    action,
    params: safeParams,
    context: safeContext,
    ...(safeMetadata ? { metadata: safeMetadata } : {}),
    ...(body.idempotency_key ? { idempotency_key: body.idempotency_key } : {}),
    verdict: decision.verdict,
    status: "final",
    reasons: decision.reasons,
    fired: decision.fired,
    shadow: decision.shadow,
    latency_ms: Math.round(latencyMs * 100) / 100,
  };

  if (decision.verdict === "allow") {
    history.push(action);
    sessions.set(sessionId, history);
  } else if (decision.verdict === "escalate") {
    const approvalId = "ap_" + randomUUID().slice(0, 12);
    record.status = "pending";
    record.approval_id = approvalId;
    approvals.set(approvalId, {
      approval_id: approvalId,
      trace_id: traceId,
      ts: record.ts,
      session_id: sessionId,
      agent_id: record.agent_id,
      action,
      params: safeParams,
      to: decision.to ?? "unrouted",
      status: "pending",
    });
  }

  traces.unshift(record);
  tracesById.set(traceId, record);
  if (idempotencyKey) idempotency.set(idempotencyKey, traceId);
  traceLog.write(JSON.stringify(record) + "\n");

  return publicDecision(record);
});

app.get("/v1/decisions/:trace_id", async (req, reply) => {
  const { trace_id } = req.params as { trace_id: string };
  const record = tracesById.get(trace_id);
  if (!record) return reply.code(404).send({ error: "unknown trace_id" });
  return {
    trace_id,
    verdict: record.status === "approved" ? "allow" : record.status === "rejected" ? "deny" : record.verdict,
    status: record.status,
    reasons: record.reasons,
  };
});

app.get("/v1/approvals", async (req) => {
  const { status } = (req.query ?? {}) as { status?: string };
  const all = [...approvals.values()].sort((a, b) => (a.ts < b.ts ? 1 : -1));
  return status ? all.filter((a) => a.status === status) : all;
});

app.post("/v1/approvals/:id", async (req, reply) => {
  const { id } = req.params as { id: string };
  const body = (req.body ?? {}) as { decision?: string; approver?: string };
  const approval = approvals.get(id);
  if (!approval) return reply.code(404).send({ error: "unknown approval id" });
  if (approval.status !== "pending") return reply.code(409).send({ error: `already ${approval.status}` });
  if (body.decision !== "approve" && body.decision !== "reject")
    return reply.code(400).send({ error: "decision must be 'approve' or 'reject'" });

  approval.status = body.decision === "approve" ? "approved" : "rejected";
  approval.approver = body.approver ?? "console";
  approval.decided_ts = new Date().toISOString();

  const record = tracesById.get(approval.trace_id);
  if (record) {
    record.status = approval.status;
    if (approval.status === "approved") {
      // an approved action counts as having happened, so sequencing rules see it
      const history = sessions.get(approval.session_id) ?? [];
      history.push(approval.action);
      sessions.set(approval.session_id, history);
    }
    traceLog.write(JSON.stringify({ ...record, event: "approval_decided", approver: approval.approver }) + "\n");
  }
  return approval;
});

app.get("/v1/traces", async (req) => {
  const { limit } = (req.query ?? {}) as { limit?: string };
  return traces.slice(0, Math.min(Number(limit ?? 100), 500));
});

app.get("/v1/traces/:id", async (req, reply) => {
  const { id } = req.params as { id: string };
  return tracesById.get(id) ?? reply.code(404).send({ error: "unknown trace" });
});

app.get("/v1/bundle", async () => ({
  name: project.bundle.name,
  version: project.bundle.version,
  actions: Object.keys(project.bundle.actions),
  rules: project.bundle.rules.map((r) => ({ id: r.id, action: r.action, verdict: r.verdict, mode: r.mode })),
  warnings: project.diagnostics.filter((d) => d.level === "warning"),
}));

await app.listen({ port, host });
console.log(`tide runtime: bundle '${project.bundle.name}' (${project.bundle.rules.length} rules) on http://${host}:${port}`);
