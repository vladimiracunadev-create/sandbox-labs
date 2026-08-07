/**
 * Llama al núcleo de un caso técnico desde Node.
 *
 * Los núcleos son módulos de Python puros —sin red, sin servidor, sin estado—
 * precisamente para poder comprobarlos así: se les pasa un cuerpo JSON por la
 * entrada estándar y devuelven su informe por la salida. Es la misma puerta que
 * usa el servicio, de modo que lo que se comprueba aquí es lo que corre allí.
 */
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** En algunos sistemas el binario es `python`, no `python3`. */
export const python = process.env.PYTHON ?? (process.platform === "win32" ? "python" : "python3");

/**
 * Ejecuta `core.handle(payload)` del caso indicado.
 *
 * El adaptador vive aquí y no en cada núcleo para que los núcleos no tengan que
 * saber que alguien los llama desde fuera.
 */
export function callCore(directory, payload) {
  const script = [
    "import json,sys,importlib.util",
    `spec = importlib.util.spec_from_file_location("core", ${JSON.stringify(resolve(root, "cases", directory, "core.py"))})`,
    "module = importlib.util.module_from_spec(spec)",
    "spec.loader.exec_module(module)",
    "print(json.dumps(module.handle(json.load(sys.stdin)), ensure_ascii=False))"
  ].join("\n");

  const run = spawnSync(python, ["-c", script], {
    input: JSON.stringify(payload),
    encoding: "utf8",
    timeout: 20_000,
    // En Windows, Python escribe la salida estándar en la página de códigos del
    // sistema y los acentos llegan rotos. Los informes están en castellano, así
    // que la codificación se fija aquí en vez de renunciar a las tildes.
    env: { ...process.env, PYTHONIOENCODING: "utf-8", PYTHONUTF8: "1" }
  });

  if (run.status !== 0) {
    throw new Error(`el núcleo de ${directory} terminó con ${run.status}:\n${run.stderr}`);
  }
  return JSON.parse(run.stdout);
}
