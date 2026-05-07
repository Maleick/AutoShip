#!/usr/bin/env bash
# Hermes agent runner — execute Hermes workers via one-shot cronjobs
set -uo pipefail

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

# Add util-linux bin to PATH for setsid on macOS
if [[ -d "/opt/homebrew/opt/util-linux/bin" ]]; then
  export PATH="/opt/homebrew/opt/util-linux/bin:$PATH"
fi

REPO_ROOT=$(autoship_repo_root) || exit 1
cd "$REPO_ROOT"

AUTOSHIP_DIR="$REPO_ROOT/.autoship"
# Allow overriding workspaces directory via HERMES_TARGET_REPO_PATH
# Default to TextQuest repo if AutoShip is the orchestrator (detect by repo name)
if [[ -n "${HERMES_TARGET_REPO_PATH:-}" && -d "$HERMES_TARGET_REPO_PATH/.autoship/workspaces" ]]; then
  WORKSPACES_DIR="$HERMES_TARGET_REPO_PATH/.autoship/workspaces"
elif [[ "$REPO_ROOT" == *"/AutoShip" ]] && [[ -d "/mnt/c/Users/xmale/Projects/TextQuest/.autoship/workspaces" ]]; then
  # AutoShip orchestrating TextQuest — use TextQuest workspaces
  WORKSPACES_DIR="/mnt/c/Users/xmale/Projects/TextQuest/.autoship/workspaces"
else
  WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
fi

