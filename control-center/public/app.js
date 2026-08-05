/*
 * Sandbox Control Center — cliente
 *
 * Sin dependencias ni bundler: el panel se sirve tal cual bajo una CSP que
 * solo permite 'self'. Todo el DOM se construye con textContent, nunca con
 * innerHTML, para que el contenido del catálogo no pueda inyectar marcado.
 *
 * El estado de los trabajos llega por SSE (/api/jobs/:id/events). El sondeo
 * periódico queda solo como red de seguridad por si el stream se corta.
 */

const WRITE_HEADERS = { "content-type": "application/json", "x-sandbox-request": "1" };
const TERMINAL = new Set(["completed", "failed", "blocked", "cancelled", "planned", "timeout"]);
const POLL_MS = 10_000;

/** Estados de trabajo mapeados a la semántica visual de los badges. */
const JOB_TONE = {
  completed: "healthy",
  planned: "healthy",
  running: "degraded",
  queued: "degraded",
  blocked: "stopped",
  failed: "stopped",
  timeout: "stopped",
  cancelled: "na"
};

/** Estados de runtime declarados en el catálogo. */
const RUNTIME_TONE = { ready: "healthy", experimental: "degraded", documented: "na", manual: "na", planned: "na" };

const byId = (id) => document.getElementById(id);
const setText = (node, value) => { node.textContent = String(value ?? ""); };

async function api(path, options = {}) {
  const response = await fetch(path, options);
  const value = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(value.error ?? `${response.status} ${response.statusText}`);
  return value;
}

// ── Carga inicial del catálogo ───────────────────────────────────────────────

const [system, catalog, policies, workloads] = await Promise.all([
  api("/api/system"),
  api("/api/catalog"),
  api("/api/policies"),
  api("/api/workloads")
]);

const runtimeById = new Map(catalog.runtimes.map((runtime) => [runtime.id, runtime]));
const policyById = new Map(policies.map((policy) => [policy.id, policy]));

setText(byId("hero-version"), `v${system.version}`);
setText(byId("foot-version"), `Sandbox Labs v${system.version}`);
setText(byId("bind-address"), `${system.host}:${system.port}`);
setText(byId("lab-count"), `${catalog.labs.length} laboratorios`);
// El badge se reconstruye entero para no depender del orden de los nodos.
{
  const badge = byId("safe-mode");
  const safe = system.safeMode === true && system.executionModel === "registered-workloads-only";
  badge.className = `status-badge ${safe ? "healthy" : "stopped"}`;
  const dot = document.createElement("span");
  dot.className = "status-dot";
  const label = document.createElement("span");
  setText(label, safe ? "Solo cargas registradas" : "Modo seguro desactivado");
  badge.replaceChildren(dot, label);
}

// ── Métricas ─────────────────────────────────────────────────────────────────

function renderMetrics() {
  const available = catalog.runtimes.filter((runtime) => runtime.status === "ready" || runtime.status === "experimental");
  const entries = [
    ["Laboratorios", catalog.labs.length],
    ["Runtimes", `${available.length}/${catalog.runtimes.length}`],
    ["Políticas", policies.length],
    ["Cargas", workloads.length],
    ["Estrictas", policies.filter((policy) => policy.enforcement.mode === "strict").length]
  ];
  const container = byId("metrics");
  container.replaceChildren();
  for (const [label, value] of entries) {
    const item = document.createElement("div");
    item.className = "metric";
    const labelNode = document.createElement("div");
    labelNode.className = "metric-label";
    setText(labelNode, label);
    const valueNode = document.createElement("div");
    valueNode.className = "metric-value";
    setText(valueNode, value);
    item.append(labelNode, valueNode);
    container.append(item);
  }
}

// ── Tarjetas de catálogo ─────────────────────────────────────────────────────

function card(container, { id, title, copy, meta, tone, state }) {
  const node = byId("card-template").content.cloneNode(true);
  setText(node.querySelector(".card-id"), id);
  setText(node.querySelector(".card-title"), title);
  setText(node.querySelector(".card-copy"), copy);
  setText(node.querySelector(".card-meta"), meta);
  setText(node.querySelector(".status-text"), state);
  node.querySelector(".status-badge").classList.add(tone);
  if (tone === "na") node.querySelector(".card").classList.add("na");
  container.append(node);
}

