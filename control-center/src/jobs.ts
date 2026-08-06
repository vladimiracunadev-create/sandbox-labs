import { createHash, randomUUID } from "node:crypto";
import { access, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { constants } from "node:fs";
import { delimiter, join, relative, resolve } from "node:path";
import { spawn } from "node:child_process";
import { validIdentifier, validateArguments } from "./security.ts";

const terminal = new Set(["completed", "failed", "blocked", "cancelled", "planned", "timeout"]);
const now = () => new Date().toISOString();
const makeId = () => `${Date.now().toString(16)}-${randomUUID().slice(0, 8)}`;

export class JobStore {
  constructor(paths, registry) {
    this.paths = paths;
    this.registry = registry;
    this.jobs = new Map();
    this.children = new Map();
    this.listeners = new Map();
  }

  async init() {
    const jobRoot = join(this.paths.dataRoot, "jobs");
    await mkdir(jobRoot, { recursive: true });
    await mkdir(this.paths.evidenceRoot, { recursive: true });
    for (const name of (await readdir(jobRoot)).filter((value) => /^[a-f0-9-]+\.json$/i.test(value))) {
      try {
        const job = JSON.parse(await readFile(join(jobRoot, name), "utf8"));
        if (["queued", "running"].includes(job.status)) {
          job.status = "failed";
          job.error = "interrupted_previous_session";
          job.updatedAt = now();
        }
        this.jobs.set(job.id, job);
      } catch {
        // Un archivo dañado no debe impedir levantar el panel.
      }
    }
  }

  list() {
    return [...this.jobs.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt)).map(publicJob);
  }

  get(jobId) {
    const job = this.jobs.get(jobId);
    return job ? publicJob(job) : null;
  }

  subscribe(jobId, response) {
    const set = this.listeners.get(jobId) ?? new Set();
    set.add(response);
    this.listeners.set(jobId, set);
    response.write(`event: snapshot\ndata: ${JSON.stringify(this.get(jobId))}\n\n`);
    const heartbeat = setInterval(() => response.write(": heartbeat\n\n"), 15_000);
    return () => {
      clearInterval(heartbeat);
      set.delete(response);
      if (set.size === 0) this.listeners.delete(jobId);
    };
  }

  async create(request) {
    validateJobRequest(request, this.registry);
    const job = {
      id: makeId(), createdAt: now(), updatedAt: now(), status: "queued",
      workloadId: request.workloadId, policyId: request.policyId, runtimeId: request.runtimeId,
      arguments: [...request.arguments], logs: [], evidenceId: null, error: null
    };
    this.jobs.set(job.id, job);
    await this.persist(job);
    queueMicrotask(() => void this.run(job));
    return publicJob(job);
  }

  async cancel(jobId) {
    const job = this.jobs.get(jobId);
    if (!job) return null;
    if (terminal.has(job.status)) return publicJob(job);
    const child = this.children.get(jobId);
    child?.kill("SIGTERM");
    if (child) {
      const forceKill = setTimeout(() => {
        if (this.children.get(jobId) === child) child.kill("SIGKILL");
      }, 2_000);
      forceKill.unref();
    }
    job.status = "cancelled";
    job.updatedAt = now();
    this.log(job, "cancel requested");
    await this.persist(job);
    this.emit(job);
    return publicJob(job);
  }

  async run(job) {
    job.status = "running";
    job.updatedAt = now();
    await this.persist(job);
    this.emit(job);
    try {
      const invocation = await cliInvocation(this.paths.repoRoot);
      if (!invocation) {
        await this.fallbackEvidence(job);
        return;
      }
      const workload = this.registry.workloadById.get(job.workloadId);
      const policy = this.registry.policyById.get(job.policyId);
      const workloadPath = relative(this.paths.repoRoot, workload.directory);
      const policyPath = relative(this.paths.repoRoot, policy.path);
      const args = [
        ...invocation.prefix, "--root", this.paths.repoRoot, "run",
        "--workload", workloadPath, "--runtime", job.runtimeId,
        "--policy", policyPath, "--json"
      ];
      for (const value of job.arguments) args.push("--arg", value);
      this.log(job, `starting ${job.workloadId} with ${job.runtimeId}`);
      const child = spawn(invocation.command, args, {
        cwd: this.paths.repoRoot,
        env: { ...process.env, SANDBOX_LABS_ALLOW_NATIVE: process.env.SANDBOX_LABS_ALLOW_NATIVE ?? "0" },
        stdio: ["ignore", "pipe", "pipe"]
      });
      this.children.set(job.id, child);
      const { code, stdout, stderr, timedOut } = await collectChild(
        child,
        policy.resources.outputBytes,
        (policy.resources.timeoutSeconds + 10) * 1000 + invocation.startupGraceMs,
        (line) => this.log(job, line)
      );
      this.children.delete(job.id);
      if (job.status === "cancelled") return;
      if (timedOut) {
        job.status = "timeout";
        job.error = "control_center_watchdog";
      } else if (code === 0) {
        try {
          const evidence = JSON.parse(stdout);
          job.evidenceId = evidence.runId;
          job.status = String(evidence.status ?? "completed").toLowerCase();
        } catch {
          job.status = job.runtimeId === "dry-run" ? "planned" : "completed";
        }
      } else {
        job.status = "failed";
        job.error = stderr.slice(0, 2000) || `sandboxctl_exit_${code}`;
      }
      job.updatedAt = now();
      await this.persist(job);
      this.emit(job);
    } catch (error) {
      this.children.delete(job.id);
      job.status = "failed";
      job.error = String(error?.message ?? error);
      job.updatedAt = now();
      this.log(job, job.error);
      await this.persist(job);
      this.emit(job);
    }
  }

  log(job, line) {
    const safe = String(line).replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, "").slice(0, 2000);
    if (!safe) return;
    job.logs.push({ at: now(), line: safe });
    if (job.logs.length > 500) job.logs.splice(0, job.logs.length - 500);
    job.updatedAt = now();
    this.emit(job);
  }

  async fallbackEvidence(job) {
    const workload = this.registry.workloadById.get(job.workloadId);
    const policy = this.registry.policyById.get(job.policyId);
    const isDryRun = job.runtimeId === "dry-run";
    const evidence = {
      schemaVersion: "1.0", runId: job.id, timestamp: now(), status: isDryRun ? "planned" : "blocked",
      runtime: { id: job.runtimeId, version: "control-center-fallback", available: false },
      host: { os: process.platform, architecture: process.arch },
      integrity: {
        policySha256: await sha256File(policy.path),
        workloadSha256: await sha256Directory(workload.directory),
        runnerSha256: "unavailable",
        runnerVersion: `control-center-${this.registry.catalog.project.version}`
      },
      policy: {
        id: policy.id, enforcement: policy.enforcement.mode, requestedControls: policy.enforcement.requiredControls,
        effectiveControls: [], unsupportedControls: policy.enforcement.requiredControls
      },
      workload: { id: workload.id, path: relative(this.paths.repoRoot, workload.directory), risk: workload.risk, expected: workload.expected.outcome },
      limits: { requested: policy.resources, effective: {} },
      result: {
        exitCode: null,
        reason: isDryRun ? "dry-run_without_compiled_cli" : "sandboxctl_unavailable",
        durationMs: 0, stdout: "", stderr: "", stdoutTruncated: false, stderrTruncated: false
      },
      violations: [],
      unsupported: policy.enforcement.requiredControls,
      plan: isDryRun
        ? ["Solicitud validada.", "Se generó evidencia sin ejecutar código."]
        : ["Solicitud validada.", "La ejecución se bloqueó porque sandboxctl no está compilado."]
    };
    await writeFile(join(this.paths.evidenceRoot, `${job.id}.json`), JSON.stringify(evidence, null, 2));
    job.evidenceId = job.id;
    job.status = evidence.status;
    job.updatedAt = now();
    this.log(job, isDryRun ? "dry-run evidence created" : "execution blocked: sandboxctl unavailable");
    await this.persist(job);
    this.emit(job);
  }

  async persist(job) {
    await writeFile(join(this.paths.dataRoot, "jobs", `${job.id}.json`), JSON.stringify(job, null, 2));
  }

  emit(job) {
    for (const response of this.listeners.get(job.id) ?? []) {
      response.write(`event: update\ndata: ${JSON.stringify(publicJob(job))}\n\n`);
    }
  }
}

