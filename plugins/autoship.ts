import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { readFileSync } from "node:fs";

const moduleDir = dirname(fileURLToPath(import.meta.url));

export const repoRoot = resolve(moduleDir, "..");
export const version = readFileSync(resolve(repoRoot, "VERSION"), "utf8").trim();
export const id = "autoship";

export async function server() {
  return {
    event() {
      return undefined;
    },
  };
}
