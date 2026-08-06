// Genera el sitio de GitHub Pages a partir del propio repositorio.
//
// El sitio no se escribe a mano: se construye desde el catálogo, los servicios
// y los README de los laboratorios. Así la web no puede contradecir al repo —
// que es el fallo más común de un sitio de proyecto mantenido aparte.
//
// Sin dependencias: el renderizador de Markdown de abajo cubre lo que usan los
// documentos del repo (encabezados, listas, tablas, bloques de código, citas,
// enlaces, énfasis). Añadir marked para esto traería 40 paquetes a un proyecto
// que no tiene ninguno.

import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "site");

const escape = (value) =>
  String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

// ── Markdown mínimo ──────────────────────────────────────────────────────────

function inline(text) {
  return escape(text)
    .replace(/`([^`]+)`/g, (_, code) => `<code>${code}</code>`)
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[\s(])\*([^*\n]+)\*/g, "$1<em>$2</em>")
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, label, href) => {
      // Los enlaces relativos a otros .md apuntan al HTML generado; el resto
      // se manda a GitHub, que es donde vive el archivo.
      let target = href;
      if (/^labs\/(\d\d)-([a-z0-9-]+)\/?$/.test(href)) target = `labs/${href.split("/")[1]}.html`;
      else if (/^\.\.\/\.\.\/(.+)$/.test(href) || href.endsWith(".md") || href.startsWith("../")) {
        target = `https://github.com/vladimiracunadev-create/sandbox-labs/blob/main/${href.replace(/^(\.\.\/)+/, "")}`;
      }
      const external = /^https?:/.test(target);
      return `<a href="${escape(target)}"${external ? ' target="_blank" rel="noreferrer noopener"' : ""}>${label}</a>`;
    });
}

