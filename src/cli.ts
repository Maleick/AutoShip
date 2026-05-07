#!/usr/bin/env node
import { parseArgs } from "node:util";
import {
  mkdir,
  writeFile,
  readFile,
  copyFile,
  readdir,
  stat,
  lstat,
  access,
} from "node:fs/promises";
import { dirname, resolve, join } from "node:path";
import { homedir } from "node:os";
import { execSync } from "node:child_process";

const PACKAGE_ROOT = resolve(import.meta.dirname, "..");
const packageJson = JSON.parse(await readFile(join(PACKAGE_ROOT, "package.json"), "utf8")) as { version?: string };
const VERSION = `v${packageJson.version ?? "0.0.0"}`;

import type { AutoshipConfig, DoctorCheck } from "./types.ts";

interface Config {
  plugin?: string[];
  command?: Record<string, CommandConfig>;
  enabled_providers?: string[];
  disabled_providers?: string[];
  [key: string]: unknown;
}

interface CommandConfig {
  description: string;
  template: string;
  agent: string;
  subtask: boolean;
}

const AUTOSHIP_COMMANDS: Record<string, Omit<CommandConfig, "template">> = {
  autoship: { description: "Start AutoShip orchestration", agent: "build", subtask: false },
  "autoship-setup": { description: "Configure AutoShip model routing and first-run setup", agent: "build", subtask: false },
  "autoship-status": { description: "Show AutoShip orchestration status", agent: "build", subtask: false },
  "autoship-stop": { description: "Gracefully stop AutoShip orchestration", agent: "build", subtask: false },
  "autoship-plan": { description: "Dry-run AutoShip issue dispatch planning", agent: "planner", subtask: true },
  "autoship-apply": { description: "Apply a proposed AutoShip workspace by creating its PR", agent: "build", subtask: false },
  "autoship-audit": { description: "Audit AutoShip local state against GitHub", agent: "planner", subtask: true },
  "autoship-cancel": { description: "Cancel an AutoShip issue workspace", agent: "build", subtask: false },
  "autoship-clean": { description: "Clean terminal AutoShip workspaces", agent: "build", subtask: false },
  "autoship-dashboard": { description: "Show AutoShip dashboard metrics", agent: "build", subtask: false },
  "autoship-retry": { description: "Retry a blocked or stuck AutoShip issue", agent: "build", subtask: false },
};

const ENABLED_PROVIDERS = ["kimi-for-coding", "nvidia", "openai", "opencode", "opencode-go", "openrouter"];
const DISABLED_PROVIDERS = ["github-copilot"];
const LEGACY_PLUGIN_REGISTRATIONS = new Set([
  "opencode-autoship@latest",
]);

type InstallItem =
  | { src: string; dest: string; content?: never }
  | { content: string; dest: string; src?: never };

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
    throw new Error(`Refusing to copy symlinked package asset: ${src}`);
  }
  await assertWritablePath(dest, "OpenCode asset");
  await mkdir(dest, { recursive: true });
  const entries = await readdir(src);
  for (const entry of entries) {
    const srcPath = join(src, entry);
    const destPath = join(dest, entry);
    const linkStat = await lstat(srcPath);
    if (linkStat.isSymbolicLink()) {
      throw new Error(`Refusing to copy symlinked package asset: ${srcPath}`);
    }
    const st = await stat(srcPath);
    if (st.isDirectory()) {
      await copyDir(srcPath, destPath);
    } else {
      await assertWritablePath(destPath, "OpenCode asset");
      await copyFile(srcPath, destPath);
    }
  }
}

async function assertWritablePath(path: string, label: string): Promise<void> {
  try {
    const st = await lstat(path);
    if (st.isSymbolicLink()) {
      throw new Error(`Refusing to write symlinked ${label}: ${path}`);
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      throw error;
    }
  }
}

async function loadConfig(path: string): Promise<Config> {
  try {
    const raw = await readFile(path, "utf8");
    const parsed = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return {};
    }
    return parsed as Config;
  } catch {
    return {};
  }
}

async function saveConfig(path: string, config: Config): Promise<void> {
  await assertWritablePath(path, "OpenCode config");
  await writeFile(path, JSON.stringify(config, null, 2) + "\n", "utf8");
}

