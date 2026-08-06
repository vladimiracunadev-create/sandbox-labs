#!/usr/bin/env node
// Comprueba que ningún enlace interno del sitio generado apunte a una página
// que no existe. Es el fallo más barato de cometer —renombrar un documento y
// olvidar quién lo enlazaba— y el más caro de descubrir en producción, porque
// un 404 no rompe el despliegue: se publica igual de bien que una página buena.

import { readdir, readFile, stat } from "node:fs/promises";
import { dirname, join, normalize, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const site = join(root, "site");

const HREF = /(?:href|src)="([^"]+)"/g;
const EXTERNAL = /^(?:https?:|mailto:|data:|#)/;

/** Todas las páginas HTML del sitio, en rutas absolutas. */
async function pages(directory) {
  const found = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const full = join(directory, entry.name);
    if (entry.isDirectory()) found.push(...(await pages(full)));
    else if (entry.name.endsWith(".html")) found.push(full);
  }
  return found;
}

async function exists(path) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

const html = (await pages(site)).sort();
const broken = [];
let checked = 0;

for (const file of html) {
  const source = await readFile(file, "utf8");
  for (const [, href] of source.matchAll(HREF)) {
    if (EXTERNAL.test(href)) continue;
    // El ancla se resuelve dentro de la propia página; aquí solo importa el archivo.
    const [path] = href.split("#");
    if (!path) continue;
    checked += 1;
    // Una ruta terminada en barra dependería de que el servidor sirva el
    // índice del directorio, y Pages no lo hace desde una subruta.
    if (path.endsWith("/")) {
      broken.push(`${relative(site, file)} → ${href} (termina en barra: apunta al index.html explícito)`);
      continue;
    }
    const target = normalize(join(dirname(file), path));
    if (!target.startsWith(site + sep)) {
      broken.push(`${relative(site, file)} → ${href} (sale del sitio)`);
    } else if (!(await exists(target))) {
      broken.push(`${relative(site, file)} → ${href}`);
    }
  }
}

if (broken.length > 0) {
  console.error(`❌ ${broken.length} enlaces internos rotos:`);
  for (const line of broken) console.error(`   ${line}`);
  process.exit(1);
}

console.log(`✅ Enlaces internos del sitio: ${checked} en ${html.length} páginas, ninguno roto`);
