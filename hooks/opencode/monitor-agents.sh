# monitor-agents-opencode.sh — Poll status files for OpenCode agents
# Adapted from monitor-agents.sh for OpenCode's file-based status

set -euo pipefail

AUTOSHIP_DIR=".autoship"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
EVENT_QUEUE="$AUTOSHIP_DIR/event-queue.json"
LOCK_FILE="$AUTOSHIP_DIR/event-queue.lock"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

[[ ! -d "$WORKSPACES_DIR" ]] && exit 0

emit_event() {
  local type="$1"
  local issue="$2"
  local status="$3"
  local workspace_dir="$WORKSPACES_DIR/$issue"
  local marker="$workspace_dir/.autoship-event-${status}.sent"
  [[ -f "$marker" ]] && return 0
  local event
  event=$(jq -n \
    --arg type "$type" \
    --arg issue "$issue" \
    --arg status "$status" \
    --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{type: $type, issue: $issue, priority: 2, data: {status: $status}, queued_at: $ts}')

  (
    flock -x 200 || exit 1
    jq --argjson evt "$event" '. + [$evt]' "$EVENT_QUEUE" > "${EVENT_QUEUE}.tmp" 2>/dev/null
    mv "${EVENT_QUEUE}.tmp" "$EVENT_QUEUE" 2>/dev/null || true
    touch "$marker" 2>/dev/null || true
  ) 200>"$LOCK_FILE"
}

check_stalled() {
  local dir="$1"
  local key=$(basename "$dir")
  local status_file="$dir/status"
  local started_file="$dir/started_at"

  [[ ! -f "$status_file" ]] && return 0
  [[ ! -f "$started_file" ]] && return 0

  local started=$(cat "$started_file")
  local now=$(date +%s)
  local started_epoch
  started_epoch=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$started" +%s 2>/dev/null || echo 0)
  local elapsed=$((now - started_epoch))

  # 60 minute stall timeout
  if (( elapsed > 3600 )); then
    local current_status=$(cat "$status_file" 2>/dev/null || echo "")
    if [[ "$current_status" == "RUNNING" ]]; then
      echo "STUCK" > "$status_file"
      emit_event "stuck" "$key" "STUCK"
    fi
  fi
}

for dir in "$WORKSPACES_DIR"/*/; do
  [[ -d "$dir" ]] || continue
  key=$(basename "$dir")
  status_file="$dir/status"

  [[ ! -f "$status_file" ]] && continue

  status=$(cat "$status_file" 2>/dev/null || echo "")

  case "$status" in
    COMPLETE)
      emit_event "verify" "$key" "COMPLETE"
      ;;
    BLOCKED)
      emit_event "blocked" "$key" "BLOCKED"
      ;;
    STUCK)
      emit_event "stuck" "$key" "STUCK"
      ;;
    RUNNING)
      check_stalled "$dir"
      ;;
  esac
done

bash "$SCRIPT_DIR/reconcile-state.sh" >/dev/null 2>&1 || true
