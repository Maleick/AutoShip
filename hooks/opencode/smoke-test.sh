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

bash "$REPO_ROOT/hooks/opencode/install.sh" >/dev/null

CONFIG_FILE="$CONFIG_HOME/opencode/opencode.json"
STATE_FILE="$REPO_ROOT/.autoship/state.json"
HOOKS_FILE="$REPO_ROOT/.autoship/hooks_dir"
PLUGIN_DEST="$CONFIG_HOME/opencode/plugins/autoship.ts"
VERSION_FILE="$CONFIG_HOME/opencode/plugins/autoship.version"
PLUGIN_URL="file://$PLUGIN_DEST"

[[ -f "$PLUGIN_DEST" ]]
[[ -f "$VERSION_FILE" ]]
[[ -f "$STATE_FILE" ]]
[[ "$(cat "$HOOKS_FILE")" == "$REPO_ROOT/hooks" ]]
grep -F "\"$PLUGIN_URL\"" "$CONFIG_FILE" >/dev/null

bash "$REPO_ROOT/hooks/opencode/init.sh" >/dev/null

[[ -f "$STATE_FILE" ]]
[[ "$(cat "$HOOKS_FILE")" == "$REPO_ROOT/hooks" ]]
jq -e '.config.maxConcurrentAgents == 15' "$STATE_FILE" >/dev/null
[[ -f "$REPO_ROOT/.autoship/model-routing.json" ]]
jq -e '[.models[] | select(.cost == "free")] | length > 0' "$REPO_ROOT/.autoship/model-routing.json" >/dev/null
jq -e 'all(.models[]; .id | test("^[a-z0-9._-]+/.+"))' "$REPO_ROOT/.autoship/model-routing.json" >/dev/null

bash "$REPO_ROOT/hooks/opencode/test-policy.sh" >/dev/null

echo "OpenCode install smoke test passed"
