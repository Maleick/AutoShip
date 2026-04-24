#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
for dep in jq gh; do
  if ! command -v "$dep" >/dev/null 2>&1; then
    echo "Error: $dep is required" >&2
    exit 1
  fi
done

if [[ -n "${OPENCODE_CONFIG_DIR:-}" ]]; then
  CONFIG_DIR="$OPENCODE_CONFIG_DIR"
elif [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
  CONFIG_DIR="$XDG_CONFIG_HOME/opencode"
else
  CONFIG_DIR="$HOME/.config/opencode"
fi

PLUGIN_DIR="$CONFIG_DIR/plugins"
PLUGIN_DEST="$PLUGIN_DIR/autoship.ts"
CONFIG_FILE="$CONFIG_DIR/opencode.json"
PLUGIN_URL="file://$PLUGIN_DEST"

mkdir -p "$PLUGIN_DIR"

if [[ ! -w "$CONFIG_DIR" ]]; then
  echo "Error: config directory is not writable: $CONFIG_DIR" >&2
  exit 1
fi

"$SCRIPT_DIR/sync-release.sh"

if [[ ! -f "$CONFIG_FILE" ]]; then
  jq -n --arg plugin "$PLUGIN_URL" '{plugin: [$plugin]}' > "$CONFIG_FILE"
else
  jq --arg plugin "$PLUGIN_URL" '
    if has("plugin") and (.plugin | type != "array") then
      error("opencode.json plugin must be an array")
    else
      .plugin = ((.plugin // []) | if index($plugin) then . else . + [$plugin] end)
    end
  ' "$CONFIG_FILE" > "$CONFIG_FILE.tmp"
  mv "$CONFIG_FILE.tmp" "$CONFIG_FILE"
fi

bash "$SCRIPT_DIR/init.sh"

echo "Installed AutoShip into $CONFIG_DIR"
