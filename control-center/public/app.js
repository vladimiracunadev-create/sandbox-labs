const api = async (path, options = {}) => {
  const response = await fetch(path, options);
  const value = await response.json();
  if (!response.ok) throw new Error(value.error ?? response.statusText);
  return value;
};

const byId = (id) => document.querySelector(`#${id}`);
const text = (node, value) => { node.textContent = String(value); };
const writeHeaders = { "content-type": "application/json", "x-sandbox-request": "1" };
const terminalStates = new Set(["completed", "failed", "blocked", "cancelled", "planned", "timeout"]);

const [system, catalog, policies, workloads] = await Promise.all([
  api("/api/system"), api("/api/catalog"), api("/api/policies"), api("/api/workloads")
]);

for (const [value, label] of [
  [catalog.labs.length, "laboratorios"],
  [catalog.runtimes.length, "runtimes"],
  [policies.length, "políticas"],
  [workloads.length, "cargas"]
]) {
  const item = document.createElement("div");
  item.className = "metric";
  const strong = document.createElement("strong");
  const span = document.createElement("span");
  text(strong, value);
  text(span, label);
  item.append(strong, span);
  byId("summary").append(item);
}
text(byId("lab-count"), `${system.name} · ${system.executionModel}`);

function card(container, code, title, badge, description) {
  const node = byId("card-template").content.cloneNode(true);
  text(node.querySelector("strong"), code);
  text(node.querySelector("h3"), title);
  text(node.querySelector(".badge"), badge);
  text(node.querySelector("p"), description);
  container.append(node);
}

for (const runtime of catalog.runtimes) {
  card(byId("runtimes"), runtime.id, runtime.label, runtime.status,
    `${runtime.controls.join(" · ")} · requiere ${runtime.requires.join(", ") || "nada"}`);
}
for (const lab of catalog.labs) {
  card(byId("labs"), lab.id, lab.title, lab.status, `${lab.slug} · nivel ${lab.level}`);
}
for (const policy of policies) {
  card(byId("policies"), policy.id, policy.description || policy.id, policy.enforcement.mode,
    `${policy.network.mode} · ${policy.resources.memoryMb} MB · ${policy.resources.timeoutSeconds} s`);
}

function fill(select, values, label) {
  for (const value of values) {
    const option = document.createElement("option");
    option.value = value.id;
    text(option, label(value));
    select.append(option);
  }
}
fill(byId("workload"), workloads, (value) => `${value.id} · ${value.risk}`);
fill(byId("policy"), policies, (value) => value.id);
fill(byId("runtime"), catalog.runtimes, (value) => `${value.id} · ${value.status}`);

byId("job-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const output = byId("form-result");
  text(output, "Creando…");
  try {
    const args = byId("arguments").value.split(",").map((value) => value.trim()).filter(Boolean);
    const job = await api("/api/jobs", {
      method: "POST",
      headers: writeHeaders,
      body: JSON.stringify({
        workloadId: byId("workload").value,
        policyId: byId("policy").value,
        runtimeId: byId("runtime").value,
        arguments: args
      })
    });
    text(output, `Trabajo ${job.id} creado`);
    await refreshJobs();
  } catch (error) {
    text(output, error.message);
  }
});

async function cancelJob(jobId) {
  await api(`/api/jobs/${jobId}/cancel`, { method: "POST", headers: writeHeaders, body: "{}" });
  await refreshJobs();
}

async function refreshJobs() {
  const jobs = await api("/api/jobs");
  const container = byId("jobs");
  container.replaceChildren();
  if (!jobs.length) {
    const empty = document.createElement("p");
    empty.className = "empty";
    text(empty, "Todavía no hay trabajos.");
    container.append(empty);
    return;
  }
  for (const job of jobs) {
    const row = document.createElement("article");
    row.className = "job";

    const info = document.createElement("div");
    info.className = "job-info";
    const title = document.createElement("strong");
    text(title, `${job.workloadId} · ${job.runtimeId}`);
    const meta = document.createElement("p");
    text(meta, `${job.policyId} · ${new Date(job.createdAt).toLocaleString()}`);
    info.append(title, meta);

    const lastLog = job.logs?.at(-1)?.line;
    if (lastLog || job.error) {
      const log = document.createElement("code");
      log.className = "job-log";
      text(log, job.error || lastLog);
      info.append(log);
    }

    const badge = document.createElement("span");
    badge.className = `badge status-${job.status}`;
    text(badge, job.status);

    const actions = document.createElement("div");
    actions.className = "job-actions";
    if (!terminalStates.has(job.status)) {
      const cancel = document.createElement("button");
      cancel.className = "danger";
      text(cancel, "Cancelar");
      cancel.addEventListener("click", () => void cancelJob(job.id));
      actions.append(cancel);
    }
    if (job.evidenceId) {
      const evidence = document.createElement("button");
      evidence.className = "secondary";
      text(evidence, "Ver evidencia");
      evidence.addEventListener("click", () => void showEvidence(job.evidenceId));
      actions.append(evidence);
    }

    row.append(info, badge, actions);
    container.append(row);
  }
}

async function showEvidence(id) {
  try {
    const evidence = await api(`/api/evidence/${id}`);
    text(byId("evidence"), JSON.stringify(evidence, null, 2));
    byId("evidence-dialog").showModal();
  } catch (error) {
    text(byId("form-result"), error.message);
  }
}

byId("close-dialog").addEventListener("click", () => byId("evidence-dialog").close());
byId("refresh-jobs").addEventListener("click", () => void refreshJobs());
await refreshJobs();
setInterval(() => void refreshJobs(), 3000);
