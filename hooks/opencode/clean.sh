#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
find "$REPO_ROOT/.autoship/workspaces" -maxdepth 1 -type d -name 'issue-*' 2>/dev/null | while IFS= read -r dir; do
  status=$(tr -d '[:space:]' < "$dir/status" 2>/dev/null || echo UNKNOWN)
  case "$status" in
    COMPLETE|BLOCKED|STUCK) rm -rf "$dir"; echo "removed $(basename "$dir")" ;;
  esac
done
