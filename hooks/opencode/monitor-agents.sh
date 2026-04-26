# monitor-agents-opencode.sh — Poll status files for OpenCode agents
# Adapted from monitor-agents.sh for OpenCode's file-based status

set -eo pipefail

AUTOSHIP_DIR=".autoship"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
EVENT_QUEUE="$AUTOSHIP_DIR/event-queue.json"
LOCK_FILE="$AUTOSHIP_DIR/event-queue.lock"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

[[ ! -d "$WORKSPACES_DIR" ]] && exit 0
mkdir -p "$AUTOSHIP_DIR"
[[ -f "$EVENT_QUEUE" ]] || printf '[]\n' > "$EVENT_QUEUE"

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

  write_event() {
    jq --argjson evt "$event" '. + [$evt]' "$EVENT_QUEUE" > "${EVENT_QUEUE}.tmp" 2>/dev/null
    mv "${EVENT_QUEUE}.tmp" "$EVENT_QUEUE" 2>/dev/null || true
    touch "$marker" 2>/dev/null || true
  }

  if [[ "$(uname -s 2>/dev/null || true)" == "Darwin" ]] && command -v lockf >/dev/null 2>&1; then
    lockf -k "$LOCK_FILE" bash -c 'jq --argjson evt "$1" '\'' . + [$evt] '\'' "$2" > "$2.tmp" && mv "$2.tmp" "$2" && touch "$3"' _ "$event" "$EVENT_QUEUE" "$marker" 2>/dev/null || write_event
  elif command -v flock >/dev/null 2>&1; then
    (
      if flock -x 200 2>/dev/null; then
        write_event
      else
        write_event
      fi
    ) 200>"$LOCK_FILE"
  else
    write_event
  fi
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
  started_epoch=$(
    date -j -f "%Y-%m-%dT%H:%M:%SZ" "$started" +%s 2>/dev/null \
      || date -d "$started" +%s 2>/dev/null \
      || echo 0
  )
  [[ "$started_epoch" =~ ^[0-9]+$ && "$started_epoch" != "0" ]] || return 0
  local elapsed=$((now - started_epoch))
  local timeout_ms timeout_secs
  timeout_ms="${AUTOSHIP_WORKER_TIMEOUT_MS:-}"
  if [[ -z "$timeout_ms" && -f "$AUTOSHIP_DIR/config.json" ]]; then
    timeout_ms=$(jq -r '.workerTimeoutMs // .stall_timeout_ms // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)
  fi
  if [[ -z "$timeout_ms" && -f "$STATE_FILE" ]]; then
    timeout_ms=$(jq -r '.config.workerTimeoutMs // .config.stall_timeout_ms // empty' "$STATE_FILE" 2>/dev/null || true)
  fi
  timeout_ms="${timeout_ms:-900000}"
  timeout_secs=$((timeout_ms / 1000))
  (( timeout_secs > 0 )) || timeout_secs=900

  if (( elapsed > timeout_secs )); then
    local current_status=$(cat "$status_file" 2>/dev/null || echo "")
    if [[ "$current_status" == "RUNNING" ]]; then
      echo "STUCK" > "$status_file"
      if [[ -x "$REPO_ROOT/hooks/capture-failure.sh" ]]; then
        bash "$REPO_ROOT/hooks/capture-failure.sh" timeout "$key" "error_summary=worker exceeded ${timeout_secs}s runtime" 2>/dev/null || true
      fi
      emit_event "stuck" "$key" "STUCK"
    fi
  fi
}

is_worker_live() {
  local pid_file="$1/worker.pid"
  [[ -s "$pid_file" ]] || return 0
  local pid
  local pid_re='^[0-9]+$'
  pid=$(tr -d '[:space:]' < "$pid_file")
  [[ "$pid" =~ $pid_re ]] || return 1
  kill -0 "$pid" 2>/dev/null
}

has_fresh_result() {
  local dir="$1"
  local result_file="$dir/AUTOSHIP_RESULT.md"
  local started_file="$dir/started_at"
  [[ -s "$result_file" ]] || return 1
  [[ ! -f "$started_file" || "$result_file" -nt "$started_file" ]]
}

reconcile_exited_worker() {
  local dir="$1"
  local key
  key=$(basename "$dir")
  local status_file="$dir/status"

  is_worker_live "$dir" && return 0

  if has_fresh_result "$dir"; then
    echo "COMPLETE" > "$status_file"
    emit_event "verify" "$key" "COMPLETE"
  else
    echo "STUCK" > "$status_file"
    if [[ -x "$REPO_ROOT/hooks/capture-failure.sh" ]]; then
      bash "$REPO_ROOT/hooks/capture-failure.sh" dead_worker "$key" "error_summary=worker process exited without fresh result" 2>/dev/null || true
    fi
    emit_event "stuck" "$key" "STUCK"
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
      reconcile_exited_worker "$dir"
      check_stalled "$dir"
      ;;
  esac
done

bash "$SCRIPT_DIR/reconcile-state.sh" >/dev/null 2>&1 || true
