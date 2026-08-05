import { rm, readdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
await rm(resolve(root, ".sandbox-data"), { recursive: true, force: true });
const evidenceRoot = resolve(root, "evidence", "runs");
for (const name of await readdir(evidenceRoot)) {
  if (name.endsWith(".json")) await rm(resolve(evidenceRoot, name), { force: true });
}
console.log("✅ Estado temporal de pruebas eliminado");
