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
AUTOSHIP_HOME="$CONFIG_DIR/.autoship"
COMMANDS_DIR="$CONFIG_DIR/commands"
SKILLS_DIR="$CONFIG_DIR/skills"

assert_not_symlink() {
  local path="$1"
  if [[ -L "$path" ]]; then
    printf 'Error: refusing to operate on symlinked path: %s\n' "$path" >&2
    exit 1
  fi
}

assert_under_config() {
  local path="$1"
  local real_config real_path
  mkdir -p "$CONFIG_DIR"
  real_config="$(cd "$CONFIG_DIR" && pwd -P)"
  mkdir -p "$path"
  real_path="$(cd "$path" && pwd -P)"
  case "$real_path" in
    "$real_config" | "$real_config"/*) ;;
    *)
      printf 'Error: refusing to operate outside OpenCode config: %s\n' "$path" >&2
      exit 1
      ;;
  esac
}

assert_no_source_symlinks() {
  local path="$1"
  local found=""
  found=$(find "$path" -type l -print -quit 2>/dev/null || true)
  if [[ -n "$found" ]]; then
    printf 'Error: refusing to copy symlinked source path: %s\n' "$found" >&2
    exit 1
  fi
}

remove_managed_path() {
  local path="$1"
  local real_autoship real_parent
  assert_not_symlink "$path"
  if [[ ! -e "$path" ]]; then
    return 0
  fi
  real_autoship="$(cd "$AUTOSHIP_HOME" && pwd -P)"
  real_parent="$(cd "$(dirname "$path")" && pwd -P)"
  case "$real_parent/$(basename "$path")" in
    "$real_autoship"/*) ;;
    *)
      printf 'Error: refusing to remove managed path outside parent: %s\n' "$path" >&2
      exit 1
      ;;
  esac
  rm -rf "$path"
}

assert_not_symlink "$CONFIG_DIR"
assert_not_symlink "$PLUGIN_DIR"
assert_not_symlink "$AUTOSHIP_HOME"
assert_not_symlink "$COMMANDS_DIR"
assert_not_symlink "$SKILLS_DIR"
assert_not_symlink "$PLUGIN_DEST"
assert_not_symlink "$VERSION_FILE"
assert_under_config "$PLUGIN_DIR"
assert_under_config "$AUTOSHIP_HOME"
assert_under_config "$COMMANDS_DIR"
assert_under_config "$SKILLS_DIR"
mkdir -p "$PLUGIN_DIR" "$AUTOSHIP_HOME" "$COMMANDS_DIR" "$SKILLS_DIR"

if [[ "$(cd "$REPO_ROOT" && pwd -P)" == "$(cd "$AUTOSHIP_HOME" && pwd -P)" ]]; then
  assert_not_symlink "$AUTOSHIP_HOME/plugins"
  assert_not_symlink "$AUTOSHIP_HOME/plugins/autoship.ts"
  assert_not_symlink "$AUTOSHIP_HOME/commands"
  assert_not_symlink "$AUTOSHIP_HOME/skills"
  assert_not_symlink "$AUTOSHIP_HOME/VERSION"
  if [[ -f "$AUTOSHIP_HOME/plugins/autoship.ts" ]]; then
    cp -f "$AUTOSHIP_HOME/plugins/autoship.ts" "$PLUGIN_DEST"
  fi
  if [[ -d "$AUTOSHIP_HOME/commands" ]]; then
    cp -R "$AUTOSHIP_HOME/commands/." "$COMMANDS_DIR/"
  fi
  if [[ -d "$AUTOSHIP_HOME/skills" ]]; then
    cp -R "$AUTOSHIP_HOME/skills/." "$SKILLS_DIR/"
  fi
  if [[ -f "$AUTOSHIP_HOME/VERSION" ]]; then
    tr -d '[:space:]' <"$AUTOSHIP_HOME/VERSION" >"$VERSION_FILE"
  else
    printf '%s\n' "installed" >"$VERSION_FILE"
  fi
  exit 0
fi

copy_assets() {
  local src="$1"
  local managed
  for managed in hooks skills commands plugins policies src; do
    assert_not_symlink "$src/$managed"
    assert_no_source_symlinks "$src/$managed"
  done
  assert_not_symlink "$src/plugins/autoship.ts"
  rm -f "$PLUGIN_DEST"
  cp -f "$src/plugins/autoship.ts" "$PLUGIN_DEST"
  for managed in hooks skills commands plugins policies src; do
    remove_managed_path "$AUTOSHIP_HOME/$managed"
  done
  mkdir -p "$AUTOSHIP_HOME"
  cp -R "$src/hooks" "$AUTOSHIP_HOME/hooks"
  cp -R "$src/skills" "$AUTOSHIP_HOME/skills"
  cp -R "$src/commands" "$AUTOSHIP_HOME/commands"
  cp -R "$src/plugins" "$AUTOSHIP_HOME/plugins"
  cp -R "$src/policies" "$AUTOSHIP_HOME/policies"
  cp -R "$src/src" "$AUTOSHIP_HOME/src"
  cp -R "$src/commands/." "$COMMANDS_DIR/"
  cp -R "$src/skills/." "$SKILLS_DIR/"
  assert_not_symlink "$AUTOSHIP_HOME/AGENTS.md"
  assert_not_symlink "$AUTOSHIP_HOME/VERSION"
  if [[ -f "$src/AGENTS.md" ]]; then
    cp -f "$src/AGENTS.md" "$AUTOSHIP_HOME/AGENTS.md"
  fi
  if [[ -f "$src/VERSION" ]]; then
    cp -f "$src/VERSION" "$AUTOSHIP_HOME/VERSION"
  fi
}

copy_assets "$REPO_ROOT"
if [[ -f "$REPO_ROOT/VERSION" ]]; then
  tr -d '[:space:]' <"$REPO_ROOT/VERSION" >"$VERSION_FILE"
else
  printf '%s\n' "dev" >"$VERSION_FILE"
fi
