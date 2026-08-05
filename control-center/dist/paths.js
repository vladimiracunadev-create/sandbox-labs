import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

export const sourceDir = fileURLToPath(new URL(".", import.meta.url));
export const defaultRepoRoot = resolve(sourceDir, "../..");

export function repoPaths(repoRoot = defaultRepoRoot) {
  return {
    repoRoot,
    publicRoot: resolve(repoRoot, "control-center/public"),
    policiesRoot: resolve(repoRoot, "policies"),
    workloadsRoot: resolve(repoRoot, "workloads"),
    evidenceRoot: resolve(repoRoot, "evidence/runs"),
    dataRoot: resolve(repoRoot, ".sandbox-data")
  };
}
