#!/usr/bin/env bash
# monitor-agents.sh — Watch .beacon/workspaces/*/pane.log for agent status words.
# Emits: [AGENT_STATUS] key=<issue-key> status=<COMPLETE|BLOCKED|STUCK>
# Run via Monitor tool for real-time agent completion detection (5s response).

BEACON_DIR=".beacon"
WORKSPACE_DIR="$BEACON_DIR/workspaces"

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

# Also check pane_dead as a fallback for third-party agents that don't emit
# status words (crash detection). — Error Recovery #4: Tool crash mid-dispatch
check_dead_panes() {
  if ! command -v tmux >/dev/null 2>&1; then
    return
  fi
  tmux list-panes -t beacon -F '#{pane_id} #{pane_dead} #{pane_title}' 2>/dev/null | \
    while IFS=' ' read -r pane_id dead title; do
      if [[ "$dead" == "1" ]]; then
        # Parse issue key from pane title (format: "TOOL: issue-key")
        key=$(echo "$title" | sed -E 's/^[^:]+: //')
        if [[ -n "$key" && -d "$WORKSPACE_DIR/$key" ]]; then
          if [[ -f "$WORKSPACE_DIR/$key/BEACON_RESULT.md" ]]; then
            echo "[AGENT_DONE_FALLBACK] key=$key pane=$pane_id"
          else
            # Crash: no result file. Mark as failed and queue for re-dispatch.
            echo "[AGENT_CRASH] key=$key pane=$pane_id"

            # Extract numeric issue ID (e.g. "issue-42" → "42")
            issue_num="${key#issue-}"

            # Update state to failed
            bash hooks/update-state.sh set-failed "$issue_num" 2>/dev/null || true

            # Write crash event to event queue (priority 1 = urgent)
            EVENT_QUEUE="$BEACON_DIR/event-queue.json"
            if [[ ! -f "$EVENT_QUEUE" ]]; then
              echo '[]' > "$EVENT_QUEUE"
            fi
            CRASH_EVENT="{\"type\": \"agent_crashed\", \"issue\": $issue_num, \"pane\": \"$pane_id\", \"priority\": 1}"
            flock "${EVENT_QUEUE}.lock" \
              jq --argjson evt "$CRASH_EVENT" '. + [$evt]' "$EVENT_QUEUE" > "${EVENT_QUEUE}.tmp" \
              && mv "${EVENT_QUEUE}.tmp" "$EVENT_QUEUE" 2>/dev/null || true

            # Log to poll.log
            echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] monitor-agents: CRASH detected key=$key pane=$pane_id — marked failed, queued agent_crashed" \
              >> "$BEACON_DIR/poll.log" 2>/dev/null || true
          fi
        fi
      fi
    done
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

      # Skip if already watching this log
      if [[ -f "$pid_file" ]]; then
        pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
          continue
        fi
      fi

      # Start a tail watcher for this log; exits after the first status word.
      (
        trap 'rm -f "$pid_file"' EXIT
        tail -f "$logfile" 2>/dev/null | grep --line-buffered -E "^(COMPLETE|BLOCKED|STUCK)$" | \
          while read -r status; do
            echo "[AGENT_STATUS] key=$key status=$status"
            break
          done
      ) &
      echo $! > "$pid_file"
    done

    # Fallback dead-pane check every 15s
    check_dead_panes

    sleep 5
  done
}

watch_logs
