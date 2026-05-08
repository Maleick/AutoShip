#!/usr/bin/env bash
# Hermes setup runner - prepare workspaces for manual delegate_task dispatch
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

log_age_minutes() {
  python3 - "$1" <<'PY'
import os
import sys
import time

st = os.stat(sys.argv[1])
print(int((time.time() - st.st_mtime) / 60))
PY
}

AUTOSHIP_DIR="$REPO_ROOT/.autoship"
# Workspaces are always under the AutoShip orchestrator repo, not the target repo.
# The target repo worktree is checked out inside the workspace directory, but
# status files, prompts, and logs live in AutoShip's .autoship/workspaces/.
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"

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
    echo "Issue $ISSUE_KEY status=$current_status - not dispatchable"
    exit 0
  fi

  if [[ "$current_status" == "STUCK" ]]; then
    echo "Issue $ISSUE_KEY was STUCK - resetting to QUEUED for retry"
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
    # Fallback: search AutoShip workspace locations - include HERMES_TARGET_REPO_PATH workspaces
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

  cd "$worktree_path"
  export GH_TOKEN="${GH_TOKEN:-}"
  export HERMES_TARGET_REPO_PATH="${HERMES_TARGET_REPO_PATH:-$REPO_ROOT}"

  # WINDOWS BRIDGE: append bridge instructions to prompt file if needed
  WINDOWS_BRIDGE="${WINDOWS_BRIDGE_PATH:-$HOME/.hermes/scripts/windows_bridge.py}"
  if [[ -f "$WINDOWS_BRIDGE" && -f "$worktree_path/.cargo/config.toml" ]]; then
    if grep -q "x86_64-pc-windows-msvc" "$worktree_path/.cargo/config.toml" 2>/dev/null; then
      echo "Windows target detected - appending bridge instructions to prompt"
      bridge_instructions="

## WINDOWS BUILD INSTRUCTIONS
This repository requires Windows-native builds. When running cargo check or cargo test,
use the Windows bridge instead of direct invocation:
  python3 $WINDOWS_BRIDGE check
