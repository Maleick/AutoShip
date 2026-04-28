#!/usr/bin/env node
import { parseArgs } from "node:util";
import {
  mkdir,
  writeFile,
  readFile,
  copyFile,
  readdir,
  lstat,
  access,
} from "node:fs/promises";
import { resolve, join } from "node:path";
import { homedir } from "node:os";
import { execSync } from "node:child_process";
import type { AutoshipConfig, DoctorCheck, ModelRouting, OpenCodeConfig } from "./types.js";

const PACKAGE_ROOT = resolve(import.meta.dirname, "..");
const VERSION = (await readFile(join(PACKAGE_ROOT, "VERSION"), "utf8")).trim();

function resolveConfigDir(): string {
  if (process.env.OPENCODE_CONFIG_DIR) {
    return process.env.OPENCODE_CONFIG_DIR;
  }
  if (process.env.XDG_CONFIG_HOME) {
    return join(process.env.XDG_CONFIG_HOME, "opencode");
  }
  return join(homedir(), ".config", "opencode");
}

function resolveProjectAutoshipDir(): string {
  try {
    const root = execSync("git rev-parse --show-toplevel", { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
    if (root) {
      return join(root, ".autoship");
    }
  } catch {
    // Fall back to the current directory when doctor is run outside git.
  }
  return join(process.cwd(), ".autoship");
}

async function copyDir(src: string, dest: string): Promise<void> {
  const srcStat = await lstat(src);
  if (srcStat.isSymbolicLink()) {
    throw new Error(`Refusing to copy symlinked source directory: ${src}`);
  }
  if (!srcStat.isDirectory()) {
    throw new Error(`Expected source directory but found non-directory: ${src}`);
  }

  try {
    const destStat = await lstat(dest);
    if (destStat.isSymbolicLink()) {
      throw new Error(`Refusing to write through symlinked path: ${dest}`);
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }

  await mkdir(dest, { recursive: true });
  const entries = await readdir(src);
  for (const entry of entries) {
    const srcPath = join(src, entry);
    const destPath = join(dest, entry);
    const st = await lstat(srcPath);
    if (st.isSymbolicLink()) {
      throw new Error(`Refusing to copy symlinked source path: ${srcPath}`);
    }
    if (st.isDirectory()) {
      await copyDir(srcPath, destPath);
    } else {
      try {
        const destStat = await lstat(destPath);
        if (destStat.isSymbolicLink()) {
          throw new Error(`Refusing to write through symlinked path: ${destPath}`);
        }
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
          throw error;
        }
      }
      await copyFile(srcPath, destPath);
    }
  }
}

async function loadConfig(path: string): Promise<OpenCodeConfig> {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
    return {};
  }
}

async function saveConfig(path: string, config: OpenCodeConfig): Promise<void> {
  try {
    const existing = await lstat(path);
    if (existing.isSymbolicLink()) {
      throw new Error(`Refusing to write through symlinked config file: ${path}`);
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }
  await writeFile(path, JSON.stringify(config, null, 2) + "\n", "utf8");
}

async function install() {
  const configDir = resolveConfigDir();
  const autoshipDir = join(configDir, ".autoship");

  console.log(`Installing opencode-autoship ${VERSION} to ${configDir}`);

  await mkdir(autoshipDir, { recursive: true });

  const items = [
    { src: join(PACKAGE_ROOT, "hooks"), dest: join(autoshipDir, "hooks") },
    { src: join(PACKAGE_ROOT, "commands"), dest: join(autoshipDir, "commands") },
    { src: join(PACKAGE_ROOT, "skills"), dest: join(autoshipDir, "skills") },
    { src: join(PACKAGE_ROOT, "plugins"), dest: join(autoshipDir, "plugins") },
    { src: join(PACKAGE_ROOT, "AGENTS.md"), dest: join(autoshipDir, "AGENTS.md") },
    { src: join(PACKAGE_ROOT, "VERSION"), dest: join(autoshipDir, "VERSION") },
  ];

  for (const item of items) {
    try {
      const st = await lstat(item.src);
      if (st.isSymbolicLink()) {
        throw new Error(`Refusing to install symlinked package asset: ${item.src}`);
      }
      if (st.isDirectory()) {
        await copyDir(item.src, item.dest);
      } else {
        try {
          const destStat = await lstat(item.dest);
          if (destStat.isSymbolicLink()) {
            throw new Error(`Refusing to write through symlinked path: ${item.dest}`);
          }
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
            throw error;
          }
        }
        await copyFile(item.src, item.dest);
      }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
      console.warn(`Warning: ${item.src} not found, skipping`);
    }
  }

  const configPath = join(configDir, "opencode.json");
  let config = await loadConfig(configPath);

  const newPlugin = "opencode-autoship";
  let plugins = config.plugin ?? [];
  if (typeof plugins === "string") {
    plugins = [plugins];
  }
  if (!plugins.includes(newPlugin)) {
    plugins = [...plugins, newPlugin];
  }

  plugins = plugins.filter(
    (p) =>
      typeof p === "string" &&
      !p.includes("autoship.ts")
  );

  config.plugin = plugins;
  await saveConfig(configPath, config);

  console.log(`\nSuccessfully installed opencode-autoship ${VERSION}`);
  console.log(`Config: ${configPath}`);
  console.log("\nNext: Run 'opencode-autoship --help' to get started");
}

async function doctor() {
  console.log("opencode-autoship doctor");
  console.log("=====================");
  console.log();

  const checks: DoctorCheck[] = [];
  let hasFailure = false;

  const configDir = resolveConfigDir();
  const autoshipDir = join(configDir, ".autoship");
  const projectAutoshipDir = resolveProjectAutoshipDir();
  const opencodeConfigPath = join(configDir, "opencode.json");

  try {
    const opencodeConfig = await loadConfig(opencodeConfigPath);
    const plugins = Array.isArray(opencodeConfig.plugin) ? opencodeConfig.plugin : [];
    if (plugins.includes("opencode-autoship")) {
      checks.push({ name: "package-registration", status: "PASS", message: "opencode-autoship is registered in opencode.json" });
    } else {
      checks.push({ name: "package-registration", status: "FAIL", message: "opencode.json does not register opencode-autoship; run opencode-autoship install" });
      hasFailure = true;
    }
  } catch {
    checks.push({ name: "package-registration", status: "FAIL", message: "Unable to read opencode.json; run opencode-autoship install" });
    hasFailure = true;
  }

  try {
    await access(join(autoshipDir, ".onboarded"));
    checks.push({ name: "onboarding", status: "PASS", message: "AutoShip is onboarded" });
  } catch {
    checks.push({ name: "onboarding", status: "WARN", message: "AutoShip has not been onboarded yet" });
  }

  try {
    await access(join(projectAutoshipDir, "config.json"));
    checks.push({ name: "config", status: "PASS", message: "Config file exists" });
  } catch {
    checks.push({ name: "config", status: "WARN", message: "Project .autoship/config.json not found; run /autoship-setup before dispatch" });
  }

  try {
    await access(join(projectAutoshipDir, "model-routing.json"));
    checks.push({ name: "model-routing", status: "PASS", message: "Model routing file exists" });
  } catch {
    checks.push({ name: "model-routing", status: "WARN", message: "Project .autoship/model-routing.json not found; run /autoship-setup before dispatch" });
  }

  for (const tool of ["gh", "git", "jq", "opencode"]) {
    try {
      execSync(`command -v ${tool}`, { stdio: "ignore", shell: "/bin/sh" });
      checks.push({ name: `tool-${tool}`, status: "PASS", message: `${tool} is available` });
    } catch {
      checks.push({ name: `tool-${tool}`, status: "FAIL", message: `${tool} is not available in PATH` });
      hasFailure = true;
    }
  }

  try {
    const configPath = join(projectAutoshipDir, "config.json");
    const config = JSON.parse(await readFile(configPath, "utf8")) as AutoshipConfig;
    const maxAgents = Number(config.maxConcurrentAgents ?? config.max_agents ?? 0);
    if (maxAgents > 0 && maxAgents <= 15) {
      checks.push({ name: "worker-cap", status: "PASS", message: `Worker cap is ${maxAgents}` });
    } else {
      checks.push({ name: "worker-cap", status: "WARN", message: `Worker cap ${maxAgents || "unset"} is outside recommended range 1-15` });
    }
  } catch {
    checks.push({ name: "worker-cap", status: "WARN", message: "Unable to validate worker cap" });
  }

  try {
    await access(join(autoshipDir, "hooks"));
    checks.push({ name: "hooks", status: "PASS", message: "Hooks directory exists" });
  } catch {
    checks.push({ name: "hooks", status: "FAIL", message: "Hooks directory not found" });
    hasFailure = true;
  }

  try {
    await access(join(autoshipDir, "commands"));
    checks.push({ name: "commands", status: "PASS", message: "Commands directory exists" });
  } catch {
    checks.push({ name: "commands", status: "FAIL", message: "Commands directory not found" });
    hasFailure = true;
  }

  try {
    await access(join(autoshipDir, "skills"));
    checks.push({ name: "skills", status: "PASS", message: "Skills directory exists" });
  } catch {
    checks.push({ name: "skills", status: "FAIL", message: "Skills directory not found" });
    hasFailure = true;
  }

  try {
    await access(join(autoshipDir, "AGENTS.md"));
    checks.push({ name: "agents-guide", status: "PASS", message: "AGENTS.md is installed" });
  } catch {
    checks.push({ name: "agents-guide", status: "FAIL", message: "AGENTS.md not found; run opencode-autoship install" });
    hasFailure = true;
  }

  try {
    const assetVersion = (await readFile(join(autoshipDir, "VERSION"), "utf8")).trim();
    if (assetVersion === VERSION) {
      checks.push({ name: "asset-version", status: "PASS", message: `Installed assets match package ${VERSION}` });
    } else {
      checks.push({ name: "asset-version", status: "FAIL", message: `Installed asset version ${assetVersion} does not match package ${VERSION}; run opencode-autoship install` });
      hasFailure = true;
    }
  } catch {
    checks.push({ name: "asset-version", status: "FAIL", message: "Installed VERSION not found; run opencode-autoship install" });
    hasFailure = true;
  }

  const { exec } = await import("node:child_process");
  const execAsync = (cmd: string): Promise<string> => new Promise((resolve, reject) => {
    exec(cmd, { encoding: "utf8", timeout: 30000 }, (err, stdout) => {
      if (err) reject(err);
      else resolve(stdout);
    });
  });

  try {
    const modelsOutput = await execAsync("opencode models");
    if (modelsOutput.trim().length > 0) {
      checks.push({ name: "model-inventory", status: "PASS", message: "OpenCode model inventory is accessible" });
      try {
        const routingPath = join(projectAutoshipDir, "model-routing.json");
        await access(routingPath);
        const routingContent = await readFile(routingPath, "utf8");
        const routing = JSON.parse(routingContent) as ModelRouting;
        const modelIds = modelsOutput.split("\n").map((l) => l.trim()).filter(Boolean);
        const configuredModels = (routing.models || []).map((m) => m.id);
        const missingModels = configuredModels.filter((m: string) => !modelIds.some((id: string) => id.includes(m) || m.includes(id)));
        if (missingModels.length > 0) {
          checks.push({ name: "model-routing-refs", status: "WARN", message: `Configured models not in current inventory: ${missingModels.join(", ")}` });
        } else {
          checks.push({ name: "model-routing-refs", status: "PASS", message: "All configured models are in current inventory" });
        }
      } catch {
        checks.push({ name: "model-routing-refs", status: "WARN", message: "Unable to validate model-routing.json references" });
      }
    } else {
      checks.push({ name: "model-inventory", status: "WARN", message: "OpenCode model inventory is empty" });
    }
  } catch {
    checks.push({ name: "model-inventory", status: "WARN", message: "Unable to access OpenCode model inventory; run /autoship-setup" });
  }

  try {
    await execAsync("gh auth status");
    checks.push({ name: "gh-auth", status: "PASS", message: "GitHub CLI is authenticated" });
    try {
      const statusOutput = await execAsync("gh auth status");
      const hasRepoScope = statusOutput.includes("repo") || statusOutput.includes("Full");
      if (hasRepoScope) {
        checks.push({ name: "gh-repo-perms", status: "PASS", message: "GitHub token has repo scope for issue-to-PR automation" });
      } else {
        checks.push({ name: "gh-repo-perms", status: "WARN", message: "GitHub token may lack repo scope; run 'gh auth refresh'" });
      }
    } catch {
      checks.push({ name: "gh-repo-perms", status: "WARN", message: "Unable to verify repo permissions" });
    }
  } catch {
    checks.push({ name: "gh-auth", status: "WARN", message: "GitHub CLI not authenticated; run 'gh auth login' or set GH_TOKEN" });
    checks.push({ name: "gh-repo-perms", status: "WARN", message: "GitHub auth required for permission check" });
  }

  const passChecks = checks.filter(c => c.status === "PASS");
  const warnChecks = checks.filter(c => c.status === "WARN");
  const failChecks = checks.filter(c => c.status === "FAIL");

  for (const check of passChecks) {
    console.log(`[PASS] ${check.name}: ${check.message}`);
  }
  for (const check of warnChecks) {
    console.log(`[WARN] ${check.name}: ${check.message}`);
  }
  for (const check of failChecks) {
    console.log(`[FAIL] ${check.name}: ${check.message}`);
  }
  console.log();
  console.log(`Summary: ${passChecks.length} passed, ${warnChecks.length} warnings, ${failChecks.length} failed`);

  if (hasFailure) {
    console.log();
    console.log("Run 'opencode-autoship install', then /autoship-setup to fix failures.");
    process.exit(1);
  }
}

function help() {
  console.log(`opencode-autoship ${VERSION}

Usage: opencode-autoship <command>

Commands:
  install   Install opencode-autoship to OpenCode config directory
  doctor    Run diagnostics
  help     Show this help message

Examples:
  opencode-autoship install
  opencode-autoship doctor
 `);
}

async function main() {
  const args = process.argv.slice(2);
  const command = args[0] ?? "help";

  switch (command) {
    case "install":
      await install();
      break;
    case "doctor":
      await doctor();
      break;
    case "help":
    case "--help":
    case "-h":
      help();
      break;
    case "--version":
    case "-v":
      console.log(`opencode-autoship ${VERSION}`);
      break;
    default:
      console.error(`Unknown command: ${command}`);
      help();
      process.exit(1);
  }
}

main().catch((err) => {
  console.error("Error:", err.message);
  process.exit(1);
});
