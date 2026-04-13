#!/usr/bin/env bash
# dispatch-codex-appserver.sh — Drive codex app-server via JSON-RPC (no tmux)
# Usage: bash hooks/dispatch-codex-appserver.sh <issue-key> <prompt-file>
# Emits COMPLETE or STUCK to .autoship/workspaces/<issue-key>/pane.log
# Emits verify or agent_stuck event to .autoship/event-queue.json

set -euo pipefail

ISSUE_KEY="${1:?usage: dispatch-codex-appserver.sh <issue-key> <prompt-file>}"
PROMPT_FILE="${2:?usage: dispatch-codex-appserver.sh <issue-key> <prompt-file>}"

# Validate ISSUE_KEY to prevent path traversal
if [[ ! "$ISSUE_KEY" =~ ^issue-[0-9]+[a-z0-9-]*$ ]]; then
  echo "Error: invalid ISSUE_KEY: $ISSUE_KEY" >&2
  exit 1
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
WORKSPACE="${REPO_ROOT}/.autoship/workspaces/${ISSUE_KEY}"
PANE_LOG="${WORKSPACE}/pane.log"

STALL_SECS=$(( ${STALL_TIMEOUT_MS:-300000} / 1000 ))

if ! command -v codex >/dev/null 2>&1; then
  echo "STUCK" >> "$PANE_LOG"
  exit 1
fi

if [[ ! -f "$PROMPT_FILE" ]]; then
  echo "ERROR: prompt file not found: $PROMPT_FILE" >&2
  echo "STUCK" >> "$PANE_LOG"
  exit 1
fi

mkdir -p "$WORKSPACE"
touch "$PANE_LOG"

FIFO_IN="${WORKSPACE}/.codex-stdin"
FIFO_OUT="${WORKSPACE}/.codex-stdout"
rm -f "$FIFO_IN" "$FIFO_OUT"
mkfifo -m 600 "$FIFO_IN" "$FIFO_OUT"

cleanup() {
  rm -f "$FIFO_IN" "$FIFO_OUT"
  [[ -n "${APP_SERVER_PID:-}" ]] && kill "$APP_SERVER_PID" 2>/dev/null || true
  [[ -n "${WATCHDOG_PID:-}" ]]   && kill "$WATCHDOG_PID"   2>/dev/null || true
}
trap cleanup EXIT

# Spawn codex app-server reading from fifo, writing to fifo
codex app-server < "$FIFO_IN" > "$FIFO_OUT" 2>/dev/null &
APP_SERVER_PID=$!

# Open write end of stdin fifo (keeps fifo open so app-server doesn't get EOF)
exec 3>"$FIFO_IN"

# Stall watchdog — kills app-server and emits stuck if deadline exceeded
(
  sleep "$STALL_SECS"
  kill "$APP_SERVER_PID" 2>/dev/null || true
  echo "STUCK" >> "$PANE_LOG"
) &
WATCHDOG_PID=$!

THREAD_ID="autoship-${ISSUE_KEY}-$$"
TOKENS_USED=0

send_rpc() {
  printf '%s\n' "$1" >&3
}

# Initialize
send_rpc '{"jsonrpc":"2.0","method":"initialize","params":{"clientInfo":{"name":"autoship","version":"1.0.0"}},"id":1}'

# thread/start
START_MSG=$(jq -n --arg tid "$THREAD_ID" \
  '{"jsonrpc":"2.0","method":"thread/start","params":{"threadId":$tid},"id":2}')
send_rpc "$START_MSG"

# turn/start with prompt content
PROMPT_TEXT=$(cat "$PROMPT_FILE")
PROMPT_JSON=$(jq -n --arg text "$PROMPT_TEXT" --arg threadId "$THREAD_ID" \
  '{"jsonrpc":"2.0","method":"turn/start","params":{"threadId":$threadId,"input":[{"type":"text","text":$text}]},"id":3}')
send_rpc "$PROMPT_JSON"

# Read responses
STATUS=""
while IFS= read -r line <"$FIFO_OUT"; do
  [[ -z "$line" ]] && continue

  METHOD=$(echo "$line" | jq -r '.method // empty' 2>/dev/null)

  case "$METHOD" in
    "thread/tokenUsage/updated")
      TOKENS_USED=$(echo "$line" | jq -r '.params.totalTokens // 0' 2>/dev/null || echo 0)
      ;;
    "turn/completed")
      STATUS="COMPLETE"
      break
      ;;
    "turn/failed")
      STATUS="STUCK"
      break
      ;;
  esac

  # Also handle JSON-RPC result responses (id-based)
  ID=$(echo "$line" | jq -r '.id // empty' 2>/dev/null)
  if [[ -n "$ID" ]] && echo "$line" | jq -e '.error' >/dev/null 2>&1; then
    STATUS="STUCK"
    break
  fi
done

# Cancel watchdog — we got a response
kill "$WATCHDOG_PID" 2>/dev/null || true

# Close stdin fifo
exec 3>&-

# Default to STUCK if no status word received
STATUS="${STATUS:-STUCK}"

# Write status to pane.log (monitor-agents.sh picks this up)
echo "$STATUS" >> "$PANE_LOG"

# Emit event to event queue (atomic flock write)
ISSUE_NUMBER="${ISSUE_KEY#issue-}"
EVENT_TYPE="$( [[ "$STATUS" == "COMPLETE" ]] && echo "verify" || echo "agent_stuck" )"
EVENT=$(jq -n \
  --arg type    "$EVENT_TYPE" \
  --arg issue   "$ISSUE_KEY" \
  --arg issueN  "$ISSUE_NUMBER" \
  --argjson tok "$TOKENS_USED" \
  --arg ts      "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  '{type: $type, issue: $issue, issue_number: ($issueN | sub("[^0-9].*"; "") | tonumber), tokens_used: $tok, timestamp: $ts}')

bash "${REPO_ROOT}/hooks/emit-event.sh" "$EVENT"

# Update token count in state.json
bash "${REPO_ROOT}/hooks/update-state.sh" set-running "$ISSUE_KEY" "tokens_used=${TOKENS_USED}" 2>/dev/null || true

exit 0
