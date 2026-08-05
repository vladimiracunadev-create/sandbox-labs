// Verifica que todos los enlaces relativos de los .md apunten a archivos reales.
// Los enlaces externos (http, mailto, anclas) quedan fuera del alcance: este
// control existe para que la documentación no se rompa al mover archivos.
import { access, readdir, readFile } from "node:fs/promises";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const external = /^(https?:|mailto:|#|\/\/)/;
const ignoredDirectories = new Set(["target", "node_modules", ".git", ".sandbox-data", "artifacts", "site"]);
const linkPattern = /\[[^\]]+\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;
const documents = [];

async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (ignoredDirectories.has(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) await walk(path);
    else if (extname(entry.name) === ".md") documents.push(path);
  }
}

await walk(root);

let checked = 0;
const broken = [];

for (const file of documents) {
  const content = await readFile(file, "utf8");
  for (const match of content.matchAll(linkPattern)) {
    const link = match[1].split("#")[0];
    if (!link || external.test(link)) continue;
    const target = resolve(dirname(file), decodeURIComponent(link));
    try {
      await access(target);
      checked += 1;
    } catch {
      broken.push(`${relative(root, file)} → ${link}`);
    }
  }
}

if (broken.length > 0) {
  console.error(`❌ Enlaces rotos: ${broken.length}`);
  for (const entry of broken) console.error(`   - ${entry}`);
  process.exit(1);
}

console.log(`✅ Enlaces locales revisados: ${checked} en ${documents.length} documentos`);
