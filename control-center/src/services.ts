// Control de sandboxes de larga duración desde el panel.
//
// El panel no levanta el sandbox por su cuenta: delega en `sandboxctl service`,
// que es quien sabe elegir runtime, compilar la política y registrar el PID. Si
// el panel montara sandboxes por su lado habría dos implementaciones del mismo
// contrato y una de las dos se quedaría atrás.

import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { spawn } from "node:child_process";
import { cliInvocation } from "./jobs.ts";

/** Acciones permitidas. Lista cerrada: el panel nunca compone un comando libre. */
const ACTIONS = new Set(["up", "down"]);

/** Levantar puede tardar: hay que compilar el CLI la primera vez y arrancar el sandbox. */
const ACTION_TIMEOUT_MS = 180_000;

export async function loadServices(paths) {
  const directory = join(paths.repoRoot, "services");
  const found = [];
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    return found;
  }
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    try {
      const manifest = JSON.parse(await readFile(join(directory, entry.name, "service.json"), "utf8"));
      found.push({ ...manifest, directory: entry.name });
    } catch {
      // Un manifiesto roto no debe impedir listar los demás.
    }
  }
  return found.sort((a, b) => a.directory.localeCompare(b.directory));
}

/** Ejecuta `sandboxctl service <args>` y devuelve stdout, stderr y código. */
async function runCli(paths, args, timeoutMs = 30_000) {
  const invocation = await cliInvocation(paths.repoRoot);
  if (!invocation) {
    throw new Error("sandboxctl_unavailable");
  }
  const child = spawn(invocation.command, [...invocation.prefix, "--root", paths.repoRoot, "service", ...args], {
    cwd: paths.repoRoot,
    env: { ...process.env, NO_COLOR: "1" },
    stdio: ["ignore", "pipe", "pipe"]
  });

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => { stdout += chunk.toString("utf8"); });
  child.stderr.on("data", (chunk) => { stderr += chunk.toString("utf8"); });

  const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs + invocation.startupGraceMs);
  const code = await new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("close", resolve);
  });
  clearTimeout(timer);
  return { code, stdout, stderr };
}

/**
 * Estado de todos los servicios.
 *
 * Si el CLI no está compilado se devuelve el catálogo con estado `unknown` en
 * lugar de fallar: el panel debe poder mostrar qué existe aunque todavía no se
 * pueda operar, y decir por qué.
 */
export async function listServices(paths) {
  const catalog = await loadServices(paths);
  try {
    const { stdout } = await runCli(paths, ["list", "--json"]);
    const live = JSON.parse(stdout);
    const byId = new Map(live.map((value) => [value.id, value]));
    return {
      available: true,
      services: catalog.map((value) => ({ ...value, ...(byId.get(value.id) ?? {}), state: byId.get(value.id)?.state ?? "stopped" }))
    };
  } catch (error) {
    return {
      available: false,
      reason: String(error?.message ?? error),
      services: catalog.map((value) => ({ ...value, state: "unknown", url: `http://127.0.0.1:${value.port}` }))
    };
  }
}

export async function actOnService(paths, id, action) {
  if (!ACTIONS.has(action)) throw new Error("unknown_action");
  const catalog = await loadServices(paths);
  if (!catalog.some((value) => value.id === id)) throw new Error("unknown_service");

  const args = action === "up" ? ["up", id] : ["down", id];
  const { code, stdout, stderr } = await runCli(paths, args, ACTION_TIMEOUT_MS);
  return {
    id,
    action,
    ok: code === 0,
    // La salida del CLI se devuelve tal cual: es donde vive el motivo real de
    // un fallo (runtime ausente, puerto ocupado, sandbox que murió al arrancar).
    output: `${stdout}${stderr}`.trim().slice(0, 4000)
  };
}

export async function serviceLogs(paths, id, lines = 60) {
  const catalog = await loadServices(paths);
  if (!catalog.some((value) => value.id === id)) throw new Error("unknown_service");
  const { stdout, stderr } = await runCli(paths, ["logs", id, "--lines", String(lines)]);
  return { id, logs: `${stdout}${stderr}`.slice(-16_000) };
}
