// Genera FILE_MANIFEST.txt: un SHA-256 por archivo versionado del repositorio.
// Sirve para verificar la integridad de un paquete de release descargado.
import { readdir, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ignoredDirectories = new Set([".git", "node_modules", "target", "artifacts", ".sandbox-data"]);
const rows = [];

async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (ignoredDirectories.has(entry.name) || entry.name === "FILE_MANIFEST.txt") continue;
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      await walk(path);
      continue;
    }
    if (!entry.isFile()) continue;
    const digest = createHash("sha256").update(await readFile(path)).digest("hex");
    rows.push(`${digest}  ${relative(root, path).replaceAll("\\", "/")}`);
  }
}

await walk(root);
rows.sort();
await writeFile(resolve(root, "FILE_MANIFEST.txt"), `${rows.join("\n")}\n`);
console.log(`✅ Manifest generado: ${rows.length} archivos`);
