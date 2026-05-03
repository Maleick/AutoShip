#!/usr/bin/env bash
# Hermes agent runner — execute Hermes worker via cronjob or delegate_task
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load shared utilities if available
if [[ -f "$SCRIPT_DIR/../lib/common.sh" ]]; then
  source "$SCRIPT_DIR/../lib/common.sh"
else
  autoship_repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || {
      echo "Error: not inside a git repository" >&2
      return 1
    }
  }
  autoship_state_set() {
    local action="$1" issue_key="$2"
    shift 2
    local repo_root
    repo_root="$(autoship_repo_root)"
    bash "$repo_root/hooks/update-state.sh" "$action" "$issue_key" "$@"
  }
fi

REPO_ROOT=$(autoship_repo_root) || exit 1
cd "$REPO_ROOT"

AUTOSHIP_DIR=".autoship"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
MAX=3

# Find queued workspaces and start them
queued=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec grep -l "^QUEUED$" {} \; 2>/dev/null || true)
running=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec grep -l "^RUNNING$" {} \; 2>/dev/null || true)
running_count=$(echo "$running" | grep -c "^$WORKSPACES_DIR" || echo 0)

if [[ ! "$running_count" =~ ^[0-9]+$ ]]; then
  running_count=0
fi

available_slots=$((MAX - running_count))
if [[ "$available_slots" -le 0 ]]; then
  echo "Max concurrent reached: $running_count / $MAX"
  exit 0
fi

echo "Hermes runner: $running_count running, $available_slots slots available"

# Start up to available_slots queued workspaces
started=0
for status_file in $queued; do
  if [[ "$started" -ge "$available_slots" ]]; then
    break
  fi
  
  workspace_dir=$(dirname "$status_file")
  issue_key=$(basename "$workspace_dir")
  
  if [[ ! -f "$workspace_dir/HERMES_PROMPT.md" ]]; then
    continue
  fi
  
  # Mark as running
  printf 'RUNNING\n' > "$status_file"
  autoship_state_set set-running "$issue_key" agent="hermes/default"
  
  echo "Starting $issue_key in $workspace_dir"
  
  # Hermes execution methods:
  # Method 1: If inside Hermes session, use delegate_task (parallel subagents)
  # Method 2: If Hermes CLI available, spawn hermes chat in worktree
  # Method 3: Queue for cronjob pickup
  
  if [[ -n "${HERMES_SESSION_ID:-}" ]]; then
    # Inside Hermes — use delegate_task for parallel execution
    echo "Using delegate_task for parallel execution"
    # The parent Hermes agent will handle delegation
    # We just mark it running and trust the parent to dispatch
  elif command -v hermes &>/dev/null; then
    # Hermes CLI available — could spawn hermes chat
    echo "Hermes CLI available — manual dispatch required"
    echo "Run: cd $workspace_dir && hermes chat"
  else
    # Queue for cronjob
    echo "Queued for Hermes cronjob pickup"
  fi
  
  started=$((started + 1))
done

echo "Started $started Hermes workers"
