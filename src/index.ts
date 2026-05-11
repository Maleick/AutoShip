import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { readFileSync } from "node:fs";
import type { PluginServer } from "./types.js";
import { classifyError, getRecoverySuggestion, formatRecoverySuggestion } from "./recovery.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const packageRoot = resolve(__dirname, "..");

export const id = "autoship";
export const repoRoot = packageRoot;

const versionPath = resolve(packageRoot, "VERSION");
export const version = readFileSync(versionPath, "utf8").trim();

export async function server(): Promise<PluginServer> {
  return {
    event() {
      return undefined;
    },
  };
}

export default server;

export * from "./types.js";
export * from "./error.js";
export { classifyError, getRecoverySuggestion, formatRecoverySuggestion } from "./recovery.js";
