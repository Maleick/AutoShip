#!/usr/bin/env bash
set -euo pipefail

autoship_install_package_fixture() {
  local repo_root="${1:-}"
  local config_home="${2:-}"
  local fixture_dir="${3:-}"

  if [[ -z "$repo_root" || -z "$config_home" ]]; then
    printf 'Usage: autoship_install_package_fixture <repo-root> <config-home> [fixture-dir]\n' >&2
    return 2
  fi

  if [[ -z "$fixture_dir" ]]; then
    fixture_dir="$(mktemp -d)"
  fi

  local package_source="$fixture_dir/source"
  local pack_dir="$fixture_dir/pack"
  local extract_dir="$fixture_dir/extract"
  local install_config_dir="$config_home/opencode"

  mkdir -p "$pack_dir" "$extract_dir" "$install_config_dir"
  cp -R "$repo_root/." "$package_source/"
  rm -rf "$package_source/.git" "$package_source/.autoship" "$package_source/node_modules" "$package_source/dist"

  (
    cd "$package_source"
    npm install --package-lock=false --no-audit --no-fund >/dev/null
    npm pack --pack-destination "$pack_dir" --silent >/dev/null
  )

  local package_tarball
  package_tarball="$(find "$pack_dir" -maxdepth 1 -name '*.tgz' -print -quit)"
  if [[ -z "$package_tarball" ]]; then
    printf 'FAIL: package fixture did not produce a tarball\n' >&2
    return 1
  fi

  tar -xzf "$package_tarball" -C "$extract_dir"

  export XDG_CONFIG_HOME="$config_home"
  export OPENCODE_CONFIG_DIR="$install_config_dir"
  AUTOSHIP_PACKAGE_FIXTURE_DIR="$fixture_dir"
  AUTOSHIP_PACKAGE_FIXTURE_ROOT="$extract_dir/package"
  AUTOSHIP_PACKAGE_FIXTURE_CONFIG_DIR="$install_config_dir"
  export AUTOSHIP_PACKAGE_FIXTURE_DIR AUTOSHIP_PACKAGE_FIXTURE_ROOT AUTOSHIP_PACKAGE_FIXTURE_CONFIG_DIR

  node "$AUTOSHIP_PACKAGE_FIXTURE_ROOT/dist/cli.js" install >/dev/null
}
