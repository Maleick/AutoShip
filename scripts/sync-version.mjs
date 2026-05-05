#!/usr/bin/env node
import { readFile, writeFile } from "node:fs/promises";

const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
const version = packageJson.version;

if (typeof version !== "string" || version.length === 0) {
  throw new Error("package.json version is missing");
}

await writeFile(new URL("../VERSION", import.meta.url), `v${version}\n`, "utf8");
