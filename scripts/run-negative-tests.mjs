import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const catalog = JSON.parse(await readFile(resolve(root, "sandbox.config.json"), "utf8"));
const runtimes = new Map(catalog.runtimes.map((value) => [value.id, new Set(value.controls)]));
const names = ["filesystem-denied.json", "network-denied.json", "native-risk-rejected.json"];

for (const name of names) {
  const scenario = JSON.parse(await readFile(resolve(root, "tests", "scenarios", name), "utf8"));
  if (scenario.runtime && scenario.requiredControl && !runtimes.get(scenario.runtime)?.has(scenario.requiredControl)) {
    throw new Error(`${name}: el runtime no declara el control ${scenario.requiredControl}`);
  }
  if (scenario.expected !== "blocked") throw new Error(`${name}: un contrato negativo debe esperar blocked`);
}
console.log(`✅ Contratos de pruebas negativas coherentes: ${names.length}`);
