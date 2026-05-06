#!/usr/bin/env bash
set -euo pipefail

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

PACK_JSON="$TMP_DIR/npm-pack-dry-run.json"
npm pack --dry-run --json --ignore-scripts >"$PACK_JSON"

node - "$PACK_JSON" <<'NODE'
const fs = require("fs");
const path = require("path");

const packJsonPath = process.argv[2];
const raw = fs.readFileSync(packJsonPath, "utf8");
const packResult = JSON.parse(raw);
const entries = Array.isArray(packResult) ? packResult : [packResult];
const files = entries.flatMap((entry) => Array.isArray(entry.files) ? entry.files : []);

const allowedRoots = new Set(["dist", "hooks", "commands", "skills", "plugins", "policies"]);
const allowedFiles = new Set(["package.json", "README.md", "INSTALL.md", "LICENSE", "AGENTS.md", "VERSION"]);
const requiredFiles = [
  "dist/index.js",
  "dist/cli.js",
  "README.md",
  "INSTALL.md",
  "LICENSE",
  "AGENTS.md",
  "VERSION",
  ".opencode/INSTALL.md",
  "plugins/autoship.ts",
  "hooks/opencode/install.sh",
  "hooks/opencode/init.sh",
  "hooks/opencode/sync-release.sh",
  "skills/autoship-setup/SKILL.md",
  "commands/autoship-setup.md",
  "policies/default.json",
  "policies/textquest.json",
];

function normalizePath(filePath) {
  return filePath.replace(/^package\//, "");
}

function isForbidden(filePath) {
  return filePath === ".autoship" || filePath.startsWith(".autoship/");
}

function isAllowed(filePath) {
  if (allowedFiles.has(filePath)) {
    return true;
  }

  if (filePath === ".opencode/INSTALL.md") {
    return true;
  }

  const [root] = filePath.split("/");
  return allowedRoots.has(root);
}

const violations = [];
const packageJson = JSON.parse(fs.readFileSync("package.json", "utf8"));

if (packageJson.bin?.["opencode-autoship"] !== "dist/cli.js") {
  violations.push("package.json bin.opencode-autoship must be dist/cli.js for npm global installs");
}

if (packageJson.scripts?.prepublishOnly !== "bash hooks/opencode/verify-package.sh") {
  violations.push("package.json scripts.prepublishOnly must run bash hooks/opencode/verify-package.sh");
}

if (packageJson.repository?.url !== "git+https://github.com/Maleick/AutoShip.git") {
  violations.push("package.json repository.url must use the npm-normalized git+https URL");
}

for (const file of files) {
  const filePath = normalizePath(String(file.path || ""));
  if (!filePath) {
    continue;
  }

  if (isForbidden(filePath)) {
    violations.push(`${filePath} is runtime state and must not be published`);
  } else if (!isAllowed(filePath)) {
    violations.push(`${filePath} is not in the package allowlist`);
  }

  try {
    const stat = fs.lstatSync(path.join(process.cwd(), filePath));
    if (stat.isSymbolicLink()) {
      violations.push(`${filePath} must not be a symlink in the package`);
    }
  } catch (error) {
    violations.push(`${filePath} could not be inspected locally: ${error.message}`);
  }
}

if (files.length === 0) {
  violations.push("npm pack dry-run returned no package files");
}

const packagedPaths = new Set(files.map((file) => normalizePath(String(file.path || ""))).filter(Boolean));
for (const requiredFile of requiredFiles) {
  if (!packagedPaths.has(requiredFile)) {
    violations.push(`${requiredFile} is required in the package`);
  }
}

if (violations.length > 0) {
  console.error("FAIL: npm package contains unexpected files:");
  for (const violation of violations) {
    console.error(`- ${violation}`);
  }
  process.exit(1);
}

console.log(`Package dry-run verified ${files.length} files`);
NODE

node -e "import('./dist/index.js')" >/dev/null