# Read Hermes max concurrent from config.yaml; allow AutoShip runs to cap lower.
MAX="${HERMES_MAX_WORKERS:-20}"
if [[ -z "${HERMES_MAX_WORKERS:-}" && -f "$HOME/.hermes/config.yaml" ]]; then
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

  current_status=$(cat "$status_file" 2>/dev/null | tr -d '\r\n' || echo "unknown")
  if [[ "$current_status" == "COMPLETE" || "$current_status" == "BLOCKED" ]]; then
    echo "Issue $ISSUE_KEY status=$current_status — not dispatchable"
    exit 0
  fi

  if [[ "$current_status" == "STUCK" ]]; then
    echo "Issue $ISSUE_KEY was STUCK — resetting to QUEUED for retry"
    printf 'QUEUED\n' >"$status_file"
    current_status="QUEUED"
  fi

  # Mark running
  printf 'RUNNING\n' >"$status_file"
  autoship_state_set set-running "$ISSUE_KEY" agent="hermes/default"

  # Extract issue number from key
  ISSUE_NUM=$(echo "$ISSUE_KEY" | sed 's/issue-//')

  # Find the worktree path
  worktree_path=""
  HERMES_TARGET_REPO_PATH="${HERMES_TARGET_REPO_PATH:-$REPO_ROOT}"
  if [[ -n "$HERMES_TARGET_REPO_PATH" ]]; then
    worktree_path=$(git -C "$HERMES_TARGET_REPO_PATH" worktree list --porcelain 2>/dev/null | grep -B1 "branch refs/heads/autoship/issue-${ISSUE_NUM}$" | grep "^worktree " | awk '{print $2}' || echo "")
  fi
  if [[ -z "$worktree_path" || ! -d "$worktree_path" ]]; then
    # Fallback: search AutoShip workspace locations — include HERMES_TARGET_REPO_PATH workspaces
    for base in "$REPO_ROOT/.autoship/workspaces" "$REPO_ROOT/.worktrees" "$HOME/Projects/AutoShip/.autoship/workspaces" "$HERMES_TARGET_REPO_PATH/.autoship/workspaces"; do
      if [[ -d "$base/issue-$ISSUE_NUM" ]]; then
        worktree_path="$base/issue-$ISSUE_NUM"
        break
      fi
    done
  fi

  # Determine prompt file (HERMES_PROMPT.md or AUTOSHIP_PROMPT.md)
  prompt_file=""
  if [[ -f "$worktree_path/HERMES_PROMPT.md" ]]; then
    prompt_file="$worktree_path/HERMES_PROMPT.md"
  elif [[ -f "$worktree_path/AUTOSHIP_PROMPT.md" ]]; then
    prompt_file="$worktree_path/AUTOSHIP_PROMPT.md"
  fi

  if [[ -z "$worktree_path" || ! -d "$worktree_path" ]]; then
    echo "Error: worktree not found for issue-$ISSUE_NUM" >&2
    printf 'BLOCKED\n' >"$status_file"
    autoship_state_set set-blocked "$ISSUE_KEY" reason="worktree not found"
    exit 1
  fi

  if [[ -z "$prompt_file" ]]; then
    echo "Error: no prompt file (HERMES_PROMPT.md or AUTOSHIP_PROMPT.md) found for $ISSUE_KEY" >&2
    printf 'BLOCKED\n' >"$status_file"
    autoship_state_set set-blocked "$ISSUE_KEY" reason="no_prompt_file"
    exit 1
  fi

  echo "Dispatching $ISSUE_KEY in $worktree_path"

  # Execute via Hermes cronjob so workers get full tool access.
  if command -v hermes &>/dev/null; then
    cd "$worktree_path"
    export GH_TOKEN="${GH_TOKEN:-}"
    export HERMES_TARGET_REPO_PATH="${HERMES_TARGET_REPO_PATH:-$REPO_ROOT}"
    HERMES_MODEL_ARGS=()
    if [[ -n "${HERMES_MODEL:-}" ]]; then
      HERMES_MODEL_ARGS+=(--model "$HERMES_MODEL")
    fi
    if [[ -n "${HERMES_PROVIDER:-}" ]]; then
      HERMES_MODEL_ARGS+=(--provider "$HERMES_PROVIDER")
    fi

    # WINDOWS BRIDGE: For repos that require Windows-native builds (MSVC),
    # use the windows_bridge.py script instead of direct WSL validation.
    # The bridge writes .ps1 scripts to Windows temp and executes via powershell -File.
    WINDOWS_BRIDGE="${WINDOWS_BRIDGE_PATH:-$HOME/.hermes/scripts/windows_bridge.py}"
    if [[ -f "$WINDOWS_BRIDGE" && -f "$worktree_path/.cargo/config.toml" ]]; then
      # Detect Windows-targeted cargo config
      if grep -q "x86_64-pc-windows-msvc" "$worktree_path/.cargo/config.toml" 2>/dev/null; then
        echo "Windows target detected — using bridge: $WINDOWS_BRIDGE"
        # The bridge runs cargo check on Windows host; the worker prompt tells
        # Hermes to use it for validation when needed.
        # Prepend bridge instructions to the prompt.
        bridge_instructions="

## WINDOWS BUILD INSTRUCTIONS
This repository requires Windows-native builds. When running cargo check or cargo test,
use the Windows bridge instead of direct invocation:
  python3 $WINDOWS_BRIDGE check
The bridge writes PowerShell scripts to Windows temp and executes via cmd.exe /c powershell.exe -File.
Do NOT run cargo directly in WSL — it will fail due to missing MSVC linker (lib.exe).
"
        # Append bridge instructions to prompt file temporarily
        cp "$prompt_file" "$workspace_dir/prompt.bak"
        echo "$bridge_instructions" >>"$prompt_file"
      fi
    fi

    job_name="autoship-${ISSUE_KEY}-$(date +%s)"
    deliver_target="${HERMES_DELIVER_TARGET:-origin}"
    hermes cron create \
      --name "$job_name" \
      --workdir "$worktree_path" \
      --deliver "$deliver_target" \
      --repeat 1 \
      "1m" \
      "$(cat "$prompt_file")" \
      "${HERMES_MODEL_ARGS[@]}"
    exit_code=$?
    # Restore original prompt if we modified it
    if [[ -f "$workspace_dir/prompt.bak" ]]; then
      mv "$workspace_dir/prompt.bak" "$prompt_file"
    fi

    if [[ $exit_code -ne 0 ]]; then
      echo "ERROR: $ISSUE_KEY cron creation exited with code $exit_code"
      printf 'BLOCKED\n' >"$workspace_dir/status"
      autoship_state_set set-blocked "$ISSUE_KEY" reason="cron_create_exit_$exit_code"
      exit 0
    fi

    echo "Created Hermes cronjob $job_name for $ISSUE_KEY"
    exit 0

  else
    echo "Hermes not available — cannot execute"
    printf 'BLOCKED\n' >"$status_file"
    autoship_state_set set-blocked "$ISSUE_KEY" reason="hermes_unavailable"
  fi

  exit 0