function renderCatalog() {
  const runtimes = byId("runtimes");
  runtimes.replaceChildren();
  for (const runtime of catalog.runtimes) {
    card(runtimes, {
      id: runtime.id,
      title: runtime.label,
      copy: runtime.controls.length ? `Aplica: ${runtime.controls.join(" · ")}` : "No declara controles efectivos.",
      meta: runtime.requires.length ? `Requiere: ${runtime.requires.join(", ")}` : "Sin requisitos externos",
      tone: RUNTIME_TONE[runtime.status] ?? "na",
      state: runtime.status
    });
  }

  const policyGrid = byId("policies");
  policyGrid.replaceChildren();
  for (const policy of policies) {
    card(policyGrid, {
      id: policy.id,
      title: policy.description || policy.id,
      copy: `Exige: ${policy.enforcement.requiredControls.join(" · ") || "nada"}`,
      meta: `red ${policy.network.mode} · ${policy.resources.memoryMb} MB · ${policy.resources.processes} procesos · ${policy.resources.timeoutSeconds} s`,
      tone: policy.enforcement.mode === "strict" ? "healthy" : "degraded",
      state: policy.enforcement.mode
    });
  }

  const labs = byId("labs");
  labs.replaceChildren();
  for (const lab of catalog.labs) {
    card(labs, {
      id: lab.id,
      title: lab.title,
      copy: `labs/${lab.id}-${lab.slug}`,
      meta: `nivel ${lab.level}`,
      tone: lab.status === "ready" ? "healthy" : "na",
      state: lab.status
    });
  }
}

// ── Formulario ───────────────────────────────────────────────────────────────

function fillSelect(select, values, label) {
  select.replaceChildren();
  for (const value of values) {
    const option = document.createElement("option");
    option.value = value.id;
    setText(option, label(value));
    select.append(option);
  }
}

/**
 * Anticipa el resultado del plan cruzando los controles que exige la política
 * con los que declara el runtime en el catálogo. Es una previsión: la decisión
 * real la toma sandboxctl al sondear el host.
 */
function updatePlanHint() {
  const policy = policyById.get(byId("policy").value);
  const runtime = runtimeById.get(byId("runtime").value);
  const hint = byId("plan-hint");
  if (!policy || !runtime) {
    setText(hint, "Selecciona una combinación para ver qué controles aplicará.");
    return;
  }
  const declared = new Set(runtime.controls);
  const required = policy.enforcement.requiredControls;
  const missing = required.filter((control) => !declared.has(control));
  const effective = required.filter((control) => declared.has(control));

  if (runtime.id === "dry-run") {
    setText(hint, "Previsión: dry-run planifica y deja evidencia, pero no ejecuta la carga.");
    return;
  }
  if (missing.length === 0) {
    setText(hint, `Previsión: ${runtime.id} declara los ${required.length} controles exigidos por ${policy.id}.`);
    return;
  }
  const verb = policy.enforcement.mode === "strict" ? "se bloqueará (fail-closed)" : "se ejecutará degradado";
  setText(hint, `Previsión: ${runtime.id} no aplica ${missing.join(", ")} — con política ${policy.enforcement.mode} ${verb}. Efectivos: ${effective.join(", ") || "ninguno"}.`);
}

function renderForm() {
  fillSelect(byId("workload"), workloads, (value) => `${value.id} · ${value.risk}`);
  fillSelect(byId("policy"), policies, (value) => `${value.id} · ${value.enforcement.mode}`);
  fillSelect(byId("runtime"), catalog.runtimes, (value) => `${value.id} · ${value.status}`);
  for (const id of ["policy", "runtime"]) byId(id).addEventListener("change", updatePlanHint);
  updatePlanHint();
}

byId("job-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const output = byId("form-result");
  const button = event.target.querySelector("button[type=submit]");
  button.disabled = true;
  output.className = "";
  setText(output, "Creando trabajo…");
  try {
    const args = byId("arguments").value.split(",").map((value) => value.trim()).filter(Boolean);
    const job = await api("/api/jobs", {
      method: "POST",
      headers: WRITE_HEADERS,
      body: JSON.stringify({
        workloadId: byId("workload").value,
        policyId: byId("policy").value,
        runtimeId: byId("runtime").value,
        arguments: args
      })
    });
    output.className = "ok";
    setText(output, `Trabajo ${job.id} creado.`);
    setText(byId("logs-output"), `[${job.id}] en cola…`);
    await refreshJobs();
  } catch (error) {
    output.className = "error";
    setText(output, `No se creó el trabajo: ${error.message}`);
  } finally {
    button.disabled = false;
  }
});

// ── Trabajos y flujo SSE ─────────────────────────────────────────────────────

/** Un EventSource por trabajo activo; se cierra al llegar a un estado terminal. */
const streams = new Map();

