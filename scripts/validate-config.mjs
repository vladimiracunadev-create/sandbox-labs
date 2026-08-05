import { access, readFile, readdir, stat } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { validateSchema } from "./lib/json-schema-validator.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const load = async (path) => JSON.parse(await readFile(path, "utf8"));
const assert = (condition, message) => { if (!condition) throw new Error(message); };

async function validate(schemaName, file) {
  const schema = await load(resolve(root, "schemas", schemaName));
  const value = await load(file);
  const errors = validateSchema(schema, value);
  if (errors.length) throw new Error(`${relative(root, file)}:\n- ${errors.join("\n- ")}`);
  return value;
}

async function ensureSchemaReference(file, value) {
  if (!value.$schema || value.$schema.startsWith("http")) return;
  await access(resolve(dirname(file), value.$schema));
}

const catalogPath = resolve(root, "sandbox.config.json");
const catalog = await validate("catalog.schema.json", catalogPath);
await ensureSchemaReference(catalogPath, catalog);

const versions = [
  ["package.json", (await load(resolve(root, "package.json"))).version],
  ["control-center/package.json", (await load(resolve(root, "control-center", "package.json"))).version],
  ["sandbox.config.json", catalog.project.version],
  ["Cargo.toml", (await readFile(resolve(root, "Cargo.toml"), "utf8")).match(/^version\s*=\s*"([^"]+)"/m)?.[1]]
];
assert(versions.every(([, value]) => value === catalog.project.version), `Versiones inconsistentes: ${versions.map(([file, value]) => `${file}=${value}`).join(", ")}`);

const knownControls = new Set(["planning", "evidence", "filesystem", "network", "processes", "memory", "cpu", "timeout", "capabilities", "syscalls", "devices", "environment", "output"]);
const runtimeIds = new Set();
for (const runtime of catalog.runtimes) {
  assert(!runtimeIds.has(runtime.id), `Runtime duplicado: ${runtime.id}`);
  runtimeIds.add(runtime.id);
  for (const control of runtime.controls) assert(knownControls.has(control), `Control desconocido ${control} en runtime ${runtime.id}`);
}
assert(runtimeIds.has(catalog.project.defaultRuntime), "defaultRuntime no registrado");
assert(catalog.runtimes.find((value) => value.id === catalog.project.defaultRuntime)?.status === "ready", "defaultRuntime debe estar ready");

const labIds = new Set();
const registeredLabDirectories = new Set();
for (const lab of catalog.labs) {
  assert(!labIds.has(lab.id), `ID de laboratorio duplicado: ${lab.id}`);
  labIds.add(lab.id);
  const name = `${lab.id}-${lab.slug}`;
  const directory = resolve(root, "labs", name);
  assert((await stat(directory)).isDirectory(), `Falta laboratorio: labs/${name}`);
  await access(resolve(directory, "README.md"));
  registeredLabDirectories.add(name);
}
const diskLabDirectories = (await readdir(resolve(root, "labs"), { withFileTypes: true })).filter((entry) => entry.isDirectory()).map((entry) => entry.name);
for (const name of diskLabDirectories) assert(registeredLabDirectories.has(name), `Laboratorio no registrado en catálogo: labs/${name}`);

const policiesDirectory = resolve(root, catalog.policiesDirectory);
const policyIds = new Set();
for (const name of (await readdir(policiesDirectory)).filter((value) => value.endsWith(".json"))) {
  const file = resolve(policiesDirectory, name);
  const policy = await validate("policy.schema.json", file);
  await ensureSchemaReference(file, policy);
  assert(!policyIds.has(policy.id), `Política duplicada: ${policy.id}`);
  assert(name === `${policy.id}.json`, `La política ${policy.id} debe llamarse ${policy.id}.json`);
  policyIds.add(policy.id);
}
assert(policyIds.size > 0, "No hay políticas");

const manifestPaths = [];
async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) await walk(path);
    else if (entry.name === "manifest.json") manifestPaths.push(path);
  }
}
await walk(resolve(root, catalog.workloadsDirectory));
const workloadIds = new Set();
for (const file of manifestPaths) {
  const workload = await validate("workload.schema.json", file);
  await ensureSchemaReference(file, workload);
  assert(!workloadIds.has(workload.id), `Carga duplicada: ${workload.id}`);
  assert(!workload.command.includes("/") && !workload.command.includes("\\"), `${workload.id}: command debe ser un ejecutable, no una ruta`);
  if (workload.kind !== "wasi") await access(resolve(dirname(file), workload.entrypoint));
  workloadIds.add(workload.id);
}
assert(workloadIds.size > 0, "No hay workloads");

// Suite de contención: cada sonda debe apuntar a una carga registrada y a una
// dimensión declarada. Una sonda huérfana no mediría nada y pasaría por buena.
const suitePath = resolve(root, "escape-suite", "suite.json");
const suite = await validate("escape-suite.schema.json", suitePath);
await ensureSchemaReference(suitePath, suite);
const dimensionIds = new Set(suite.dimensions.map((value) => value.id));
const probeIds = new Set();
for (const probe of suite.probes) {
  assert(!probeIds.has(probe.id), `Sonda duplicada: ${probe.id}`);
  probeIds.add(probe.id);
  assert(dimensionIds.has(probe.dimension), `${probe.id}: dimensión desconocida ${probe.dimension}`);
  assert(workloadIds.has(probe.workload), `${probe.id}: apunta a una carga no registrada (${probe.workload})`);
  assert(knownControls.has(probe.control), `${probe.id}: control desconocido ${probe.control}`);
}
for (const dimension of dimensionIds) {
  assert(suite.probes.some((probe) => probe.dimension === dimension), `La dimensión ${dimension} no tiene ninguna sonda`);
}

console.log(`✅ Configuración válida: ${catalog.labs.length} labs, ${catalog.runtimes.length} runtimes, ${policyIds.size} policies, ${workloadIds.size} workloads, ${probeIds.size} sondas de contención · v${catalog.project.version}`);