async function install() {
  const configDir = resolveConfigDir();
  const autoshipDir = join(configDir, ".autoship");

  console.log(`Installing opencode-autoship ${VERSION} to ${configDir}`);

  await assertWritablePath(configDir, "OpenCode config root");
  await assertWritablePath(autoshipDir, "OpenCode asset root");
  await mkdir(autoshipDir, { recursive: true });

  const items: InstallItem[] = [
    { src: join(PACKAGE_ROOT, "hooks"), dest: join(autoshipDir, "hooks") },
    { src: join(PACKAGE_ROOT, "commands"), dest: join(autoshipDir, "commands") },
    { src: join(PACKAGE_ROOT, "skills"), dest: join(autoshipDir, "skills") },
    { src: join(PACKAGE_ROOT, "plugins"), dest: join(autoshipDir, "plugins") },
    { src: join(PACKAGE_ROOT, "commands"), dest: join(configDir, "commands") },
    { src: join(PACKAGE_ROOT, "skills"), dest: join(configDir, "skills") },
    { src: join(PACKAGE_ROOT, "AGENTS.md"), dest: join(autoshipDir, "AGENTS.md") },
    { content: `${VERSION}\n`, dest: join(autoshipDir, "VERSION") },
    { content: `${VERSION}\n`, dest: join(configDir, "plugins", "autoship.version") },
  ];

  for (const item of items) {
    try {
      if (item.content !== undefined) {
        await assertWritablePath(item.dest, "OpenCode asset");
        await mkdir(dirname(item.dest), { recursive: true });
        await writeFile(item.dest, item.content, "utf8");
        continue;
      }
      const linkStat = await lstat(item.src);
      if (linkStat.isSymbolicLink()) {
        throw new Error(`Refusing to copy symlinked package asset: ${item.src}`);
      }
      const st = await stat(item.src);
      if (st.isDirectory()) {
        await copyDir(item.src, item.dest);
      } else {
        await assertWritablePath(item.dest, "OpenCode asset");
        await copyFile(item.src, item.dest);
      }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") {
        console.warn(`Warning: ${item.src} not found, skipping`);
        continue;
      }
      throw error;
    }
  }

  const configPath = join(configDir, "opencode.json");
  let config = await loadConfig(configPath);

  const newPlugin = "opencode-autoship";
  let plugins = config.plugin ?? [];
  if (typeof plugins === "string") {
    plugins = [plugins];
  }
  plugins = plugins.filter((plugin) => !isLegacyPluginRegistration(plugin));
  if (!plugins.includes(newPlugin)) {
    plugins = [...plugins, newPlugin];
  }

  config.plugin = plugins;
  config.command = { ...(config.command ?? {}) };
  for (const [name, command] of Object.entries(AUTOSHIP_COMMANDS)) {
    config.command[name] = {
      ...command,
      template: `{file:commands/${name}.md}\n\n$ARGUMENTS`,
    };
  }
  config.enabled_providers = ENABLED_PROVIDERS;
  config.disabled_providers = DISABLED_PROVIDERS;
  await saveConfig(configPath, config);

  console.log(`\nSuccessfully installed opencode-autoship ${VERSION}`);
  console.log(`Config: ${configPath}`);
  console.log("\nNext: Run 'opencode-autoship --help' to get started");
}

function isLegacyPluginRegistration(plugin: string): boolean {
  if (LEGACY_PLUGIN_REGISTRATIONS.has(plugin)) {
    return true;
  }
  return plugin.startsWith("file://") && plugin.includes("autoship");
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
    checks.push({ name: "config", status: "FAIL", message: "Project .autoship/config.json not found; run /autoship-setup" });
    hasFailure = true;
  }

  try {
    await access(join(projectAutoshipDir, "model-routing.json"));
    checks.push({ name: "model-routing", status: "PASS", message: "Model routing file exists" });
  } catch {
    checks.push({ name: "model-routing", status: "FAIL", message: "Project .autoship/model-routing.json not found; run /autoship-setup" });
    hasFailure = true;
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
        const routing = JSON.parse(routingContent) as { models?: Array<{ id: string }> };
        const modelIds = modelsOutput.split("\n").map((l) => l.trim()).filter(Boolean);
        const configuredModels = (routing.models || []).map((m) => m.id);
        const missingModels = configuredModels.filter((m) => !modelIds.some((id) => id.includes(m) || m.includes(id)));
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