fi

# Batch mode: find and dispatch all queued workspaces
# Use tr to strip \r from CRLF line endings before grepping
queued=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^QUEUED$"' _ {} \; -print 2>/dev/null || true)
running=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^RUNNING$"' _ {} \; -print 2>/dev/null || true)
running_count=$(echo "$running" | grep -c "^$WORKSPACES_DIR" || echo 0)

if [[ ! "$running_count" =~ ^[0-9]+$ ]]; then
  running_count=0
fi

# --- STUCK retry logic: reset stale STUCK workspaces to QUEUED ---
stuck_reset=0
while IFS= read -r status_file; do
  if [[ -z "$status_file" ]]; then continue; fi
  workspace_dir=$(dirname "$status_file")
  issue_key=$(basename "$workspace_dir")
  # Only retry STUCK workspaces that have no active runner process
  # and no runner.log newer than 30 minutes
  log_file="$workspace_dir/runner.log"
  retry_allowed=true
  if [[ -f "$log_file" ]]; then
    # If log exists and was modified in the last 30 minutes, do NOT retry.
    # WSL 9pfs find -mmin is broken (always returns true for -N), so use
    # Python stat to compute actual age in minutes.
    log_age_min=$(python3 -c "import os,time; st=os.stat('$log_file'); print(int((time.time()-st.st_mtime)/60))" 2>/dev/null || echo "99999")
    if [[ "$log_age_min" =~ ^[0-9]+$ && "$log_age_min" -lt 30 ]]; then
      # Log is recent, but if there's NO active hermes process, the log age
      # is just from a previous run that finished. Allow retry in that case.
      active_hermes=$(ps aux 2>/dev/null | grep -E "[h]ermes chat" | grep -F "$workspace_dir" || true)
      if [[ -n "$active_hermes" ]]; then
        retry_allowed=false
      fi
    fi
  fi
  # Also check for an active process via runner.log PID if available
  if [[ -f "$log_file" && "$retry_allowed" == true ]]; then
    # Look for a PID line in the log (e.g., "PID: 12345") and verify it's still running
    pid=$(grep -m1 -oP 'PID:\s*\K[0-9]+' "$log_file" 2>/dev/null || true)
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      retry_allowed=false
    fi
  fi
  # Check for an active hermes process in the workspace (process-based validation)
  if [[ "$retry_allowed" == true ]]; then
    # Look for any hermes chat process that has this workspace as its CWD
    active_hermes=$(ps aux 2>/dev/null | grep -E "[h]ermes chat" | grep -F "$workspace_dir" || true)
    if [[ -n "$active_hermes" ]]; then
      # Process is alive but status is STUCK — this is a false-positive STUCK.
      # Reset to RUNNING so the runner doesn't keep retrying it.
      printf 'RUNNING\n' >"$status_file"
      echo "Corrected STUCK→RUNNING $issue_key (active hermes process detected)"
      # Skip QUEUED retry for this workspace since it already has a live worker
      retry_allowed=false
    fi
  fi
  # Also check if the runner.log was modified in the last 5 minutes — if so, a worker may still be starting up
  if [[ "$retry_allowed" == true && -f "$log_file" ]]; then
    log_age_min=$(python3 -c "import os,time; st=os.stat('$log_file'); print(int((time.time()-st.st_mtime)/60))" 2>/dev/null || echo "99999")
    if [[ "$log_age_min" =~ ^[0-9]+$ && "$log_age_min" -lt 5 ]]; then
      # Even if log is <5min old, if there's no active hermes process, the log
      # is from a previous run that finished. Allow retry in that case.
      active_hermes=$(ps aux 2>/dev/null | grep -E "[h]ermes chat" | grep -F "$workspace_dir" || true)
      if [[ -n "$active_hermes" ]]; then
        retry_allowed=false
      fi
    fi
  fi
  # NEW: If workspace has a COMPLETE result (AUTOSHIP_RESULT.md or HERMES_RESULT.md) but status is STUCK/QUEUED,
  # skip dispatch — it's a completed workspace that was incorrectly reset.
  if [[ "$retry_allowed" == true ]]; then
    if [[ -f "$workspace_dir/AUTOSHIP_RESULT.md" || -f "$workspace_dir/HERMES_RESULT.md" ]]; then
      result_header=$(head -n 5 "$workspace_dir/AUTOSHIP_RESULT.md" "$workspace_dir/HERMES_RESULT.md" 2>/dev/null | grep -i "^## Status" | head -n1 | tr -d '\r')
      if [[ "$result_header" == *"COMPLETE"* ]]; then
        printf 'COMPLETE\n' >"$status_file"
        echo "Corrected QUEUED→COMPLETE $issue_key (result file shows COMPLETE)"
        retry_allowed=false
      fi
    fi
  fi
  if [[ "$retry_allowed" == true ]]; then
    printf 'QUEUED\n' >"$status_file"
    stuck_reset=$((stuck_reset + 1))
    echo "Retried STUCK→QUEUED $issue_key (no recent activity)"
  fi
