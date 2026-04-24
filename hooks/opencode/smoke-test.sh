#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "Error: not inside a git repository" >&2
  exit 1
}

CONFIG_HOME="$(mktemp -d)"
AUTOSHIP_BACKUP=""
if [[ -d "$REPO_ROOT/.autoship" ]]; then
  AUTOSHIP_BACKUP="$(mktemp -d)"
  cp -R "$REPO_ROOT/.autoship" "$AUTOSHIP_BACKUP/"
fi

trap 'rm -rf "$CONFIG_HOME"; if [[ -n "$AUTOSHIP_BACKUP" && -d "$AUTOSHIP_BACKUP/.autoship" ]]; then rm -rf "$REPO_ROOT/.autoship"; cp -R "$AUTOSHIP_BACKUP/.autoship" "$REPO_ROOT/"; fi; rm -rf "$AUTOSHIP_BACKUP"' EXIT

export XDG_CONFIG_HOME="$CONFIG_HOME"

PACKAGE_REPO="$(mktemp -d)"
cp -R "$REPO_ROOT/." "$PACKAGE_REPO/"
rm -rf "$PACKAGE_REPO/.git" "$PACKAGE_REPO/.autoship" "$PACKAGE_REPO/node_modules" "$PACKAGE_REPO/dist"
(cd "$PACKAGE_REPO" && npm install --package-lock=false --no-audit --no-fund >/dev/null && npm run build >/dev/null)
node "$PACKAGE_REPO/dist/cli.js" install >/dev/null

CONFIG_FILE="$CONFIG_HOME/opencode/opencode.json"
STATE_FILE="$REPO_ROOT/.autoship/state.json"
HOOKS_FILE="$REPO_ROOT/.autoship/hooks_dir"
AUTOSHIP_INSTALL_DIR="$CONFIG_HOME/opencode/.autoship"

jq -e '.plugin | index("opencode-autoship")' "$CONFIG_FILE" >/dev/null
if jq -e '.plugin[] | select(type == "string" and contains("autoship.ts"))' "$CONFIG_FILE" >/dev/null; then
  echo "FAIL: package install registered legacy autoship.ts plugin" >&2
  exit 1
fi
[[ -d "$AUTOSHIP_INSTALL_DIR/hooks" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/commands" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/skills" ]]
[[ -f "$AUTOSHIP_INSTALL_DIR/AGENTS.md" ]]
[[ -f "$AUTOSHIP_INSTALL_DIR/VERSION" ]]

bash "$REPO_ROOT/hooks/opencode/init.sh" >/dev/null

[[ -f "$STATE_FILE" ]]
[[ "$(cat "$HOOKS_FILE")" == "$REPO_ROOT/hooks" ]]
jq -e '.config.maxConcurrentAgents == 15' "$STATE_FILE" >/dev/null
[[ -f "$REPO_ROOT/.autoship/model-routing.json" ]]
jq -e '[.models[] | select(.cost == "free")] | length > 0' "$REPO_ROOT/.autoship/model-routing.json" >/dev/null
jq -e 'all(.models[]; .id | test("^[a-z0-9._-]+/.+"))' "$REPO_ROOT/.autoship/model-routing.json" >/dev/null

bash "$REPO_ROOT/hooks/opencode/test-policy.sh" >/dev/null

echo "OpenCode install smoke test passed"
