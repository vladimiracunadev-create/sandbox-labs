import { readFile, readdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { validateSchema } from "./lib/json-schema-validator.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const evidenceRoot = resolve(root, "evidence", "runs");
const schema = JSON.parse(await readFile(resolve(root, "schemas", "evidence.schema.json"), "utf8"));
const files = (await readdir(evidenceRoot)).filter((name) => name.endsWith(".json"));

for (const name of files) {
  const path = resolve(evidenceRoot, name);
  const value = JSON.parse(await readFile(path, "utf8"));
  const errors = validateSchema(schema, value);
  if (errors.length) throw new Error(`${path}:\n- ${errors.join("\n- ")}`);
}
console.log(`✅ Evidencias válidas: ${files.length}`);