done <<< "$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^STUCK$"' _ {} \; -print 2>/dev/null || true)"

# Re-count queued after STUCK retry
if [[ "$stuck_reset" -gt 0 ]]; then
  queued=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^QUEUED$"' _ {} \; -print 2>/dev/null || true)
fi

available_slots=$((MAX - running_count))
if [[ "$available_slots" -le 0 ]]; then
  echo "Max concurrent reached: $running_count / $MAX"
  exit 0
fi

echo "Hermes runner: $running_count running, $available_slots slots available (max=$MAX), $stuck_reset stuck→queued retries"

# Start up to available_slots queued workspaces
started=0
while IFS= read -r status_file; do
  if [[ -z "$status_file" ]]; then
    continue
  fi
  if [[ "$started" -ge "$available_slots" ]]; then
    break
  fi

  workspace_dir=$(dirname "$status_file")
  issue_key=$(basename "$workspace_dir")

  if [[ -f "$workspace_dir/HERMES_PROMPT.md" ]]; then
    prompt_file="$workspace_dir/HERMES_PROMPT.md"
  elif [[ -f "$workspace_dir/AUTOSHIP_PROMPT.md" ]]; then
    prompt_file="$workspace_dir/AUTOSHIP_PROMPT.md"
  else
    # No prompt file — skip dispatch but log why
    echo "Skipping $issue_key: no prompt file (HERMES_PROMPT.md or AUTOSHIP_PROMPT.md)"
    continue
  fi

  # Check if this is a Windows-target repo (has .cargo/config.toml with x86_64-pc-windows-msvc)
  is_windows_repo=false
  if [[ -f "$workspace_dir/.cargo/config.toml" ]]; then
    if grep -q "x86_64-pc-windows-msvc" "$workspace_dir/.cargo/config.toml" 2>/dev/null; then
      is_windows_repo=true
    fi
  fi

  # --- Prompt file discovery: look in workspace_dir and in the actual git worktree ---
  prompt_file=""
  if [[ -f "$workspace_dir/HERMES_PROMPT.md" ]]; then
    prompt_file="$workspace_dir/HERMES_PROMPT.md"
  elif [[ -f "$workspace_dir/AUTOSHIP_PROMPT.md" ]]; then
    prompt_file="$workspace_dir/AUTOSHIP_PROMPT.md"
  fi
  # If no prompt in workspace_dir, resolve the real git worktree path for this issue
  # and look there.  The workspace_dir may be a bare status container (issue-3059)
  # or a full worktree checkout (issue-3057).
  if [[ -z "$prompt_file" ]]; then
    ISSUE_NUM=$(echo "$issue_key" | sed 's/issue-//')
    worktree_path=""
    HERMES_TARGET_REPO_PATH="${HERMES_TARGET_REPO_PATH:-$REPO_ROOT}"
    if [[ -n "$HERMES_TARGET_REPO_PATH" ]]; then
      worktree_path=$(git -C "$HERMES_TARGET_REPO_PATH" worktree list --porcelain 2>/dev/null | grep -B1 "autoship/issue-${ISSUE_NUM}$" | grep "^worktree " | awk '{print $2}' || echo "")
    fi
    if [[ -z "$worktree_path" || ! -d "$worktree_path" ]]; then
      for base in "$REPO_ROOT/.autoship/workspaces" "$REPO_ROOT/.worktrees" "$HOME/Projects/AutoShip/.autoship/workspaces" "$HERMES_TARGET_REPO_PATH/.autoship/workspaces"; do
        if [[ -d "$base/issue-$ISSUE_NUM" ]]; then
          worktree_path="$base/issue-$ISSUE_NUM"
          break
        fi
      done
    fi
    if [[ -n "$worktree_path" && -d "$worktree_path" ]]; then
      if [[ -f "$worktree_path/HERMES_PROMPT.md" ]]; then
        prompt_file="$worktree_path/HERMES_PROMPT.md"
      elif [[ -f "$worktree_path/AUTOSHIP_PROMPT.md" ]]; then
        prompt_file="$worktree_path/AUTOSHIP_PROMPT.md"
      fi
    fi
  fi

  # Fallback: if the workspace_dir itself is a full worktree (has subdirs like textquest/),
  # the prompt may have been consumed or renamed.  Check for any *PROMPT*.md file.
  if [[ -z "$prompt_file" ]]; then
    prompt_file=$(find "$workspace_dir" -maxdepth 1 -type f \( -name "*PROMPT*.md" -o -name "*prompt*.md" \) 2>/dev/null | head -n1)
  fi

  if [[ -z "$prompt_file" ]]; then
    echo "Skipping $issue_key: no prompt file (HERMES_PROMPT.md or AUTOSHIP_PROMPT.md)"
    continue
  fi

  # Mark as RUNNING before dispatch
  printf 'RUNNING\n' >"$status_file"

  # Dispatch this single issue, detached from terminal
  # Log to workspace log file for debugging
  log_file="$workspace_dir/runner.log"
  # Prefer setsid (proper session detachment), fallback to nohup
  if command -v setsid &>/dev/null; then
    setsid bash "$0" "$issue_key" >"$log_file" 2>&1 &
  else
    # macOS fallback: use nohup + subshell + redirect to detach
    (nohup bash "$0" "$issue_key" >"$log_file" 2>&1 &) &
  fi

  started=$((started + 1))
  echo "Dispatched $issue_key (prompt=$prompt_file)"
done <<<"$queued"

echo "Started $started Hermes workers"

# Don't wait — let workers run in background
# The cron will call runner again to check progress

# Auto-cleanup completed worktrees after batch — ALWAYS run, not just when started>0
# This prevents terminal workspaces from accumulating and blocking queue replenishment
echo "Running worktree cleanup..."
bash "$SCRIPT_DIR/cleanup-worktrees.sh" --verbose || true

# Auto-prune if thresholds exceeded
echo "Checking auto-prune thresholds..."
bash "$SCRIPT_DIR/auto-prune.sh" || echo "Auto-prune triggered (thresholds exceeded)"
