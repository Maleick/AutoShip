#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

if [[ "${1:-}" == "--build-artifacts" ]]; then
  # Clean Rust target dirs (both target/ and target-isolated/)
  find "$REPO_ROOT/.autoship/workspaces" -mindepth 2 -maxdepth 2 -type d \( -name target -o -name target-isolated \) 2>/dev/null | while IFS= read -r dir; do
    workspace_dir="${dir%/*}"
    status=$(tr -d '[:space:]' <"$workspace_dir/status" 2>/dev/null || echo UNKNOWN)
    case "$status" in
      RUNNING | VERIFYING | ACTIVE)
        echo "skipped active ${workspace_dir#$REPO_ROOT/}"
        continue
        ;;
    esac
    rm -rf "$dir"
    echo "removed ${dir#$REPO_ROOT/}"
  done

  # Clean graphify outputs (large knowledge graph artifacts)
  find "$REPO_ROOT/.autoship/workspaces" -mindepth 2 -maxdepth 2 -type d -name graphify-out 2>/dev/null | while IFS= read -r dir; do
    workspace_dir="${dir%/*}"
    status=$(tr -d '[:space:]' <"$workspace_dir/status" 2>/dev/null || echo UNKNOWN)
    case "$status" in
      RUNNING | VERIFYING | ACTIVE)
        echo "skipped active ${workspace_dir#$REPO_ROOT/}"
        continue
        ;;
    esac
    rm -rf "$dir"
    echo "removed ${dir#$REPO_ROOT/}"
  done

  # Clean Cargo registry caches within workspaces
  find "$REPO_ROOT/.autoship/workspaces" -mindepth 4 -maxdepth 4 -type d -name registry 2>/dev/null | while IFS= read -r dir; do
    workspace_dir="$(dirname "$dir")"
    workspace_dir="$(dirname "$workspace_dir")"
    workspace_dir="$(dirname "$workspace_dir")"
    status=$(tr -d '[:space:]' <"$workspace_dir/status" 2>/dev/null || echo UNKNOWN)
    case "$status" in
      RUNNING | VERIFYING | ACTIVE)
        echo "skipped active ${workspace_dir#$REPO_ROOT/}"
        continue
        ;;
    esac
    rm -rf "$dir"
    echo "removed ${dir#$REPO_ROOT/}"
  done

  exit 0
fi

# Full workspace cleanup for terminal states
find "$REPO_ROOT/.autoship/workspaces" -maxdepth 1 -type d -name 'issue-*' 2>/dev/null | while IFS= read -r dir; do
  status=$(tr -d '[:space:]' <"$dir/status" 2>/dev/null || echo UNKNOWN)
  case "$status" in
    COMPLETE | BLOCKED | STUCK)
      rm -rf "$dir"
      echo "removed $(basename "$dir")"
      ;;
  esac
done