The bridge writes PowerShell scripts to Windows temp and executes via cmd.exe /c powershell.exe -File.
Do NOT run cargo directly in WSL - it will fail due to missing MSVC linker (lib.exe).
"
      echo "$bridge_instructions" >>"$prompt_file"
    fi
  fi

  # Mark workspace as running
  printf 'RUNNING\n' >"$workspace_dir/status"
  autoship_state_set set-running "$ISSUE_KEY" agent="hermes" model="delegate_task"

  # --- EXECUTE WORKER ---
  # If inside a Hermes session, use delegate_task directly.
  # Otherwise, fall back to hermes chat with the prompt file.
  WORKER_RESULT="BLOCKED"
  WORKER_REASON="no execution method available"

  if [[ -n "${HERMES_SESSION_ID:-}" ]]; then
    echo "Hermes session detected - executing via delegate_task..."
    # delegate_task is a Hermes tool; we cannot call it from bash.
    # Instead, write a ready marker and exit so the parent Hermes process
    # can poll for DELEGATE_TASK_READY workspaces and invoke delegate_task.
    printf 'DELEGATE_TASK_READY\n' >"$workspace_dir/status"
    echo "Workspace ready for delegate_task: $ISSUE_KEY"
    echo "Worktree: $worktree_path"
    echo "Prompt: $prompt_file"
    echo "Status: DELEGATE_TASK_READY"
    echo ""
    echo "Dispatch command:"
    echo "  delegate_task --workdir \"$worktree_path\" --toolsets '[\"terminal\",\"file\",\"web\"]' --prompt \"\$(cat $prompt_file)\" --timeout 600"
    exit 0
  fi

  # No Hermes session - try hermes chat CLI in headless mode
  if command -v hermes &>/dev/null; then
    echo "Executing worker via hermes chat (headless)..."
    printf 'RUNNING\n' >"$workspace_dir/status"

    # Use -q for single-query mode (reads prompt from file, non-interactive)
    # Use -Q for quiet mode (no TTY/spinner, banner suppressed)
    # Use --max-turns to prevent runaway sessions
    HERMES_TIMEOUT="${HERMES_WORKER_TIMEOUT:-600}"
    HERMES_MAX_TURNS="${HERMES_WORKER_MAX_TURNS:-90}"
    # Run hermes chat in the existing worktree directory.
    # Do NOT use --worktree - the workspace is already a git worktree.
    hermes_cmd=(hermes chat -q "$(cat "$prompt_file")" -Q --max-turns "$HERMES_MAX_TURNS" -t terminal,file,web)
    if command -v timeout >/dev/null 2>&1; then
      hermes_cmd=(timeout "$HERMES_TIMEOUT" "${hermes_cmd[@]}")
    elif command -v gtimeout >/dev/null 2>&1; then
      hermes_cmd=(gtimeout "$HERMES_TIMEOUT" "${hermes_cmd[@]}")
    fi
    "${hermes_cmd[@]}" >"$workspace_dir/hermes-worker.log" 2>&1
    worker_exit=$?

    if [[ $worker_exit -eq 0 ]]; then
      WORKER_RESULT="COMPLETE"
      WORKER_REASON="hermes chat completed successfully"
    elif [[ $worker_exit -eq 124 ]]; then
      WORKER_RESULT="STUCK"
      WORKER_REASON="hermes chat timed out (exit 124)"
    else
      WORKER_RESULT="BLOCKED"
      WORKER_REASON="hermes chat failed (exit $worker_exit)"
    fi
  fi

  # --- POST-EXECUTION: detect result files if worker wrote them ---
  if [[ -f "$workspace_dir/HERMES_RESULT.md" ]]; then
    result_status=$(head -n 20 "$workspace_dir/HERMES_RESULT.md" | grep -i "^## Status" | head -n1 | sed 's/.*://' | tr -d ' \r' || echo "")
    if [[ -n "$result_status" ]]; then
      WORKER_RESULT="$result_status"
      WORKER_REASON="HERMES_RESULT.md reports status: $result_status"
    fi
  elif [[ -f "$workspace_dir/AUTOSHIP_RESULT.md" ]]; then
    result_status=$(head -n 20 "$workspace_dir/AUTOSHIP_RESULT.md" | grep -i "^## Status" | head -n1 | sed 's/.*://' | tr -d ' \r' || echo "")
    if [[ -n "$result_status" ]]; then
      WORKER_RESULT="$result_status"
      WORKER_REASON="AUTOSHIP_RESULT.md reports status: $result_status"
    fi
  fi

  # Also check for git commits as evidence of work done
  if [[ "$WORKER_RESULT" != "COMPLETE" && "$WORKER_RESULT" != "BLOCKED" ]]; then
    commit_count=$(git -C "$worktree_path" rev-list --count autoship/issue-${ISSUE_NUM}...HEAD 2>/dev/null || echo 0)
    if [[ "$commit_count" -gt 0 ]]; then
      # Worker made commits but didn't finish workflow - mark STUCK for retry
      WORKER_RESULT="STUCK"
      WORKER_REASON="worker made $commit_count commit(s) but did not complete PR/status workflow"
    fi
  fi

  # --- FINALIZE STATUS ---
  printf '%s\n' "$WORKER_RESULT" >"$workspace_dir/status"

  if [[ "$WORKER_RESULT" == "COMPLETE" ]]; then
    autoship_state_set set-complete "$ISSUE_KEY"
  elif [[ "$WORKER_RESULT" == "BLOCKED" ]]; then
    autoship_state_set set-blocked "$ISSUE_KEY" reason="$WORKER_REASON"
  else
    autoship_state_set set-stuck "$ISSUE_KEY" reason="$WORKER_REASON"
  fi

  printf 'Worker finished: %s -> %s (%s)\n' "$ISSUE_KEY" "$WORKER_RESULT" "$WORKER_REASON"
  echo "Log: $workspace_dir/hermes-worker.log"
  exit 0
fi

# Batch mode: find and dispatch all queued workspaces
# Use tr to strip \r from CRLF line endings before grepping
queued=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^QUEUED$"' _ {} \; -print 2>/dev/null || true)
# Count running workers via PID files (more accurate than status file alone)
running_count=0
while IFS= read -r status_file; do
  if [[ -z "$status_file" ]]; then continue; fi
  workspace_dir=$(dirname "$status_file")
  pid_file="$workspace_dir/runner.pid"
  if [[ -f "$pid_file" ]]; then
    pid=$(cat "$pid_file" 2>/dev/null || echo "")
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      running_count=$((running_count + 1))
    fi
  fi
done <<<"$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^RUNNING$"' _ {} \; -print 2>/dev/null || true)"

# Fallback: if no PID files, count by status file alone
if [[ "$running_count" -eq 0 ]]; then
  running=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^RUNNING$"' _ {} \; -print 2>/dev/null || true)
  running_count=$(echo "$running" | grep -c "^$WORKSPACES_DIR" || echo 0)
