#!/usr/bin/env bash
set -euo pipefail

detect_commands() {
  local test_cmd=""
  local build_cmd=""
  if [[ -f package.json ]]; then
    if jq -e '.scripts.test' package.json >/dev/null 2>&1; then
      test_cmd="npm test"
    fi
    if jq -e '.scripts.build' package.json >/dev/null 2>&1; then
      build_cmd="npm run build"
    fi
  elif [[ -f pyproject.toml ]]; then
    test_cmd="pytest"
  elif [[ -f Cargo.toml ]]; then
    test_cmd="cargo test"
    build_cmd="cargo build"
  fi
  jq -n --arg test "$test_cmd" --arg build "$build_cmd" '{testCommand:$test,buildCommand:$build}'
}

detect_commands
