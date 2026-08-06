import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { repoPaths, defaultRepoRoot } from "./paths.ts";
import { isTrustedHostHeader, isTrustedWriteRequest, readJsonBody, safePublicPath, validIdentifier } from "./security.ts";
import { loadRegistry } from "./registry.ts";
import { JobStore, listEvidence } from "./jobs.ts";
import { actOnService, listServices, serviceLogs } from "./services.ts";

function sendJson(response, status, payload) {
  response.writeHead(status, { "content-type": "application/json; charset=utf-8", "x-content-type-options": "nosniff", "cache-control": "no-store" });
  response.end(JSON.stringify(payload, null, 2));
}

function contentType(path) {
  return ({ ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".svg": "image/svg+xml" })[extname(path)] ?? "application/octet-stream";
}

function errorStatus(error) {
  return ({ body_too_large: 413, invalid_json: 400, invalid_job: 400, unknown_workload: 400, unknown_policy: 400, unknown_runtime: 400, unknown_service: 400, unknown_action: 400, sandboxctl_unavailable: 503, invalid_arguments: 400, native_not_allowed_for_workload: 400 })[String(error?.message ?? error)] ?? 500;
}

export async function createSandboxServer(options = {}) {
  const repoRoot = options.repoRoot ?? defaultRepoRoot;
  const host = options.host ?? "127.0.0.1";
  const port = Number(options.port ?? process.env.PORT ?? 9093);
  const defaults = repoPaths(repoRoot);
  const paths = {
    ...defaults,
    dataRoot: options.dataRoot ?? defaults.dataRoot,
    evidenceRoot: options.evidenceRoot ?? defaults.evidenceRoot
  };
  const registry = await loadRegistry(paths);
  const jobs = new JobStore(paths, registry);
  await jobs.init();
  const server = createServer(async (request, response) => {
    try {
      if (!isTrustedHostHeader(request.headers.host, host, port)) {
        sendJson(response, 421, { error: "untrusted_host" });
        return;
      }
      const url = new URL(request.url ?? "/", `http://${host}:${port}`);
      if (url.pathname.startsWith("/api/")) {
        await api(request, response, url, { host, port, paths, registry, jobs });
        return;
      }
      if (request.method !== "GET" && request.method !== "HEAD") {
        sendJson(response, 405, { error: "method_not_allowed" });
        return;
      }
      const path = await safePublicPath(paths.publicRoot, url.pathname);
      if (!path) { sendJson(response, 400, { error: "invalid_path" }); return; }
      try {
        const content = await readFile(path);
        response.writeHead(200, {
          "content-type": contentType(path), "x-content-type-options": "nosniff",
          "content-security-policy": "default-src 'self'; style-src 'self'; script-src 'self'; connect-src 'self'",
          "referrer-policy": "no-referrer", "cross-origin-resource-policy": "same-origin",
          "permissions-policy": "camera=(), microphone=(), geolocation=()"
        });
        response.end(request.method === "HEAD" ? undefined : content);
      } catch { sendJson(response, 404, { error: "not_found" }); }
    } catch (error) {
      const status = errorStatus(error);
      // Una petición mal formada es un error del cliente, no del servidor: solo
      // los 5xx merecen ruido en la consola del operador.
      if (status >= 500) console.error(error);
      sendJson(response, status, { error: String(error?.message ?? error) });
    }
  });
  return { server, host, port, paths, registry, jobs };
}

async function api(request, response, url, ctx) {
  if (request.method === "GET" && url.pathname === "/api/system") { sendJson(response, 200, { name: "Sandbox Control Center", version: ctx.registry.catalog.project.version, host: ctx.host, port: ctx.port, safeMode: true, executionModel: "registered-workloads-only" }); return; }
  if (request.method === "GET" && url.pathname === "/api/catalog") { sendJson(response, 200, ctx.registry.catalog); return; }
  if (request.method === "GET" && url.pathname === "/api/policies") { sendJson(response, 200, ctx.registry.policies.map(({ path, file, ...policy }) => policy)); return; }
  if (request.method === "GET" && url.pathname === "/api/workloads") { sendJson(response, 200, ctx.registry.workloads.map(({ directory, manifestPath, ...workload }) => workload)); return; }
  if (request.method === "GET" && url.pathname === "/api/services") { sendJson(response, 200, await listServices(ctx.paths)); return; }
  const serviceAction = url.pathname.match(/^\/api\/services\/([a-z0-9-]+)\/(up|down)$/);
  if (request.method === "POST" && serviceAction) {
    // Levantar o bajar un sandbox es una escritura: exige la misma cabecera de
    // confianza que crear un trabajo.
    if (!isTrustedWriteRequest(request, ctx.host, ctx.port)) { sendJson(response, 403, { error: "untrusted_request" }); return; }
    const result = await actOnService(ctx.paths, serviceAction[1], serviceAction[2]);
    sendJson(response, result.ok ? 200 : 500, result);
    return;
  }
  const serviceLog = url.pathname.match(/^\/api\/services\/([a-z0-9-]+)\/logs$/);
  if (request.method === "GET" && serviceLog) { sendJson(response, 200, await serviceLogs(ctx.paths, serviceLog[1])); return; }
  if (request.method === "GET" && url.pathname === "/api/jobs") { sendJson(response, 200, ctx.jobs.list()); return; }
  if (request.method === "POST" && url.pathname === "/api/jobs") {
    if (!isTrustedWriteRequest(request, ctx.host, ctx.port)) { sendJson(response, 403, { error: "untrusted_request" }); return; }
    sendJson(response, 202, await ctx.jobs.create(await readJsonBody(request))); return;
  }
  const jobMatch = url.pathname.match(/^\/api\/jobs\/([a-z0-9-]+)$/);
  if (request.method === "GET" && jobMatch) { const job = ctx.jobs.get(jobMatch[1]); sendJson(response, job ? 200 : 404, job ?? { error: "job_not_found" }); return; }
  const cancel = url.pathname.match(/^\/api\/jobs\/([a-z0-9-]+)\/cancel$/);
  if (request.method === "POST" && cancel) {
    if (!isTrustedWriteRequest(request, ctx.host, ctx.port)) { sendJson(response, 403, { error: "untrusted_request" }); return; }
    const job = await ctx.jobs.cancel(cancel[1]); sendJson(response, job ? 200 : 404, job ?? { error: "job_not_found" }); return;
  }
  const events = url.pathname.match(/^\/api\/jobs\/([a-z0-9-]+)\/events$/);
  if (request.method === "GET" && events) {
    if (!ctx.jobs.get(events[1])) { sendJson(response, 404, { error: "job_not_found" }); return; }
    response.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache, no-store", connection: "keep-alive", "x-accel-buffering": "no" });
    const unsubscribe = ctx.jobs.subscribe(events[1], response);
    request.on("close", unsubscribe);
    return;
  }
  if (request.method === "GET" && url.pathname === "/api/evidence") { sendJson(response, 200, await listEvidence(ctx.paths.evidenceRoot)); return; }
  const evidence = url.pathname.match(/^\/api\/evidence\/([a-f0-9-]+)$/i);
  if (request.method === "GET" && evidence && validIdentifier(evidence[1])) {
    try { sendJson(response, 200, JSON.parse(await readFile(join(ctx.paths.evidenceRoot, `${evidence[1]}.json`), "utf8"))); }
    catch { sendJson(response, 404, { error: "evidence_not_found" }); }
    return;
  }
  sendJson(response, 404, { error: "api_not_found" });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const app = await createSandboxServer();
  app.server.listen(app.port, app.host, () => console.log(`Sandbox Control Center: http://${app.host}:${app.port}`));
}
