import { realpath } from "node:fs/promises";
import { resolve, sep } from "node:path";

export function isInside(root, candidate) {
  return candidate === root || candidate.startsWith(`${root}${sep}`);
}

export function decodePath(urlPath) {
  try {
    let decoded = urlPath;
    for (let pass = 0; pass < 3; pass += 1) {
      const next = decodeURIComponent(decoded);
      if (next === decoded) break;
      decoded = next;
    }
    if (decoded.includes("\0") || decoded.includes("\\")) return null;
    if (decoded.split("/").some((segment) => segment === "." || segment === "..")) return null;
    return decoded;
  } catch {
    return null;
  }
}

export async function safePublicPath(publicRoot, urlPath) {
  const decoded = decodePath(urlPath);
  if (decoded === null) return null;
  const requested = decoded === "/" ? "/index.html" : decoded;
  const root = await realpath(publicRoot);
  const candidate = resolve(root, `.${requested}`);
  if (!isInside(root, candidate)) return null;
  try {
    const actual = await realpath(candidate);
    return isInside(root, actual) ? actual : null;
  } catch {
    return candidate;
  }
}

export function validIdentifier(value) {
  return typeof value === "string" && /^[a-z0-9-]{1,80}$/.test(value);
}

export function validateArguments(args) {
  return Array.isArray(args) && args.length <= 16 && args.every((arg) =>
    typeof arg === "string" && arg.length <= 256 && !/[\u0000-\u001f\u007f]/.test(arg)
  );
}


export function isTrustedHostHeader(value, host, port) {
  if (typeof value !== "string" || value.length > 120) return false;
  const escapedHost = host.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  if (Number(port) === 0) return new RegExp(`^(?:${escapedHost}|localhost):\\d+$`, "i").test(value);
  return value.toLowerCase() === `${host}:${port}`.toLowerCase() || value.toLowerCase() === `localhost:${port}`.toLowerCase();
}

export function isTrustedWriteRequest(request, host, port) {
  if (request.headers["x-sandbox-request"] !== "1") return false;
  const origin = request.headers.origin;
  if (!origin) return true;
  return origin === `http://${host}:${port}` || origin === `http://localhost:${port}`;
}

export async function readJsonBody(request, maxBytes = 32_768) {
  let size = 0;
  const chunks = [];
  for await (const chunk of request) {
    const value = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    size += value.length;
    if (size > maxBytes) throw new Error("body_too_large");
    chunks.push(value);
  }
  try {
    return JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
  } catch {
    throw new Error("invalid_json");
  }
}
