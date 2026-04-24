#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
MAX=$(jq -r '.config.maxConcurrentAgents // .max_concurrent_agents // empty' "$STATE_FILE" 2>/dev/null || true)
if [[ -z "$MAX" && -f "$AUTOSHIP_DIR/config.json" ]]; then
  MAX=$(jq -r '.maxConcurrentAgents // .max_agents // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)
fi
MAX="${MAX:-15}"
DRY_RUN="${AUTOSHIP_RUNNER_DRY_RUN:-false}"

active_count() {
  local count=0
  if [[ -d "$WORKSPACES_DIR" ]]; then
    count=$((grep -Rsl '^RUNNING$' "$WORKSPACES_DIR"/*/status 2>/dev/null || true) | wc -l | tr -d ' ')
  fi
  printf '%s\n' "$count"
}

run_worker() {
  local model="$1"
  env \
    -u OPENCODE \
    -u OPENCODE_CLIENT \
    -u OPENCODE_PID \
    -u OPENCODE_PROCESS_ROLE \
    -u OPENCODE_RUN_ID \
    -u OPENCODE_SERVER_PASSWORD \
    -u OPENCODE_SERVER_USERNAME \
    opencode run --model "$model" "$(cat AUTOSHIP_PROMPT.md)"
}

is_billing_or_quota_failure() {
  local log_file="${1:-AUTOSHIP_RUNNER.log}"
  [[ -f "$log_file" ]] || return 1
  grep -Eiq 'insufficient balance|billing|quota|rate limit|credit' "$log_file"
}

record_model_failure() {
  local model="$1"
  local log_file="${2:-AUTOSHIP_RUNNER.log}"
  local history_file="$REPO_ROOT/$AUTOSHIP_DIR/model-history.json"
  local tmp
  tmp=$(mktemp)
  local summary
  summary=$(tail -5 "$log_file" 2>/dev/null || true)
  [[ -f "$history_file" ]] || printf '{}\n' > "$history_file"
  jq --arg model "$model" --arg summary "$summary" --arg now "$(date -u +%Y-%m-%dT%H:%M:%SZ)" '
    .[$model] = ((.[$model] // {}) + {
      fail: (((.[$model].fail // 0) | tonumber) + 1),
      last_error: $summary,
      last_failed_at: $now
    })
  ' "$history_file" > "$tmp" && mv "$tmp" "$history_file"
}

select_free_fallback_model() {
  local failed_model="$1"
  local routing_file="$REPO_ROOT/$AUTOSHIP_DIR/model-routing.json"
  local task_type="medium_code"
  if [[ -f "$REPO_ROOT/$STATE_FILE" ]]; then
    task_type=$(jq -r --arg key "$issue_id" '.issues[$key].task_type // "medium_code"' "$REPO_ROOT/$STATE_FILE" 2>/dev/null || echo "medium_code")
  fi
  [[ -f "$routing_file" ]] || return 1
  jq -r --arg failed "$failed_model" --arg task "$task_type" '
    [(.models // [])[] |
      select((.enabled // true) == true) |
      select(.cost == "free") |
      select(.id != $failed) |
      select(((.max_task_types // []) | length == 0) or ((.max_task_types // []) | index($task) != null))]
    | sort_by(-(.strength // 0), .id)
    | .[0].id // empty
  ' "$routing_file"
}

mark_stuck_unless_terminal() {
  local wid="$1"
  local repo_root="$2"
  local current=""
  [[ -f status ]] && current=$(tr -d '[:space:]' < status)
  case "$current" in
    COMPLETE|BLOCKED|STUCK) ;;
    *)
      echo "STUCK" > status
      if [[ -x "$repo_root/hooks/capture-failure.sh" ]]; then
        error_msg=$(tail -5 AUTOSHIP_RUNNER.log 2>/dev/null || echo "worker exited without terminal status")
        bash "$repo_root/hooks/capture-failure.sh" stuck "$wid" "error_summary=$error_msg" 2>/dev/null || true
      fi
      ;;
  esac
}

started=0
for dir in "$WORKSPACES_DIR"/*/; do
  [[ -d "$dir" ]] || continue
  status_file="$dir/status"
  prompt_file="$dir/AUTOSHIP_PROMPT.md"
  model_file="$dir/model"
  role_file="$dir/role"
  [[ -f "$status_file" && -f "$prompt_file" ]] || continue
  status=$(tr -d '[:space:]' < "$status_file")
  [[ "$status" == "QUEUED" ]] || continue

  active=$(active_count)
  if (( active >= MAX )); then
    echo "CAP_REACHED: $active active / $MAX max"
    break
  fi

  model="opencode/nemotron-3-super-free"
  role="implementer"
  [[ -f "$model_file" ]] && model=$(cat "$model_file")
  [[ -f "$role_file" ]] && role=$(cat "$role_file")
  issue_id="$(basename "$dir")"
  echo "RUNNING" > "$status_file"
  bash "$REPO_ROOT/hooks/update-state.sh" set-running "$(basename "$dir")" agent="$model" model="$model" role="$role" 2>/dev/null || true

  if [[ "$DRY_RUN" == "true" ]]; then
    echo "DRY_RUN start $(basename "$dir") with $model"
  else
    (
      cd "$dir"
      if command -v opencode >/dev/null 2>&1; then
        if run_worker "$model" > AUTOSHIP_RUNNER.log 2>&1; then
          mark_stuck_unless_terminal "$issue_id" "$REPO_ROOT"
        else
          if is_billing_or_quota_failure AUTOSHIP_RUNNER.log; then
            record_model_failure "$model" AUTOSHIP_RUNNER.log
            fallback_model=$(select_free_fallback_model "$model" || true)
            if [[ -n "$fallback_model" ]]; then
              printf '%s\n' "$fallback_model" > model
              bash "$REPO_ROOT/hooks/update-state.sh" set-running "$issue_id" agent="$fallback_model" model="$fallback_model" role="$role" 2>/dev/null || true
              if run_worker "$fallback_model" >> AUTOSHIP_RUNNER.log 2>&1; then
                mark_stuck_unless_terminal "$issue_id" "$REPO_ROOT"
              else
                echo "STUCK" > status
                if [[ -x "$REPO_ROOT/hooks/capture-failure.sh" ]]; then
                  error_msg=$(tail -5 AUTOSHIP_RUNNER.log 2>/dev/null || echo "fallback worker run failed")
                  bash "$REPO_ROOT/hooks/capture-failure.sh" model_failure "$issue_id" "error_summary=$error_msg" 2>/dev/null || true
                fi
              fi
            else
              echo "STUCK" > status
              if [[ -x "$REPO_ROOT/hooks/capture-failure.sh" ]]; then
                error_msg=$(tail -5 AUTOSHIP_RUNNER.log 2>/dev/null || echo "worker run failed")
                bash "$REPO_ROOT/hooks/capture-failure.sh" model_failure "$issue_id" "error_summary=$error_msg" 2>/dev/null || true
              fi
            fi
          else
            echo "STUCK" > status
            if [[ -x "$REPO_ROOT/hooks/capture-failure.sh" ]]; then
              error_msg=$(tail -5 AUTOSHIP_RUNNER.log 2>/dev/null || echo "worker run failed")
              bash "$REPO_ROOT/hooks/capture-failure.sh" model_failure "$issue_id" "error_summary=$error_msg" 2>/dev/null || true
            fi
          fi
        fi
      else
        echo "opencode CLI not found" > AUTOSHIP_RUNNER.log
        echo "STUCK" > status
        if [[ -x "$REPO_ROOT/hooks/capture-failure.sh" ]]; then
          bash "$REPO_ROOT/hooks/capture-failure.sh" model_failure "$issue_id" "error_summary=opencode CLI not found" 2>/dev/null || true
        fi
      fi
    ) &
    printf '%s\n' "$!" > "$dir/worker.pid"
    echo "Started $(basename "$dir") with $model"
  fi
  started=$((started + 1))
done

echo "Runner started $started workspace(s)"