fi

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
    log_age_min=$(log_age_minutes "$log_file" 2>/dev/null || echo "99999")
    if [[ "$log_age_min" =~ ^[0-9]+$ && "$log_age_min" -lt 30 ]]; then
      # Log is recent, but if there's NO active Hermes process, the log age
      # is just from a previous run that finished. Allow retry in that case.
      active_hermes=$(ps aux 2>/dev/null | grep -E "[h]ermes" | grep -F "$workspace_dir" || true)
      if [[ -n "$active_hermes" ]]; then
        retry_allowed=false
      fi
    fi
  fi
  # Check for an active runner via PID file
  if [[ "$retry_allowed" == true ]]; then
    pid_file="$workspace_dir/runner.pid"
    if [[ -f "$pid_file" ]]; then
      pid=$(cat "$pid_file" 2>/dev/null || echo "")
      if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
        retry_allowed=false
        # Update status to RUNNING since process is alive
        printf 'RUNNING\n' >"$status_file"
        printf 'Corrected STUCK to RUNNING for %s (PID %s is alive)\n' "$issue_key" "$pid"
      fi
    fi
  fi
  # Check for an active Hermes process in the workspace (process-based validation)
  if [[ "$retry_allowed" == true ]]; then
    # Look for any Hermes process that has this workspace as its CWD
    active_hermes=$(ps aux 2>/dev/null | grep -E "[h]ermes" | grep -F "$workspace_dir" || true)
    if [[ -n "$active_hermes" ]]; then
      # Process is alive but status is STUCK - this is a false-positive STUCK.
      # Reset to RUNNING so the runner doesn't keep retrying it.
      printf 'RUNNING\n' >"$status_file"
      printf 'Corrected STUCK to RUNNING for %s (active hermes process detected)\n' "$issue_key"
      # Skip QUEUED retry for this workspace since it already has a live worker
      retry_allowed=false
    fi
  fi
  # Also check if the runner.log was modified in the last 5 minutes - if so, a worker may still be starting up
  if [[ "$retry_allowed" == true && -f "$log_file" ]]; then
    log_age_min=$(log_age_minutes "$log_file" 2>/dev/null || echo "99999")
    if [[ "$log_age_min" =~ ^[0-9]+$ && "$log_age_min" -lt 5 ]]; then
      # Even if log is <5min old, if there's no active hermes process, the log
      # is from a previous run that finished. Allow retry in that case.
      active_hermes=$(ps aux 2>/dev/null | grep -E "[h]ermes" | grep -F "$workspace_dir" || true)
      if [[ -n "$active_hermes" ]]; then
        retry_allowed=false
      fi
    fi
  fi
  # NEW: If workspace has a COMPLETE result (AUTOSHIP_RESULT.md or HERMES_RESULT.md) but status is STUCK/QUEUED,
  # skip dispatch - it's a completed workspace that was incorrectly reset.
  if [[ "$retry_allowed" == true ]]; then
    if [[ -f "$workspace_dir/AUTOSHIP_RESULT.md" || -f "$workspace_dir/HERMES_RESULT.md" ]]; then
      result_header=$(head -n 5 "$workspace_dir/AUTOSHIP_RESULT.md" "$workspace_dir/HERMES_RESULT.md" 2>/dev/null | grep -i "^## Status" | head -n1 | tr -d '\r')
      if [[ "$result_header" == *"COMPLETE"* ]]; then
        printf 'COMPLETE\n' >"$status_file"
        printf 'Corrected QUEUED to COMPLETE for %s (result file shows COMPLETE)\n' "$issue_key"
        retry_allowed=false
      fi
    fi
  fi
  if [[ "$retry_allowed" == true ]]; then
    printf 'QUEUED\n' >"$status_file"
    stuck_reset=$((stuck_reset + 1))
    printf 'Retried STUCK to QUEUED for %s (no recent activity)\n' "$issue_key"
  fi
done <<<"$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^STUCK$"' _ {} \; -print 2>/dev/null || true)"

# Re-count queued after STUCK retry
if [[ "$stuck_reset" -gt 0 ]]; then
  queued=$(find "$WORKSPACES_DIR" -maxdepth 2 -name "status" -exec sh -c 'cat "$1" | tr -d "\r" | grep -q "^QUEUED$"' _ {} \; -print 2>/dev/null || true)
fi

available_slots=$((MAX - running_count))
if [[ "$available_slots" -le 0 ]]; then
  echo "Max concurrent reached: $running_count / $MAX"
  exit 0
fi

echo "Hermes runner: $running_count running, $available_slots slots available (max=$MAX), $stuck_reset stuck-to-queued retries"

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
  pid_file="$workspace_dir/runner.pid"

  # Prefer setsid (proper session detachment), fallback to nohup
  if command -v setsid &>/dev/null; then
    setsid bash "$0" "$issue_key" >"$log_file" 2>&1 &
    worker_pid=$!
  else
    # macOS fallback: use nohup + subshell + redirect to detach
    (nohup bash "$0" "$issue_key" >"$log_file" 2>&1 &) &
    worker_pid=$!
  fi

  # Record PID for status tracking
  echo "$worker_pid" >"$pid_file"
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) PID: $worker_pid" >>"$log_file"

  started=$((started + 1))
  echo "Dispatched $issue_key (prompt=$prompt_file, pid=$worker_pid)"
done <<<"$queued"

echo "Started $started Hermes workers"

# Do not block - let workers run in background
# The cron will call runner again to check progress via PID files

# Auto-cleanup completed worktrees after batch - ALWAYS run, not just when started>0
# This prevents terminal workspaces from accumulating and blocking queue replenishment
echo "Running worktree cleanup..."
bash "$SCRIPT_DIR/cleanup-worktrees.sh" --verbose || true

# Auto-prune if thresholds exceeded
echo "Checking auto-prune thresholds..."
bash "$SCRIPT_DIR/auto-prune.sh" || echo "Auto-prune triggered (thresholds exceeded)"
