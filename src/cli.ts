#!/usr/bin/env node
import { parseArgs } from "node:util";
import {
  mkdir,
  writeFile,
  readFile,
  copyFile,
  readdir,
  stat,
  access,
} from "node:fs/promises";
import { resolve, join } from "node:path";
import { homedir } from "node:os";

const PACKAGE_ROOT = resolve(import.meta.dirname, "..");
const VERSION = (await readFile(join(PACKAGE_ROOT, "VERSION"), "utf8")).trim();

interface Config {
  plugin?: string[];
  [key: string]: unknown;
}

function resolveConfigDir(): string {
  if (process.env.OPENCODE_CONFIG_DIR) {
    return process.env.OPENCODE_CONFIG_DIR;
  }
  if (process.env.XDG_CONFIG_HOME) {
    return join(process.env.XDG_CONFIG_HOME, "opencode");
  }
  return join(homedir(), ".config", "opencode");
}

async function copyDir(src: string, dest: string): Promise<void> {
  await mkdir(dest, { recursive: true });
  const entries = await readdir(src);
  for (const entry of entries) {
    const srcPath = join(src, entry);
    const destPath = join(dest, entry);
    const st = await stat(srcPath);
    if (st.isDirectory()) {
      await copyDir(srcPath, destPath);
    } else {
      await copyFile(srcPath, destPath);
    }
  }
}

async function loadConfig(path: string): Promise<Config> {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    return {};
  }
}

async function saveConfig(path: string, config: Config): Promise<void> {
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
      const st = await stat(item.src);
      if (st.isDirectory()) {
        await copyDir(item.src, item.dest);
      } else {
        await copyFile(item.src, item.dest);
      }
    } catch {
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
      !p.includes("autoship.ts") &&
      !p.endsWith("/autoship.ts")
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

  interface Check {
    name: string;
    status: "PASS" | "WARN" | "FAIL";
    message: string;
  }

  const checks: Check[] = [];
  let hasFailure = false;

  const configDir = resolveConfigDir();
  const autoshipDir = join(configDir, ".autoship");
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
    await access(join(autoshipDir, "config.json"));
    checks.push({ name: "config", status: "PASS", message: "Config file exists" });
  } catch {
    checks.push({ name: "config", status: "FAIL", message: "Config file not found" });
    hasFailure = true;
  }

  try {
    await access(join(autoshipDir, "model-routing.json"));
    checks.push({ name: "model-routing", status: "PASS", message: "Model routing file exists" });
  } catch {
    checks.push({ name: "model-routing", status: "FAIL", message: "Model routing file not found" });
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
        const routingPath = join(autoshipDir, "model-routing.json");
        await access(routingPath);
        const routingContent = await readFile(routingPath, "utf8");
        const routing = JSON.parse(routingContent);
        const modelIds = modelsOutput.split("\n").map((l) => l.trim()).filter(Boolean);
        const configuredModels = (routing.models || []).map((m: { id: string }) => m.id);
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
