// Borra el estado temporal de las pruebas: datos de ejecución y evidencias.
//
// Se niega a hacerlo si hay servicios levantados. Borrar `.sandbox-data` con un
// servicio en marcha destruye el registro que lo nombra, y desde que un servicio
// ya no muere con el CLI que lo levantó —eso se quitó a propósito para que
// sobreviva a `service up`— eso deja un sandbox corriendo que nada del CLI
// vuelve a encontrar. Pasó: tres del caso 03 estuvieron cuatro horas vivos.
import { readdir, readFile, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const servicesRoot = resolve(root, ".sandbox-data", "services");

/** Servicios con registro, que es lo que se perdería al borrar. */
async function registered() {
  try {
    const names = (await readdir(servicesRoot)).filter((name) => name.endsWith(".json"));
    const found = [];
    for (const name of names) {
      try {
        const record = JSON.parse(await readFile(resolve(servicesRoot, name), "utf8"));
        found.push({ id: record.id, pid: record.pid });
      } catch {
        // Un registro ilegible no bloquea la limpieza: ya no nombra nada.
      }
    }
    return found;
  } catch {
    return [];
  }
}

const running = await registered();
if (running.length > 0) {
  console.error(`❌ Hay ${running.length} servicio(s) con registro. Borrar ahora los dejaría corriendo sin`);
  console.error("   que nada del CLI pueda volver a encontrarlos.\n");
  for (const service of running) console.error(`   · ${service.id} (PID ${service.pid})`);
  console.error("\n   Bájalos primero:\n");
  console.error("     cargo run -p sandboxctl -- service down --all\n");
  process.exit(1);
}

await rm(resolve(root, ".sandbox-data"), { recursive: true, force: true });
const evidenceRoot = resolve(root, "evidence", "runs");
for (const name of await readdir(evidenceRoot)) {
  if (name.endsWith(".json") || name === ".chain") await rm(resolve(evidenceRoot, name), { force: true });
}
console.log("✅ Estado temporal de pruebas eliminado");
