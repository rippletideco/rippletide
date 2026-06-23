/* Tide Console — dependency-free v0 (React/tRPC console is Phase 2). */

const $ = (sel) => document.querySelector(sel);
let selectedTrace = null;

async function api(path, opts) {
  const res = await fetch(path, opts);
  return res.json();
}

function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function verdictChip(v) {
  return `<span class="verdict ${esc(v)}">${esc(v)}</span>`;
}

// ---- tabs ----
document.querySelectorAll(".tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    document.querySelectorAll("main .panel").forEach((p) => p.classList.add("hidden"));
    $("#tab-" + btn.dataset.tab).classList.remove("hidden");
  });
});

// ---- bundle ----
async function loadBundle() {
  const b = await api("/v1/bundle");
  $("#bundle-info").textContent = `bundle '${b.name}' v${b.version} · ${b.rules.length} rules · ${b.actions.length} actions`;
  const tbody = $("#rules-table tbody");
  tbody.innerHTML = b.rules
    .map((r) => `<tr><td>${esc(r.id)}</td><td>${esc(r.action)}</td><td>${verdictChip(r.verdict)}</td><td>${verdictChip(r.mode === "shadow" ? "shadow" : r.verdict === "allow" ? "allow" : "deny").replace(/>[^<]+</, ">" + esc(r.mode) + "<")}</td></tr>`)
    .join("");
}

// ---- decisions ----
async function refreshDecisions() {
  const traces = await api("/v1/traces?limit=100");
  const list = $("#decision-list");
  list.innerHTML = traces
    .map((t) => {
      const status = t.status !== "final" ? t.status : t.verdict;
      const shadowNote = t.shadow && t.shadow.length ? ` <span class="verdict shadow">+${t.shadow.length} shadow</span>` : "";
      return `<li data-id="${esc(t.trace_id)}" class="${selectedTrace === t.trace_id ? "selected" : ""}">
        <div class="row">${verdictChip(status)}<span class="action">${esc(t.action)}</span>${shadowNote}<span class="latency">${t.latency_ms} ms</span></div>
        <div class="meta">${esc(t.session_id)} · ${esc(t.agent_id)} · ${new Date(t.ts).toLocaleTimeString()}</div>
        ${t.reasons && t.reasons.length ? `<div class="reason">${esc(t.reasons[0])}</div>` : ""}
      </li>`;
    })
    .join("");
  list.querySelectorAll("li").forEach((li) =>
    li.addEventListener("click", () => {
      selectedTrace = li.dataset.id;
      showTrace(li.dataset.id);
      refreshDecisions();
    }),
  );
}

async function showTrace(id) {
  const t = await api("/v1/traces/" + id);
  const fired = (t.fired || [])
    .map((f) => `<div class="fired ${esc(f.verdict)}"><div class="rid">${esc(f.id)} → ${esc(f.verdict)}</div><div class="rr">${esc(f.reason)}</div></div>`)
    .join("");
  const shadow = (t.shadow || [])
    .map((f) => `<div class="fired shadow"><div class="rid">${esc(f.id)} (shadow → would ${esc(f.verdict)})</div><div class="rr">${esc(f.reason)}</div></div>`)
    .join("");
  $("#trace-detail").innerHTML = `
    <h3>${esc(t.action)} ${verdictChip(t.status !== "final" ? t.status : t.verdict)}</h3>
    <div class="meta">${esc(t.trace_id)} · ${esc(t.ts)} · evaluated in ${t.latency_ms} ms</div>
    <div class="section"><h4>Causal trace</h4>${fired || '<div class="meta">no enforce-mode rules fired — inside the envelope</div>'}</div>
    ${shadow ? `<div class="section"><h4>Shadow findings (recorded, not applied)</h4>${shadow}</div>` : ""}
    <div class="section"><h4>Proposed params</h4><pre>${esc(JSON.stringify(t.params, null, 2))}</pre></div>
    <div class="section"><h4>Context</h4><pre>${esc(JSON.stringify(t.context, null, 2))}</pre></div>`;
}

// ---- approvals ----
async function refreshApprovals() {
  const all = await api("/v1/approvals");
  const pending = all.filter((a) => a.status === "pending");
  const decided = all.filter((a) => a.status !== "pending");
  const badge = $("#pending-badge");
  badge.textContent = pending.length;
  badge.classList.toggle("hidden", pending.length === 0);

  $("#approval-list").innerHTML =
    pending
      .map(
        (a) => `<li>
      <div class="row">${verdictChip("pending")}<span class="action">${esc(a.action)}</span><span class="meta">→ ${esc(a.to)}</span></div>
      <pre>${esc(JSON.stringify(a.params, null, 2))}</pre>
      <div class="meta">${esc(a.session_id)} · ${esc(a.agent_id)} · ${new Date(a.ts).toLocaleTimeString()}</div>
      <div class="btns">
        <button class="approve" data-id="${esc(a.approval_id)}" data-d="approve">Approve</button>
        <button class="reject" data-id="${esc(a.approval_id)}" data-d="reject">Reject</button>
      </div>
    </li>`,
      )
      .join("") || '<li class="meta">Queue is empty.</li>';

  $("#approval-history").innerHTML =
    decided
      .map(
        (a) => `<li><div class="row">${verdictChip(a.status)}<span class="action">${esc(a.action)}</span>
        <span class="meta">by ${esc(a.approver || "?")} at ${a.decided_ts ? new Date(a.decided_ts).toLocaleTimeString() : ""}</span></div></li>`,
      )
      .join("") || '<li class="meta">Nothing decided yet.</li>';

  document.querySelectorAll("#approval-list button").forEach((btn) =>
    btn.addEventListener("click", async () => {
      await api("/v1/approvals/" + btn.dataset.id, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ decision: btn.dataset.d, approver: "console-user" }),
      });
      refreshApprovals();
      refreshDecisions();
    }),
  );
}

loadBundle();
refreshDecisions();
refreshApprovals();
setInterval(refreshDecisions, 1500);
setInterval(refreshApprovals, 1500);