function publicJob(job) {
  return {
    id: job.id, createdAt: job.createdAt, updatedAt: job.updatedAt, status: job.status,
    workloadId: job.workloadId, policyId: job.policyId, runtimeId: job.runtimeId,
    arguments: job.arguments, logs: job.logs, evidenceId: job.evidenceId, error: job.error
  };
}

export function validateJobRequest(request, registryOrWorkloads, policies, runtimeIds) {
  if (Array.isArray(registryOrWorkloads)) {
    const registry = {
      workloadById: new Map(registryOrWorkloads.map((value) => [value.id, value])),
      policyById: new Map(policies.map((value) => [value.id, value])),
      runtimeById: new Map([...runtimeIds].map((value) => [value, { id: value }]))
    };
    return validateJobRequest(request, registry);
  }
  const registry = registryOrWorkloads;
  if (!request || typeof request !== "object") throw new Error("invalid_job");
  if (!validIdentifier(request.workloadId) || !registry.workloadById.has(request.workloadId)) throw new Error("unknown_workload");
  if (!validIdentifier(request.policyId) || !registry.policyById.has(request.policyId)) throw new Error("unknown_policy");
  if (!validIdentifier(request.runtimeId) || !registry.runtimeById.has(request.runtimeId)) throw new Error("unknown_runtime");
  if (!validateArguments(request.arguments)) throw new Error("invalid_arguments");
  if (request.runtimeId === "native" && !registry.workloadById.get(request.workloadId).allowNative) throw new Error("native_not_allowed_for_workload");
}

