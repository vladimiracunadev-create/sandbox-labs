import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { decodePath, isTrustedHostHeader, safePublicPath, validIdentifier, validateArguments } from "../dist/security.js";

test("acepta archivo interno", async () => {
  const root = await mkdtemp(join(tmpdir(), "sl-public-"));
  await writeFile(join(root, "index.html"), "ok");
  assert.equal(await safePublicPath(root, "/index.html"), join(root, "index.html"));
});

test("rechaza traversals simples y doblemente codificados", async () => {
  const root = await mkdtemp(join(tmpdir(), "sl-public-"));
  await writeFile(join(root, "index.html"), "ok");
  for (const path of ["/../secret", "/%2e%2e/secret", "/%252e%252e/secret", "/..%2fsecret", "/foo\\bar", "/%00bad"]) {
    assert.equal(await safePublicPath(root, path), null, path);
  }
});

// Windows solo permite crear symlinks con privilegios o con el modo
// desarrollador activo. Sin ellos la prueba no puede montar el escenario, así
// que se salta en vez de reportar un fallo que no es del código bajo prueba.
test("rechaza symlink que sale del public root", async (t) => {
  const base = await mkdtemp(join(tmpdir(), "sl-public-"));
  const root = join(base, "public");
  await mkdir(root);
  await writeFile(join(base, "secret"), "x");
  try {
    await symlink(join(base, "secret"), join(root, "link"));
  } catch (error) {
    if (error.code === "EPERM" || error.code === "EACCES") {
      t.skip("el sistema no permite crear symlinks sin privilegios");
      return;
    }
    throw error;
  }
  assert.equal(await safePublicPath(root, "/link"), null);
});

test("valida Host local y bloquea DNS rebinding", () => {
  assert.equal(isTrustedHostHeader("127.0.0.1:9093", "127.0.0.1", 9093), true);
  assert.equal(isTrustedHostHeader("localhost:9093", "127.0.0.1", 9093), true);
  assert.equal(isTrustedHostHeader("evil.example:9093", "127.0.0.1", 9093), false);
  assert.equal(isTrustedHostHeader("127.0.0.1:43123", "127.0.0.1", 0), true);
  assert.equal(isTrustedHostHeader(undefined, "127.0.0.1", 9093), false);
  assert.equal(isTrustedHostHeader("127.0.0.1:9093".padEnd(200, "x"), "127.0.0.1", 9093), false);
});

test("acepta solo identificadores del catálogo", () => {
  for (const value of ["hello", "dry-run", "wasi-hello", "a"]) {
    assert.equal(validIdentifier(value), true, value);
  }
  for (const value of ["../etc", "Hello", "con espacio", "", "x".repeat(81), 42, null]) {
    assert.equal(validIdentifier(value), false, String(value));
  }
});

test("limita los argumentos que llegan al runtime", () => {
  assert.equal(validateArguments([]), true);
  assert.equal(validateArguments(["uno", "dos"]), true);
  assert.equal(validateArguments("no-es-array"), false);
  assert.equal(validateArguments([42]), false);
  assert.equal(validateArguments([`con${String.fromCharCode(0)}nulo`]), false);
  assert.equal(validateArguments(["salto\nde\nlinea"]), false);
  assert.equal(validateArguments(["x".repeat(257)]), false);
  assert.equal(validateArguments(Array.from({ length: 17 }, () => "x")), false);
});

test("normaliza rutas y descarta codificaciones anidadas", () => {
  assert.equal(decodePath("/index.html"), "/index.html");
  assert.equal(decodePath("/%69ndex.html"), "/index.html");
  assert.equal(decodePath("/../secret"), null);
  assert.equal(decodePath("/a/./b"), null);
  assert.equal(decodePath("/a%5Cb"), null);
  assert.equal(decodePath("/%ZZ"), null);
});
