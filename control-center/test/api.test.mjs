import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { request } from "node:http";
import { createSandboxServer } from "../dist/server.js";
import { validateSchema } from "../../scripts/lib/json-schema-validator.mjs";

// El repositorio se deriva de la ubicación del test, no de process.cwd(): la
// suite corre desde control-center/ (pnpm --dir) y desde la raíz (make).
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

// La evidencia se genera sin invocar cargo: compilar el CLI dentro de una
// prueba de API la vuelve lenta y dependiente del entorno. La ruta real de
// sandboxctl la cubre el job de Rust en CI.
process.env.SANDBOX_LABS_CLI_FALLBACK = "off";

async function startServer(t) {
  const temp = await mkdtemp(join(tmpdir(), "sandbox-control-center-"));
  const app = await createSandboxServer({
    repoRoot,
    dataRoot: join(temp, "data"),
    evidenceRoot: join(temp, "evidence"),
    port: 0
  });
  await new Promise((done) => app.server.listen(0, "127.0.0.1", done));
  t.after(async () => {
    await new Promise((done) => app.server.close(done));
    await rm(temp, { recursive: true, force: true });
  });
  const { port } = app.server.address();
  return { app, temp, port, base: `http://127.0.0.1:${port}` };
}

// fetch() prohíbe fijar el header Host, así que el chequeo anti DNS-rebinding
// necesita un cliente HTTP de bajo nivel.
function rawGet(port, path, host) {
  return new Promise((done, fail) => {
    const call = request({ host: "127.0.0.1", port, path, method: "GET", headers: { host } }, (response) => {
      response.resume();
      done(response.statusCode);
    });
    call.on("error", fail);
    call.end();
  });
}

test("API restringe escrituras y crea job registrado", async (t) => {
  const { temp, base } = await startServer(t);
  const body = JSON.stringify({ workloadId: "hello", policyId: "minimal", runtimeId: "dry-run", arguments: [] });

  const rejected = await fetch(`${base}/api/jobs`, { method: "POST", headers: { "content-type": "application/json" }, body });
  assert.equal(rejected.status, 403, "sin cabecera x-sandbox-request debe rechazar");

  const created = await fetch(`${base}/api/jobs`, { method: "POST", headers: { "content-type": "application/json", "x-sandbox-request": "1" }, body });
  assert.equal(created.status, 202);
  const job = await created.json();

  let current;
  for (let attempt = 0; attempt < 200; attempt += 1) {
    await new Promise((done) => setTimeout(done, 25));
    current = await fetch(`${base}/api/jobs/${job.id}`).then((response) => response.json());
    if (current.status === "planned") break;
    assert.notEqual(current.status, "failed", `el job falló: ${current.error}`);
  }
  assert.equal(current.status, "planned");
  assert.equal(current.workloadId, "hello");

  const evidence = JSON.parse(await readFile(join(temp, "evidence", `${current.evidenceId}.json`), "utf8"));
  const schema = JSON.parse(await readFile(join(repoRoot, "schemas", "evidence.schema.json"), "utf8"));
  assert.deepEqual(validateSchema(schema, evidence), []);
});

test("publica catálogo y metadatos sin exponer rutas del host", async (t) => {
  const { base } = await startServer(t);

  const system = await fetch(`${base}/api/system`).then((response) => response.json());
  assert.equal(system.safeMode, true);
  assert.equal(system.executionModel, "registered-workloads-only");

  const catalog = await fetch(`${base}/api/catalog`).then((response) => response.json());
  assert.ok(Array.isArray(catalog.cases) && catalog.cases.length > 0);
  // Cada caso debe declarar la idea que enseña: sin eso es un tema, no un caso.
  assert.ok(catalog.cases.every((value) => typeof value.idea === "string" && value.idea.length > 20));

  const policies = await fetch(`${base}/api/policies`).then((response) => response.json());
  assert.ok(policies.length > 0);
  assert.ok(policies.every((policy) => policy.path === undefined && policy.file === undefined));

  const workloads = await fetch(`${base}/api/workloads`).then((response) => response.json());
  assert.ok(workloads.length > 0);
  assert.ok(workloads.every((workload) => workload.directory === undefined && workload.manifestPath === undefined));

  // No existe endpoint de comandos arbitrarios: cualquier ruta inventada es 404.
  const missing = await fetch(`${base}/api/exec`, { method: "POST", headers: { "x-sandbox-request": "1" }, body: "{}" });
  assert.equal(missing.status, 404);
});

test("rechaza jobs con referencias no registradas", async (t) => {
  const { base } = await startServer(t);
  const headers = { "content-type": "application/json", "x-sandbox-request": "1" };

  const invalid = [
    { workloadId: "no-existe", policyId: "minimal", runtimeId: "dry-run", arguments: [] },
    { workloadId: "hello", policyId: "no-existe", runtimeId: "dry-run", arguments: [] },
    { workloadId: "hello", policyId: "minimal", runtimeId: "no-existe", arguments: [] },
    { workloadId: "../../etc/passwd", policyId: "minimal", runtimeId: "dry-run", arguments: [] },
    { workloadId: "hello", policyId: "minimal", runtimeId: "dry-run", arguments: "no-es-array" },
    { workloadId: "hello", policyId: "minimal", runtimeId: "dry-run", arguments: [42] }
  ];
  for (const body of invalid) {
    const response = await fetch(`${base}/api/jobs`, { method: "POST", headers, body: JSON.stringify(body) });
    assert.equal(response.status, 400, JSON.stringify(body));
  }
});

test("bloquea Host no confiable (DNS rebinding)", async (t) => {
  const { port } = await startServer(t);
  assert.equal(await rawGet(port, "/api/system", `127.0.0.1:${port}`), 200);
  assert.equal(await rawGet(port, "/api/system", "evil.example"), 421);
});

test("sirve la interfaz con cabeceras de seguridad", async (t) => {
  const { base } = await startServer(t);
  const response = await fetch(`${base}/`);
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type"), /text\/html/);
  assert.equal(response.headers.get("x-content-type-options"), "nosniff");
  assert.match(response.headers.get("content-security-policy"), /default-src 'self'/);
  assert.equal(response.headers.get("referrer-policy"), "no-referrer");
});

test("rechaza traversals que sobreviven a la normalización de URL", async (t) => {
  const { port } = await startServer(t);
  // Estas formas no las colapsa el parser de URL, así que llegan a decodePath:
  // son las que realmente ejercitan la defensa del servidor.
  for (const path of ["/%252e%252e/package.json", "/..%2fpackage.json", "/a%5Cb", "/%00bad"]) {
    assert.equal(await rawGet(port, path, `127.0.0.1:${port}`), 400, path);
  }
  // Un archivo inexistente dentro del root sigue siendo 404, no 400.
  assert.equal(await rawGet(port, "/no-existe.css", `127.0.0.1:${port}`), 404);
});