// Margen adicional cuando el CLI se invoca vía `cargo run`: la primera
// compilación del workspace puede tardar minutos y no forma parte del tiempo
// de ejecución de la carga, así que no debe consumir el watchdog de la política.
const CARGO_STARTUP_GRACE_MS = 600_000;

export async function cliInvocation(root) {
  // Permite forzar la evidencia de reserva (pruebas y entornos sin toolchain).
  if (process.env.SANDBOX_LABS_CLI_FALLBACK === "off") return null;
  const explicit = process.env.SANDBOXCTL_BIN;
  if (explicit) {
    try { await access(explicit, constants.X_OK); return { command: explicit, prefix: [], startupGraceMs: 0 }; } catch { /* continue */ }
  }
  const suffix = process.platform === "win32" ? ".exe" : "";
  for (const path of [resolve(root, `target/release/sandboxctl${suffix}`), resolve(root, `target/debug/sandboxctl${suffix}`)]) {
    try { await access(path, constants.X_OK); return { command: path, prefix: [], startupGraceMs: 0 }; } catch { /* continue */ }
  }
  const cargo = await findOnPath(process.platform === "win32" ? "cargo.exe" : "cargo");
  if (cargo) return { command: cargo, prefix: ["run", "-q", "-p", "sandboxctl", "--"], startupGraceMs: CARGO_STARTUP_GRACE_MS };
  return null;
}

async function findOnPath(name) {
  for (const directory of (process.env.PATH ?? "").split(delimiter).filter(Boolean)) {
    const candidate = join(directory, name);
    try { await access(candidate, constants.X_OK); return candidate; } catch { /* continue */ }
  }
  return null;
}

async function collectChild(child, cap, timeoutMs, onLine) {
  let stdout = "";
  let stderr = "";
  let timedOut = false;
  const partial = { stdout: "", stderr: "" };
  const consume = (chunk, target) => {
    const text = chunk.toString("utf8");
    partial[target] += text;
    const lines = partial[target].split(/\r?\n/);
    partial[target] = lines.pop() ?? "";
    for (const line of lines.filter(Boolean)) onLine(`${target}: ${line}`);
    if (target === "stdout") stdout = (stdout + text).slice(0, cap);
    else stderr = (stderr + text).slice(0, cap);
  };
  child.stdout.on("data", (chunk) => consume(chunk, "stdout"));
  child.stderr.on("data", (chunk) => consume(chunk, "stderr"));
  const timer = setTimeout(() => { timedOut = true; child.kill("SIGKILL"); }, timeoutMs);
  const code = await new Promise((done, reject) => { child.once("error", reject); child.once("close", done); });
  clearTimeout(timer);
  if (partial.stdout) onLine(`stdout: ${partial.stdout}`);
  if (partial.stderr) onLine(`stderr: ${partial.stderr}`);
  return { code, stdout, stderr, timedOut };
}

async function sha256File(path) {
  return createHash("sha256").update(await readFile(path)).digest("hex");
}

async function sha256Directory(root) {
  const digest = createHash("sha256");
  async function walk(dir) {
    const entries = await readdir(dir, { withFileTypes: true });
    entries.sort((a, b) => a.name.localeCompare(b.name));
    for (const entry of entries) {
      const path = join(dir, entry.name);
      digest.update(relative(root, path));
      digest.update("\0");
      if (entry.isDirectory()) await walk(path);
      else if (entry.isFile()) digest.update(await readFile(path));
      digest.update("\0");
    }
  }
  await walk(root);
  return digest.digest("hex");
}

export async function listEvidence(root) {
  await mkdir(root, { recursive: true });
  const names = (await readdir(root)).filter((name) => /^[a-f0-9-]+\.json$/i.test(name)).sort().reverse();
  return Promise.all(names.slice(0, 100).map(async (name) => {
    try {
      const value = JSON.parse(await readFile(join(root, name), "utf8"));
      return { runId: value.runId, status: String(value.status).toLowerCase(), runtime: value.runtime?.id, workload: value.workload?.id, timestamp: value.timestamp };
    } catch {
      return { runId: name.replace(/\.json$/, ""), status: "invalid" };
    }
  }));
}