function watch(jobId) {
  if (streams.has(jobId)) return;
  const source = new EventSource(`/api/jobs/${jobId}/events`);
  streams.set(jobId, source);
  const onMessage = (event) => {
    let job;
    try {
      job = JSON.parse(event.data);
    } catch {
      return;
    }
    if (!job) return;
    applyJob(job);
    if (TERMINAL.has(job.status)) unwatch(jobId);
  };
  source.addEventListener("snapshot", onMessage);
  source.addEventListener("update", onMessage);
  source.addEventListener("error", () => unwatch(jobId));
}

function unwatch(jobId) {
  streams.get(jobId)?.close();
  streams.delete(jobId);
}

/** Índice de trabajos por id, para no repintar toda la lista en cada evento. */
const jobs = new Map();

function applyJob(job) {
  jobs.set(job.id, job);
  renderJobs();
  const lines = job.logs?.map((entry) => entry.line) ?? [];
  if (lines.length) setText(byId("logs-output"), `[${job.workloadId} · ${job.runtimeId}]\n${lines.join("\n")}`);
}

async function cancelJob(jobId) {
  try {
    await api(`/api/jobs/${jobId}/cancel`, { method: "POST", headers: WRITE_HEADERS, body: "{}" });
    await refreshJobs();
  } catch (error) {
    const output = byId("form-result");
    output.className = "error";
    setText(output, `No se pudo cancelar: ${error.message}`);
  }
}

async function showEvidence(evidenceId) {
  try {
    const evidence = await api(`/api/evidence/${evidenceId}`);
    setText(byId("evidence"), JSON.stringify(evidence, null, 2));
    byId("evidence-dialog").showModal();
  } catch (error) {
    const output = byId("form-result");
    output.className = "error";
    setText(output, `No se pudo abrir la evidencia: ${error.message}`);
  }
}

function jobRow(job) {
  const row = document.createElement("article");
  row.className = "job";

  const info = document.createElement("div");
  info.className = "job-info";
  const title = document.createElement("strong");
  setText(title, `${job.workloadId} · ${job.runtimeId}`);
  const meta = document.createElement("p");
  setText(meta, `política ${job.policyId} · ${new Date(job.createdAt).toLocaleString()}`);
  info.append(title, meta);

  const lastLine = job.error || job.logs?.at(-1)?.line;
  if (lastLine) {
    const log = document.createElement("code");
    log.className = "job-log";
    setText(log, lastLine);
    info.append(log);
  }

  const badge = document.createElement("span");
  badge.className = `status-badge ${JOB_TONE[job.status] ?? "na"}`;
  const dot = document.createElement("span");
  dot.className = "status-dot";
  const label = document.createElement("span");
  setText(label, job.status);
  badge.append(dot, label);

  const actions = document.createElement("div");
  actions.className = "job-actions";
  if (!TERMINAL.has(job.status)) {
    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "btn btn-danger";
    setText(cancel, "Cancelar");
    cancel.addEventListener("click", () => void cancelJob(job.id));
    actions.append(cancel);
  }
  if (job.evidenceId) {
    const evidence = document.createElement("button");
    evidence.type = "button";
    evidence.className = "btn btn-secondary";
    setText(evidence, "Ver evidencia");
    evidence.addEventListener("click", () => void showEvidence(job.evidenceId));
    actions.append(evidence);
  }

  row.append(info, badge, actions);
  return row;
}

function renderJobs() {
  const container = byId("jobs");
  const ordered = [...jobs.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  setText(byId("jobs-count"), `${ordered.length} ${ordered.length === 1 ? "trabajo" : "trabajos"}`);
  container.replaceChildren();
  if (ordered.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    setText(empty, "Todavía no hay trabajos. Crea uno con dry-run para ver el flujo completo sin ejecutar código.");
    container.append(empty);
    return;
  }
  for (const job of ordered) container.append(jobRow(job));
}

async function refreshJobs() {
  const list = await api("/api/jobs");
  jobs.clear();
  for (const job of list) {
    jobs.set(job.id, job);
    if (!TERMINAL.has(job.status)) watch(job.id);
  }
  renderJobs();
}

// ── Arranque ─────────────────────────────────────────────────────────────────

byId("close-dialog").addEventListener("click", () => byId("evidence-dialog").close());
byId("refresh-jobs").addEventListener("click", () => void refreshJobs());

renderMetrics();
renderCatalog();
renderForm();
await refreshJobs();

// Red de seguridad: si un stream se corta, el sondeo lento reconcilia el estado.
setInterval(() => void refreshJobs().catch(() => {}), POLL_MS);
