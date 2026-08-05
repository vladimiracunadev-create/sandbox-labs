// Comprueba un informe de `sandboxctl escape --json` contra lo que se espera
// de ese runtime.
//
// Vive aquí y no incrustado en el YAML de CI por tres motivos: se puede
// ejecutar en local igual que en el runner, shellcheck no tiene que adivinar
// qué hace un `node -e` de veinte líneas, y las expectativas quedan escritas
// en un sitio donde se leen.
//
// Uso:
//   node scripts/assert-containment.mjs <informe.json> --contained network,process
//   node scripts/assert-containment.mjs <informe.json> --expect-escape
//   node scripts/assert-containment.mjs <informe.json> --contained network --skip-if-unmeasurable

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const [, , reportPath, ...flags] = process.argv;

if (!reportPath) {
  console.error("uso: assert-containment.mjs <informe.json> [--contained a,b] [--expect-escape] [--skip-if-unmeasurable]");
  process.exit(2);
}

const flagValue = (name) => {
  const index = flags.indexOf(name);
  return index === -1 ? null : flags[index + 1] ?? "";
};
const hasFlag = (name) => flags.includes(name);

const report = JSON.parse(await readFile(resolve(reportPath), "utf8"));
const runtime = report.reports?.[0];

if (!runtime) {
  console.error(`::error::${reportPath} no contiene ningún informe de runtime`);
  process.exit(1);
}

const results = runtime.results ?? [];
const byDimension = new Map(results.map((value) => [value.dimension, value]));
const measured = results.filter((value) => value.verdict === "contained" || value.verdict === "escaped");

console.log(`Runtime: ${runtime.runtime} · disponible=${runtime.available} · ${results.length} sondas`);
for (const value of results) {
  console.log(`  ${value.verdict.padEnd(14)} ${value.dimension.padEnd(12)} ${value.detail}`);
}

// El entorno decide qué se puede medir. En un runner con user namespaces
// restringidos por AppArmor, `unshare` no llega ni a arrancar: eso no es una
// fuga, es que ahí no hay nada que medir. Se distingue de un fallo real.
if (measured.length === 0 && hasFlag("--skip-if-unmeasurable")) {
  console.log(`::notice::${runtime.runtime} no pudo medirse en este entorno; no hay nada que verificar`);
  process.exit(0);
}

let failures = 0;

const contained = flagValue("--contained");
if (contained) {
  for (const dimension of contained.split(",").filter(Boolean)) {
    const value = byDimension.get(dimension);
    if (!value) {
      console.error(`::error::${runtime.runtime}: no hay sonda para la dimensión ${dimension}`);
      failures += 1;
    } else if (value.verdict !== "contained") {
      console.error(`::error::${runtime.runtime} no contuvo ${dimension}: ${value.verdict} — ${value.detail}`);
      failures += 1;
    } else {
      console.log(`✅ ${runtime.runtime} contiene ${dimension}: ${value.detail}`);
    }
  }
}

// Contraprueba: sin aislamiento las sondas TIENEN que escaparse. Si aquí
// saliera todo contenido, las sondas no estarían midiendo nada y los ✅ del
// resto de la matriz no valdrían nada.
if (hasFlag("--expect-escape")) {
  const escaped = results.filter((value) => value.verdict === "escaped");
  if (escaped.length === 0) {
    console.error(`::error::${runtime.runtime} no escapó por ninguna dimensión: las sondas no están midiendo`);
    failures += 1;
  } else {
    console.log(`✅ ${runtime.runtime} escapa por ${escaped.length} dimensiones (esperado): ${escaped.map((v) => v.dimension).join(", ")}`);
  }
}

// Una falsa garantía invalida el informe aunque el resto cuadre.
const falseAssurances = results.filter((value) => value.declared && value.verdict === "escaped");
if (falseAssurances.length > 0) {
  for (const value of falseAssurances) {
    console.error(`::error::${runtime.runtime} DECLARA ${value.control} y la sonda ${value.probe} demostró que no lo aplica: ${value.detail}`);
  }
  failures += falseAssurances.length;
}

process.exit(failures > 0 ? 1 : 0);
