import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";

async function json(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function manifests(root) {
  const found = [];
  async function walk(dir) {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) await walk(path);
      else if (entry.name === "manifest.json") found.push({ ...(await json(path)), directory: dir, manifestPath: path });
    }
  }
  await walk(root);
  return found.sort((a, b) => a.id.localeCompare(b.id));
}

export async function loadRegistry(paths) {
  const catalog = await json(join(paths.repoRoot, "sandbox.config.json"));
  const policyNames = (await readdir(paths.policiesRoot)).filter((name) => name.endsWith(".json")).sort();
  const policies = await Promise.all(policyNames.map(async (name) => ({
    ...(await json(join(paths.policiesRoot, name))),
    file: name,
    path: join(paths.policiesRoot, name)
  })));
  const workloads = await manifests(paths.workloadsRoot);
  return {
    catalog,
    policies,
    workloads,
    runtimes: catalog.runtimes,
    policyById: new Map(policies.map((policy) => [policy.id, policy])),
    workloadById: new Map(workloads.map((workload) => [workload.id, workload])),
    runtimeById: new Map(catalog.runtimes.map((runtime) => [runtime.id, runtime]))
  };
}
