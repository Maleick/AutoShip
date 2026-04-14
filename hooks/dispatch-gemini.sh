#!/usr/bin/env bash
# dispatch-gemini.sh — Dispatch Gemini CLI via `gemini -p` (headless)
# Usage: bash hooks/dispatch-gemini.sh <issue-key> <prompt-file>
# Emits COMPLETE or STUCK to .autoship/workspaces/<issue-key>/pane.log
# Emits verify or agent_stuck event to .autoship/event-queue.json
#
# Uses `gemini -p` (headless prompt mode) with --yolo (auto-approve).
# No sandbox issues with worktrees — Gemini doesn't restrict file access.

set -euo pipefail

source "${BASH_SOURCE%/*}/lib/dispatch-helpers.sh"

ISSUE_KEY="${1:?usage: dispatch-gemini.sh <issue-key> <prompt-file>}"
PROMPT_FILE="${2:?usage: dispatch-gemini.sh <issue-key> <prompt-file>}"

# Validate ISSUE_KEY to prevent path traversal
if [[ ! "$ISSUE_KEY" =~ ^issue-[0-9]+[a-z0-9-]*$ ]]; then
  echo "Error: invalid ISSUE_KEY: $ISSUE_KEY" >&2
  exit 1
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
WORKSPACE="${REPO_ROOT}/.autoship/workspaces/${ISSUE_KEY}"
PANE_LOG="${WORKSPACE}/pane.log"
HOOKS_DIR="$(cat "${REPO_ROOT}/.autoship/hooks_dir" 2>/dev/null || echo "${REPO_ROOT}/hooks")"

STALL_SECS=$(( ${STALL_TIMEOUT_MS:-300000} / 1000 ))

TOOL="gemini"

# Fast-fail health check
if ! command -v gemini >/dev/null 2>&1; then
  mkdir -p "$WORKSPACE"
  echo "STUCK" >> "$PANE_LOG"
  bash "$HOOKS_DIR/quota-update.sh" stuck "$TOOL" || true

  ISSUE_NUMBER="${ISSUE_KEY#issue-}"
  EVENT=$(jq -n \
    --arg type    "agent_stuck" \
    --arg issue   "$ISSUE_KEY" \
    --arg issueN  "$ISSUE_NUMBER" \
    --argjson tok 0 \
    --arg ts      "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    '{type: $type, issue: $issue, issue_number: ($issueN | tonumber), tokens_used: $tok, timestamp: $ts}')
  bash "$HOOKS_DIR/emit-event.sh" "$EVENT" 2>/dev/null || true
  exit 0
fi

if [[ ! -f "$PROMPT_FILE" ]]; then
  echo "ERROR: prompt file not found: $PROMPT_FILE" >&2
  echo "STUCK" >> "$PANE_LOG"
  exit 1
fi

mkdir -p "$WORKSPACE"

# Clean up stale watcher and increment attempt counter
kill_old_watcher "$WORKSPACE"
ATTEMPT_COUNT_FILE="${WORKSPACE}/.attempt_count"
ATTEMPT_NUM=$(( $(cat "$ATTEMPT_COUNT_FILE" 2>/dev/null || echo 0) + 1 ))
echo "$ATTEMPT_NUM" > "$ATTEMPT_COUNT_FILE"
echo "=== Dispatch attempt #$ATTEMPT_NUM ===" >> "$PANE_LOG"

# ---------------------------------------------------------------------------
# Run gemini -p with stall watchdog
# ---------------------------------------------------------------------------
EXIT_CODE=0
GEMINI_PID=""

cleanup() {
  [[ -n "${GEMINI_PID:-}" ]] && kill "$GEMINI_PID" 2>/dev/null || true
  [[ -n "${WATCHDOG_PID:-}" ]] && kill "$WATCHDOG_PID" 2>/dev/null || true
}
trap cleanup EXIT

# Read prompt from file and pass via -p flag
# Run in the workspace directory so gemini sees the repo context
PROMPT_TEXT=$(cat "$PROMPT_FILE")
(cd "$WORKSPACE" && gemini -p "$PROMPT_TEXT" --yolo) >> "$PANE_LOG" 2>&1 &
GEMINI_PID=$!

# Stall watchdog — kills gemini if deadline exceeded
(
  sleep "$STALL_SECS"
  kill "$GEMINI_PID" 2>/dev/null || true
  echo "STUCK" >> "$PANE_LOG"
) &
WATCHDOG_PID=$!

# Wait for gemini to finish
wait "$GEMINI_PID" 2>/dev/null || EXIT_CODE=$?
GEMINI_PID=""

# Cancel watchdog
kill "$WATCHDOG_PID" 2>/dev/null || true
WATCHDOG_PID=""

# ---------------------------------------------------------------------------
# Determine status
# ---------------------------------------------------------------------------
STATUS=""
if [[ -f "${WORKSPACE}/AUTOSHIP_RESULT.md" ]]; then
  STATUS="COMPLETE"
elif [[ $EXIT_CODE -eq 0 ]]; then
  # Gemini exited cleanly but didn't produce a result file.
  # Check if it made any git changes (uncommitted or committed).
  CHANGES=$(git -C "$WORKSPACE" diff --name-only HEAD -- ':!pane.log' ':!.watcher.pid' ':!run-agent.sh' 2>/dev/null | wc -l | tr -d ' ')
  COMMITS=$(git -C "$WORKSPACE" log "$(git -C "$WORKSPACE" merge-base HEAD master 2>/dev/null || echo HEAD)"..HEAD --oneline 2>/dev/null | wc -l | tr -d ' ')
  if [[ "$CHANGES" -gt 0 ]] || [[ "$COMMITS" -gt 0 ]]; then
    STATUS="COMPLETE"
  else
    STATUS="STUCK"
  fi
else
  STATUS="STUCK"
fi

echo "$STATUS" >> "$PANE_LOG"

if [[ "$STATUS" == "STUCK" ]]; then
  bash "$HOOKS_DIR/quota-update.sh" stuck "$TOOL" || true
fi

# ---------------------------------------------------------------------------
# Parse token usage from gemini output if present
# ---------------------------------------------------------------------------
TOKENS_USED=0
if [[ -f "$PANE_LOG" ]]; then
  PARSED=$(grep -oiE '(total tokens|tokens used|token_count|tokencount)[[:space:]]*[:=][[:space:]]*[0-9]+' "$PANE_LOG" \
    | grep -oE '[0-9]+$' | tail -1 || true)
  [[ -n "$PARSED" ]] && TOKENS_USED="$PARSED"
fi

# ---------------------------------------------------------------------------
# Emit event to queue
# ---------------------------------------------------------------------------
ISSUE_NUMBER="${ISSUE_KEY#issue-}"
EVENT_TYPE="$( [[ "$STATUS" == "COMPLETE" ]] && echo "verify" || echo "agent_stuck" )"
EVENT=$(jq -n \
  --arg type    "$EVENT_TYPE" \
  --arg issue   "$ISSUE_KEY" \
  --arg issueN  "$ISSUE_NUMBER" \
  --argjson tok "$TOKENS_USED" \
  --arg ts      "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  '{type: $type, issue: $issue, issue_number: ($issueN | sub("[^0-9].*"; "") | tonumber), tokens_used: $tok, timestamp: $ts}')

bash "$HOOKS_DIR/emit-event.sh" "$EVENT" 2>/dev/null || true

# Update token count in state
bash "$HOOKS_DIR/update-state.sh" set-running "$ISSUE_KEY" "tokens_used=${TOKENS_USED}" 2>/dev/null || true

exit 0
