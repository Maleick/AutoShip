#!/usr/bin/env bash
# monitor-agents-opencode.sh — Poll status files for OpenCode agents
# Dependency graph: lib/common.sh (optional), capture-failure.sh, reconcile-state.sh
# Leaf callers: capture-failure.sh, reconcile-state.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load shared utilities if available; inline fallback for standalone/test use.
if [[ -f "$SCRIPT_DIR/lib/common.sh" ]]; then
  source "$SCRIPT_DIR/lib/common.sh"
else
  autoship_repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || pwd
  }
  autoship_capture_failure() {
    local category="$1" issue_id="$2"
    shift 2
    local repo_root
    repo_root="$(autoship_repo_root)"
    bash "$repo_root/hooks/capture-failure.sh" "$category" "$issue_id" "$@" 2>/dev/null || true
  }
fi

REPO_ROOT="$(autoship_repo_root)"

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
EVENT_QUEUE="$AUTOSHIP_DIR/event-queue.json"
LOCK_FILE="$AUTOSHIP_DIR/event-queue.lock"

[[ ! -d "$WORKSPACES_DIR" ]] && exit 0
mkdir -p "$AUTOSHIP_DIR"
[[ -f "$EVENT_QUEUE" ]] || printf '[]\n' >"$EVENT_QUEUE"

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
    jq --argjson evt "$event" '. + [$evt]' "$EVENT_QUEUE" >"${EVENT_QUEUE}.tmp" 2>/dev/null
    mv "${EVENT_QUEUE}.tmp" "$EVENT_QUEUE" 2>/dev/null || true
    touch "$marker" 2>/dev/null || true
  }

  # Skip flock on macOS (Darwin) due to compatibility issues with fd-based locking
  if command -v flock >/dev/null 2>&1 && [[ "$(uname -s)" != "Darwin" ]]; then
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
  # Validate timeout_ms is numeric
  if [[ ! "$timeout_ms" =~ ^[0-9]+$ ]]; then
    timeout_ms=900000
  fi
  timeout_secs=$((timeout_ms / 1000))
  ((timeout_secs > 0)) || timeout_secs=900

  if ((elapsed > timeout_secs)); then
    local current_status=$(cat "$status_file" 2>/dev/null || echo "")
    if [[ "$current_status" == "RUNNING" ]]; then
      echo "STUCK" >"$status_file"
      autoship_capture_failure timeout "$key" "error_summary=worker exceeded ${timeout_secs}s runtime"
      emit_event "stuck" "$key" "STUCK"
    fi
  fi
}

is_worker_live() {
  local pid_file="$1/worker.pid"
  [[ -s "$pid_file" ]] || return 0
  local pid
  pid=$(tr -d '[:space:]' <"$pid_file")
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
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
    echo "COMPLETE" >"$status_file"
    emit_event "verify" "$key" "COMPLETE"
  else
    echo "STUCK" >"$status_file"
    autoship_capture_failure dead_worker "$key" "error_summary=worker process exited without fresh result"
    emit_event "stuck" "$key" "STUCK"
  fi
}

for dir in "$WORKSPACES_DIR"/*/; do
  [[ -d "$dir" ]] || continue
  key=$(basename "$dir")
  [[ "$key" =~ ^issue-[0-9]+$ ]] || continue
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

bash "$SCRIPT_DIR/reconcile-state.sh" >/dev/null 2>&1
