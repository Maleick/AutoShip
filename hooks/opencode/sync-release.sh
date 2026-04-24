#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [[ -n "${OPENCODE_CONFIG_DIR:-}" ]]; then
  CONFIG_DIR="$OPENCODE_CONFIG_DIR"
elif [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
  CONFIG_DIR="$XDG_CONFIG_HOME/opencode"
else
  CONFIG_DIR="$HOME/.config/opencode"
fi

PLUGIN_DIR="$CONFIG_DIR/plugins"
PLUGIN_DEST="$PLUGIN_DIR/autoship.ts"
VERSION_FILE="$PLUGIN_DIR/autoship.version"
REPO_REF="Maleick/AutoShip"

mkdir -p "$PLUGIN_DIR"

LATEST_TAG=""
if command -v gh >/dev/null 2>&1; then
  LATEST_TAG=$(gh api "repos/$REPO_REF/releases/latest" --jq '.tag_name' 2>/dev/null || true)
fi

if [[ -z "$LATEST_TAG" ]]; then
  rm -f "$PLUGIN_DEST"
  cp -f "$REPO_ROOT/plugins/autoship.ts" "$PLUGIN_DEST"
  printf '%s\n' "dev" > "$VERSION_FILE"
  exit 0
fi

CURRENT_TAG=""
if [[ -f "$VERSION_FILE" ]]; then
  CURRENT_TAG="$(cat "$VERSION_FILE" 2>/dev/null || true)"
fi

if [[ "$CURRENT_TAG" == "$LATEST_TAG" && -f "$PLUGIN_DEST" && ! -L "$PLUGIN_DEST" ]]; then
  exit 0
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

archive="$tmp_dir/autoship.tgz"
curl -fsSL "https://api.github.com/repos/$REPO_REF/tarball/$LATEST_TAG" -o "$archive"
tar -xzf "$archive" -C "$tmp_dir"

release_plugin=$(printf '%s\n' "$tmp_dir"/*/plugins/autoship.ts)
if [[ ! -f "$release_plugin" ]]; then
  rm -f "$PLUGIN_DEST"
  cp -f "$REPO_ROOT/plugins/autoship.ts" "$PLUGIN_DEST"
  printf '%s\n' "dev" > "$VERSION_FILE"
  exit 0
fi

rm -f "$PLUGIN_DEST"
cp "$release_plugin" "$PLUGIN_DEST"
printf '%s\n' "$LATEST_TAG" > "$VERSION_FILE"