function renderMarkdown(markdown) {
  const lines = markdown.split(/\r?\n/);
  const html = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];

    if (line.startsWith("```")) {
      const language = line.slice(3).trim();
      const body = [];
      index += 1;
      while (index < lines.length && !lines[index].startsWith("```")) body.push(lines[index++]);
      index += 1;
      if (language === "mermaid") {
        html.push(`<pre class="mermaid">${escape(body.join("\n"))}</pre>`);
      } else {
        html.push(`<pre><code>${escape(body.join("\n"))}</code></pre>`);
      }
      continue;
    }

    if (line.startsWith("|") && lines[index + 1]?.match(/^\|[\s:|-]+\|$/)) {
      const head = line.split("|").slice(1, -1).map((cell) => cell.trim());
      index += 2;
      const rows = [];
      while (index < lines.length && lines[index].startsWith("|")) {
        rows.push(lines[index++].split("|").slice(1, -1).map((cell) => cell.trim()));
      }
      html.push(
        `<div class="table-wrap"><table><thead><tr>${head.map((c) => `<th>${inline(c)}</th>`).join("")}</tr></thead>` +
          `<tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${inline(c)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`
      );
      continue;
    }

    const heading = line.match(/^(#{1,4})\s+(.*)$/);
    if (heading) {
      const level = heading[1].length;
      html.push(`<h${level}>${inline(heading[2])}</h${level}>`);
      index += 1;
      continue;
    }

    if (/^>\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]/.test(line)) {
      const kind = line.match(/\[!(\w+)\]/)[1].toLowerCase();
      const body = [];
      index += 1;
      while (index < lines.length && lines[index].startsWith(">")) body.push(lines[index++].replace(/^>\s?/, ""));
      html.push(`<div class="callout ${kind}">${inline(body.join(" ").trim())}</div>`);
      continue;
    }

    if (line.startsWith(">")) {
      const body = [];
      while (index < lines.length && lines[index].startsWith(">")) body.push(lines[index++].replace(/^>\s?/, ""));
      html.push(`<blockquote>${inline(body.join(" ").trim())}</blockquote>`);
      continue;
    }

    if (/^[-*]\s+/.test(line)) {
      const items = [];
      while (index < lines.length && /^[-*]\s+/.test(lines[index])) {
        items.push(`<li>${inline(lines[index++].replace(/^[-*]\s+/, ""))}</li>`);
      }
      html.push(`<ul>${items.join("")}</ul>`);
      continue;
    }

    if (/^\d+\.\s+/.test(line)) {
      const items = [];
      while (index < lines.length && /^\d+\.\s+/.test(lines[index])) {
        items.push(`<li>${inline(lines[index++].replace(/^\d+\.\s+/, ""))}</li>`);
      }
      html.push(`<ol>${items.join("")}</ol>`);
      continue;
    }

    if (line.trim() === "---") {
      html.push("<hr>");
      index += 1;
      continue;
    }

    if (line.trim() === "") {
      index += 1;
      continue;
    }

    const paragraph = [];
    while (index < lines.length && lines[index].trim() !== "" && !/^([#>|`-]|\d+\.)/.test(lines[index])) {
      paragraph.push(lines[index++]);
    }
    if (paragraph.length) html.push(`<p>${inline(paragraph.join(" "))}</p>`);
    else index += 1;
  }
  return html.join("\n");
}

// ── Plantilla ────────────────────────────────────────────────────────────────

const STYLE = await readFile(join(root, "site", "_style.css"), "utf8").catch(() => "");

function page({ title, description, body, active = "", depth = 0 }) {
  const base = depth ? "../" : "";
  const nav = [
    ["", "Inicio"],
    ["labs/", "Laboratorios"],
    ["conceptos.html", "Qué es un sandbox"],
  ]
    .map(([href, label]) => {
      const target = href === "" ? `${base}index.html` : `${base}${href}`;
      const cls = active === href ? ' class="on"' : "";
      return `<a${cls} href="${target}">${label}</a>`;
    })
    .join("");

  return `<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="color-scheme" content="light dark">
<meta name="description" content="${escape(description)}">
<title>${escape(title)}</title>
<style>${STYLE}</style>
</head>
<body>
<header class="topbar"><div class="topbar-in">
  <a class="brand" href="${base}index.html">🛡️ sandbox-labs</a>
  <nav>${nav}</nav>
  <a class="gh" href="https://github.com/vladimiracunadev-create/sandbox-labs" target="_blank" rel="noreferrer noopener">GitHub ↗</a>
</div></header>
<main class="shell">
${body}
</main>
<footer class="foot"><div class="shell">
  sandbox-labs · Apache-2.0 ·
  <a href="https://github.com/vladimiracunadev-create/sandbox-labs">código</a> ·
  <a href="https://github.com/vladimiracunadev-create/sandbox-labs/blob/main/SECURITY.md">seguridad</a>
</div></footer>
<script type="module">
  // startOnLoad solo actúa si mermaid se carga antes de que el DOM esté listo.
  // Con un import dinámico posterior no hace nada: hay que llamar a run() a
  // mano. Ese era el motivo por el que los diagramas no aparecían.
  const blocks = document.querySelectorAll("pre.mermaid");
  if (blocks.length) {
    try {
      const { default: mermaid } = await import("https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs");
      const dark = matchMedia("(prefers-color-scheme: dark)").matches;
      mermaid.initialize({ startOnLoad: false, theme: dark ? "dark" : "default", securityLevel: "strict" });
      await mermaid.run({ nodes: blocks });
    } catch (error) {
      // Sin red, con el CDN caído o con un bloqueador de por medio, el
      // diagrama se queda como código legible en vez de desaparecer sin más.
      for (const block of blocks) {
        block.classList.add("mermaid-fallback");
        block.dataset.note = "diagrama sin renderizar · " + error.message;
      }
    }
  }
</script>
</body>
</html>
`;
}

// ── Contenido ────────────────────────────────────────────────────────────────

const catalog = JSON.parse(await readFile(join(root, "sandbox.config.json"), "utf8"));

const services = [];
for (const entry of await readdir(join(root, "services"), { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  services.push(JSON.parse(await readFile(join(root, "services", entry.name, "service.json"), "utf8")));
}
services.sort((a, b) => a.port - b.port);

const labs = [];
for (const lab of catalog.labs) {
  const slug = `${lab.id}-${lab.slug}`;
  const markdown = await readFile(join(root, "labs", slug, "README.md"), "utf8");
  labs.push({ ...lab, slug, markdown });
}

await rm(out, { recursive: true, force: true });
await mkdir(join(out, "labs"), { recursive: true });
await writeFile(join(out, "_style.css"), STYLE);

// Página de cada laboratorio
for (const [index, lab] of labs.entries()) {
  const previous = labs[index - 1];
  const next = labs[index + 1];
  const pager =
    `<div class="pager">` +
    (previous ? `<a href="${previous.slug}.html">← ${escape(previous.title)}</a>` : `<span></span>`) +
    (next ? `<a href="${next.slug}.html">${escape(next.title)} →</a>` : `<span></span>`) +
    `</div>`;
  await writeFile(
    join(out, "labs", `${lab.slug}.html`),
    page({
      title: `Lab ${lab.id} · ${lab.title} — sandbox-labs`,
      description: lab.title,
      active: "labs/",
      depth: 1,
      body: `<article class="doc">${renderMarkdown(lab.markdown)}${pager}</article>`,
    })
  );
}

// Índice de laboratorios
const byLevel = new Map();
for (const lab of labs) {
  if (!byLevel.has(lab.level)) byLevel.set(lab.level, []);
  byLevel.get(lab.level).push(lab);
}
const LEVEL_LABEL = { initial: "Fundamentos", core: "Controles del kernel", advanced: "Fronteras fuertes", platform: "Plataforma" };
const labsIndex = [...byLevel.entries()]
  .map(
    ([level, group]) => `<h2>${escape(LEVEL_LABEL[level] ?? level)}</h2>
<div class="grid">${group
      .map(
        (lab) => `<a class="card lab" href="${lab.slug}.html">
  <div class="card-top"><span class="num">${escape(lab.id)}</span><span class="chip ${lab.status}">${escape(lab.status)}</span></div>
  <h3>${escape(lab.title)}</h3>
  <p>${escape(lab.markdown.split("\n").find((line) => line && !line.startsWith("#") && !line.startsWith(">")) ?? "")}</p>
</a>`
      )
      .join("")}</div>`
  )
  .join("");

await writeFile(
  join(out, "labs", "index.html"),
  page({
    title: "Laboratorios — sandbox-labs",
    description: "18 laboratorios de aislamiento, del baseline sin restricciones a la plataforma multi-tenant.",
    active: "labs/",
    depth: 1,
    body: `<section class="hero small">
  <span class="eyebrow">${labs.length} laboratorios</span>
  <h1>Del baseline sin restricciones a la plataforma multi-tenant</h1>
  <p>Cada laboratorio aísla una dimensión y la mide. Empieza por el 01: sin ver qué alcanza
  una carga cuando nadie la contiene, los controles siguientes no significan nada.</p>
</section>
${labsIndex}`,
  })
);

// Conceptos
await writeFile(
  join(out, "conceptos.html"),
  page({
    title: "Qué es un sandbox y por qué importa — sandbox-labs",
    description: "Qué es un sandbox, qué problema resuelve, qué fronteras existen y cuál elegir.",
    active: "conceptos.html",
    body: await readFile(join(root, "site-src", "conceptos.html"), "utf8"),
  })
);

// Portada
const serviceCards = services
  .map(
    (service) => `<div class="card svc">
  <div class="card-top"><span class="num">:${service.port}</span><span class="chip ${escape(service.category)}">${escape(service.category)}</span></div>
  <h3>${escape(service.name)}</h3>
  <p>${escape(service.description)}</p>
  <p class="teaches"><b>Enseña:</b> ${escape(service.teaches)}</p>
  <code class="cmd">sandboxctl service up ${escape(service.id)}</code>
</div>`
  )
  .join("");

await writeFile(
  join(out, "index.html"),
  page({
    title: "sandbox-labs — levanta sandboxes y comprueba qué contienen",
    description:
      "Levanta servicios dentro de un sandbox, ábrelos en el navegador y comprueba con sondas qué contiene realmente cada frontera de aislamiento.",
    body: (await readFile(join(root, "site-src", "index.html"), "utf8"))
      .replace("<!--SERVICES-->", serviceCards)
      .replace(/<!--LABS_COUNT-->/g, String(labs.length))
      .replace(/<!--SERVICES_COUNT-->/g, String(services.length))
      .replace(/<!--VERSION-->/g, catalog.project.version),
  })
);

console.log(
  `✅ Sitio generado: ${labs.length + 3} páginas (portada, conceptos, índice de labs y ${labs.length} laboratorios)`
);
