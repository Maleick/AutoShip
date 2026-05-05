#!/usr/bin/env bash
# Hermes agent runner — execute Hermes worker via delegate_task with timeout
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

# Read Hermes max concurrent from config.yaml
MAX=3
if [[ -f "$HOME/.hermes/config.yaml" ]]; then
  config_max=$(grep 'max_concurrent_children' "$HOME/.hermes/config.yaml" | awk '{print $2}' | tr -d '"')
  if [[ "$config_max" =~ ^[0-9]+$ ]]; then
    MAX="$config_max"
  fi
fi

# Single-issue mode: runner.sh <issue_key>
if [[ -n "${1:-}" ]]; then
  ISSUE_KEY="$1"
  workspace_dir="$WORKSPACES_DIR/$ISSUE_KEY"
  status_file="$workspace_dir/status"
  
  if [[ ! -f "$status_file" ]]; then
    echo "Error: workspace not found for $ISSUE_KEY" >&2
    exit 1
  fi
  
  current_status=$(cat "$status_file" 2>/dev/null || echo "unknown")
  if [[ "$current_status" == "COMPLETE" || "$current_status" == "BLOCKED" ]]; then
    echo "Issue $ISSUE_KEY status=$current_status — not dispatchable"
    exit 0
  fi
  
  # Mark running
  printf 'RUNNING\n' > "$status_file"
  autoship_state_set set-running "$ISSUE_KEY" agent="hermes/default"
  
  # Extract issue number from key
  ISSUE_NUM=$(echo "$ISSUE_KEY" | sed 's/issue-//')
  
  # Find the worktree path
  worktree_path=""
  HERMES_TARGET_REPO_PATH="${HERMES_TARGET_REPO_PATH:-$REPO_ROOT}"
  if [[ -n "$HERMES_TARGET_REPO_PATH" ]]; then
    worktree_path=$(git -C "$HERMES_TARGET_REPO_PATH" worktree list --porcelain 2>/dev/null | grep -B1 "autoship/issue-${ISSUE_NUM}$" | grep "^worktree " | awk '{print $2}' || echo "")
  fi
  if [[ -z "$worktree_path" || ! -d "$worktree_path" ]]; then
    # Fallback: search AutoShip workspace locations
    for base in "$REPO_ROOT/.autoship/workspaces" "$REPO_ROOT/.worktrees" "$HOME/Projects/AutoShip/.autoship/workspaces"; do
      if [[ -d "$base/issue-$ISSUE_NUM" ]]; then
        worktree_path="$base/issue-$ISSUE_NUM"
        break
      fi
    done
  fi
  
  if [[ -z "$worktree_path" || ! -d "$worktree_path" ]]; then
    echo "Error: worktree not found for issue-$ISSUE_NUM" >&2
    printf 'BLOCKED\n' > "$status_file"
    autoship_state_set set-blocked "$ISSUE_KEY" reason="worktree not found"
    exit 1
  fi

  echo "Dispatching $ISSUE_KEY in $worktree_path"
  
  # Execute via delegate_task if inside Hermes session, else hermes chat
  if [[ -n "${HERMES_SESSION_ID:-}" ]]; then
    # Inside Hermes — use delegate_task for parallel execution
    echo "Inside Hermes session — using delegate_task..."

    # Read the prompt content
    prompt_content=$(cat "$workspace_dir/HERMES_PROMPT.md" 2>/dev/null || echo "Complete issue #$ISSUE_NUM in $worktree_path")

    # delegate_task is available in this Hermes session
    # The parent agent will receive the task and execute it
    echo "DELEGATE_TASK_READY: $ISSUE_KEY"
    echo "Worktree: $worktree_path"
    echo "Prompt: $workspace_dir/HERMES_PROMPT.md"

    # Write a marker file that the parent can detect
    printf 'DELEGATED\n' > "$status_file"

    # The parent Hermes agent should:
    # 1. Detect DELEGATED status
    # 2. Read HERMES_PROMPT.md
    # 3. Call delegate_task with the prompt as goal
    # 4. Update status to COMPLETE/BLOCKED/STUCK based on result

    echo "Parent agent should now call delegate_task for $ISSUE_KEY"

  elif command -v hermes &>/dev/null; then
    # Hermes CLI available — spawn hermes chat
    cd "$worktree_path"
    # Timeout: 10 minutes (600 seconds) for atomic work
    timeout 600 hermes chat --prompt "$workspace_dir/HERMES_PROMPT.md" --workdir "$worktree_path" || {
      exit_code=$?
      if [[ $exit_code -eq 124 ]]; then
        echo "TIMEOUT: $ISSUE_KEY exceeded 10 minutes"
        printf 'STUCK\n' > "$status_file"
        autoship_state_set set-stuck "$ISSUE_KEY" reason="timeout_10min"
      else
        echo "ERROR: $ISSUE_KEY exited with code $exit_code"
        printf 'BLOCKED\n' > "$status_file"
        autoship_state_set set-blocked "$ISSUE_KEY" reason="exit_code_$exit_code"
      fi
      exit 0
    }
    
    # Check result
    result_status=$(cat "$status_file" 2>/dev/null || echo "unknown")
    echo "Result: $ISSUE_KEY = $result_status"
    
    if [[ "$result_status" == "COMPLETE" ]]; then
      autoship_state_set set-complete "$ISSUE_KEY"
      # Trigger PR creation
      bash "$SCRIPT_DIR/../opencode/create-pr.sh" "$ISSUE_NUM" "$worktree_path"
    elif [[ "$result_status" == "BLOCKED" ]]; then
      autoship_state_set set-blocked "$ISSUE_KEY"
    elif [[ "$result_status" == "STUCK" ]]; then
      autoship_state_set set-stuck "$ISSUE_KEY"
    fi
  else
    echo "Hermes not available — cannot execute"
    printf 'BLOCKED\n' > "$status_file"
    autoship_state_set set-blocked "$ISSUE_KEY" reason="hermes_unavailable"
  fi
  
  exit 0
fi

# Batch mode: find and dispatch all queued workspaces
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

echo "Hermes runner: $running_count running, $available_slots slots available (max=$MAX)"

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
  
  # Dispatch this single issue
  bash "$0" "$issue_key" &
  
  started=$((started + 1))
done

wait
echo "Started $started Hermes workers"

  # Auto-cleanup completed worktrees after batch
  if [[ "$started" -gt  0 ]]; then
    echo "Running worktree cleanup..."
    bash "$SCRIPT_DIR/cleanup-worktrees.sh" --verbose
    
    # Auto-prune if thresholds exceeded
    echo "Checking auto-prune thresholds..."
    bash "$SCRIPT_DIR/auto-prune.sh" || echo "Auto-prune triggered (thresholds exceeded)"
  fi
