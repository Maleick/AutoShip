#!/usr/bin/env bash
# monitor-agents.sh — Watch .autoship/workspaces/*/pane.log for agent status words.
# Emits: [AGENT_STATUS] key=<issue-key> status=<COMPLETE|BLOCKED|STUCK>
# Run via Monitor tool for real-time agent completion detection (5s response).

AUTOSHIP_DIR=".autoship"
WORKSPACE_DIR="$AUTOSHIP_DIR/workspaces"
STATE_FILE="$AUTOSHIP_DIR/state.json"

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

# Check for stalled agents (no output for >60 minutes)
check_stalled_agents() {
  local now=$(date +%s)
  for workspace in "$WORKSPACE_DIR"/*; do
    [[ -d "$workspace" ]] || continue
    key=$(basename "$workspace")
    logfile="$workspace/pane.log"

    if [[ -f "$logfile" ]]; then
      # Get last line timestamp
      last_output=$(tail -1 "$logfile" 2>/dev/null)
      if [[ -n "$last_output" ]]; then
        # Extract timestamp if logged, otherwise use file mtime
        last_mtime=$(stat -f%m "$logfile" 2>/dev/null || stat -c%Y "$logfile" 2>/dev/null || echo 0)
        elapsed=$(( now - last_mtime ))

        # If >3600 seconds (60 minutes) with no output, mark as stalled
        if (( elapsed > 3600 )); then
          echo "[AGENT_STALLED] key=$key elapsed_seconds=$elapsed" >> .autoship/poll.log
          echo "WARN: Agent $key stalled (no output for ${elapsed}s, limit=3600s)" >&2
        fi
      fi
    fi
  done
}

# Also check pane_dead as a fallback for third-party agents that don't emit
# status words (crash detection). — Error Recovery #4: Tool crash mid-dispatch
check_dead_panes() {
  if ! command -v tmux >/dev/null 2>&1; then
    return
  fi
  tmux list-panes -t autoship -F '#{pane_id} #{pane_dead} #{pane_title}' 2>/dev/null | \
    while IFS=' ' read -r pane_id dead title; do
      if [[ "$dead" == "1" ]]; then
        # Parse issue key from pane title (format: "TOOL: issue-key")
        key=$(echo "$title" | sed -E 's/^[^:]+: //')
        if [[ -n "$key" && -d "$WORKSPACE_DIR/$key" ]]; then
          if [[ -f "$WORKSPACE_DIR/$key/AUTOSHIP_RESULT.md" ]]; then
            echo "[AGENT_DONE_FALLBACK] key=$key pane=$pane_id"
          else
            # Crash: no result file. Mark as failed and queue for re-dispatch.
            echo "[AGENT_CRASH] key=$key pane=$pane_id"

            # Extract numeric issue ID (e.g. "issue-42" → "42")
            issue_num="${key#issue-}"

            # Update state to failed
            bash hooks/update-state.sh set-failed "$issue_num" 2>/dev/null || true

            # Write crash event to event queue (priority 1 = urgent)
            EVENT_QUEUE="$AUTOSHIP_DIR/event-queue.json"
            if [[ ! -f "$EVENT_QUEUE" ]]; then
              echo '[]' > "$EVENT_QUEUE"
            fi
            CRASH_EVENT=$(jq -n \
              --argjson num "$issue_num" \
              --arg pane "$pane_id" \
              '{"type":"agent_crashed","issue":$num,"pane":$pane,"priority":1}')
            bash hooks/emit-event.sh "$CRASH_EVENT" 2>/dev/null || true

            # Log to poll.log
            echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] monitor-agents: CRASH detected key=$key pane=$pane_id — marked failed, queued agent_crashed" \
              >> "$AUTOSHIP_DIR/poll.log" 2>/dev/null || true
          fi
        fi
      fi
    done
}

# Return true (0) if the issue with the given key has worktree_free=true in state.json.
# These issues are dispatched without a worktree (e.g. claude-haiku/claude-sonnet subagents)
# and their completion is detected by monitor-prs.sh when the PR merges — not by watching
# a pane.log that doesn't exist.
is_worktree_free() {
  local key="$1"
  local issue_num="${key#issue-}"
  if [[ -f "$STATE_FILE" ]] && command -v jq >/dev/null 2>&1; then
    local flag
    flag=$(jq -r --arg k "$issue_num" '.issues[$k].worktree_free // false' "$STATE_FILE" 2>/dev/null)
    [[ "$flag" == "true" ]]
  else
    return 1
  fi
}

# Watch all existing pane.log files. Also periodically scan for new ones.
watch_logs() {
  local pids=()

  while true; do
    # Find all pane.log files not currently being watched
    for logfile in "$WORKSPACE_DIR"/*/pane.log; do
      [[ -f "$logfile" ]] || continue
      key=$(basename "$(dirname "$logfile")")
      pid_file="$WORKSPACE_DIR/$key/.watcher.pid"

      # Skip worktree_free issues — their completion is handled by monitor-prs.sh
      if is_worktree_free "$key"; then
        continue
      fi

      # Skip if already watching this log
      if [[ -f "$pid_file" ]]; then
        pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
          continue
        fi
      fi

      # Start a tail watcher for this log; exits after the first status word.
      # Uses watermark detection to ignore stale entries from previous dispatch attempts.
      (
        trap 'rm -f "$pid_file"' EXIT
        LAST_WATERMARK=""
        tail -f "$logfile" 2>/dev/null | while IFS= read -r line; do
          # Detect dispatch attempt watermark (indicates log was truncated)
          if [[ "$line" =~ ^===\ Dispatch\ attempt\ \#[0-9]+\ ===$ ]]; then
            LAST_WATERMARK="$line"
            continue  # Skip watermark line itself
          fi
          # Only process status words from current dispatch attempt
          if [[ "$line" =~ ^(COMPLETE|BLOCKED|STUCK)$ ]]; then
            status="$line"
            # CRITICAL: Validate .result_verified sentinel exists for COMPLETE status
            if [[ "$status" == "COMPLETE" ]]; then
              if [[ ! -f "$WORKSPACE_DIR/$key/.result_verified" ]]; then
                echo "[AGENT_STATUS_INVALID] key=$key status=$status reason=missing_result_verified" >> .autoship/poll.log
                echo "ERROR: Agent $key reported COMPLETE but .result_verified sentinel missing — file write may not have completed" >> .autoship/poll.log
                continue  # Skip this status, keep watching
              fi
            fi
            echo "[AGENT_STATUS] key=$key status=$status watermark=$LAST_WATERMARK"
            break
          fi
        done
      ) &
      echo $! > "$pid_file"
    done

    # Fallback checks every 15s
    check_dead_panes
    check_stalled_agents

    sleep 5
  done
}

watch_logs
