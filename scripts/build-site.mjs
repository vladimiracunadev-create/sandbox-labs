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
for (const entry of await readdir(join(root, catalog.casesDirectory), { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  services.push(JSON.parse(await readFile(join(root, catalog.casesDirectory, entry.name, "service.json"), "utf8")));
}
services.sort((a, b) => a.port - b.port);

const cases = catalog.cases.map((item) => {
  const built = services.find((service) => service.id === item.slug);
  return { ...item, built: Boolean(built), service: built ?? null };
});

await rm(out, { recursive: true, force: true });
await mkdir(out, { recursive: true });
await writeFile(join(out, "_style.css"), STYLE);

const STATUS = { ready: ["ready", "listo"], building: ["documented", "en obra"], planned: ["manual", "pendiente"] };

const caseCards = cases
  .map((item) => {
    const [chip, label] = STATUS[item.status] ?? STATUS.planned;
    return `<div class="card svc">
  <div class="card-top"><span class="num">${escape(item.id)} · :${item.port}</span><span class="chip ${chip}">${escape(label)}</span></div>
  <h3>${escape(item.title)}</h3>
  <p class="teaches"><b>La idea:</b> ${escape(item.idea)}</p>
  ${item.built ? `<p>${escape(item.service.description)}</p><code class="cmd">sandboxctl service up ${escape(item.slug)}</code>` : `<p>Todavía no construido.</p>`}
</div>`;
  })
  .join("");

await writeFile(
  join(out, "conceptos.html"),
  page({
    title: "Qué es un sandbox y por qué importa — sandbox-labs",
    description: "Qué es un sandbox, qué problema resuelve y en qué se diferencia de Docker, WSL y un unikernel.",
    active: "conceptos.html",
    body: await readFile(join(root, "site-src", "conceptos.html"), "utf8"),
  })
);

await writeFile(
  join(out, "index.html"),
  page({
    title: "sandbox-labs — ejecuta código que no controlas",
    description:
      "Cada caso es un producto que se levanta en su propio localhost, donde haces tareas reales, y que se apaga dejando constancia de qué pudo tocar.",
    body: (await readFile(join(root, "site-src", "index.html"), "utf8"))
      .replace("<!--SERVICES-->", caseCards)
      .replace(/<!--LABS_COUNT-->/g, String(cases.length))
      .replace(/<!--SERVICES_COUNT-->/g, String(cases.filter((c) => c.built).length))
      .replace(/<!--VERSION-->/g, catalog.project.version),
  })
);

console.log(
  `✅ Sitio generado: portada y conceptos · ${cases.length} casos (${cases.filter((c) => c.built).length} construidos)`
);