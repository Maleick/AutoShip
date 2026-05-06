#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
{
  [[ -f "$REPO_ROOT/AUTOSHIP_SPEC.md" ]] && cat "$REPO_ROOT/AUTOSHIP_SPEC.md"
  [[ -f "$REPO_ROOT/config/model-routing.json" ]] && cat "$REPO_ROOT/config/model-routing.json"
  find "$REPO_ROOT/skills" "$REPO_ROOT/commands" -maxdepth 3 -type f 2>/dev/null | sort | xargs shasum -a 256 2>/dev/null || true
} | shasum -a 256 | awk '{print $1}'
