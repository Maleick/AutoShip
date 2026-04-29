#!/usr/bin/env bash
# Dependency graph: lib/common.sh (optional), lib/state-lib.sh (optional), select-model.sh, update-state.sh, capture-failure.sh
# Leaf callers: update-state.sh, capture-failure.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load shared utilities if available; inline fallback for standalone/test use.
if [[ -f "$SCRIPT_DIR/lib/common.sh" ]]; then
  source "$SCRIPT_DIR/lib/common.sh"
else
  autoship_repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || {
      echo "Error: not inside a git repository" >&2
      return 1
    }
  }
  autoship_config_value() {
    local key="$1" default="$2"
    local value="" repo_root state_file config_file
    repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
    state_file="$repo_root/.autoship/state.json"
    config_file="$repo_root/.autoship/config.json"
    if [[ -f "$state_file" ]]; then
      value=$(jq -r --arg key "$key" '.config[$key] // empty' "$state_file" 2>/dev/null || true)
    fi
    if [[ -z "$value" && -f "$config_file" ]]; then
      value=$(jq -r --arg key "$key" '.[$key] // empty' "$config_file" 2>/dev/null || true)
    fi
    printf '%s\n' "${value:-$default}"
  }
  autoship_running_count() {
    local ws_dir="$(git rev-parse --show-toplevel 2>/dev/null || true)/.autoship/workspaces"
    if [[ -d "$ws_dir" ]]; then
      grep -Rsl '^RUNNING$' "$ws_dir"/*/status 2>/dev/null | wc -l | tr -d ' '
    else
      printf '0\n'
    fi
  }
  autoship_state_set() {
    local action="$1" issue_key="$2"
    shift 2
    local repo_root
    repo_root="$(autoship_repo_root)"
    bash "$repo_root/hooks/update-state.sh" "$action" "$issue_key" "$@" 2>/dev/null || true
  }
  autoship_capture_failure() {
    local category="$1" issue_id="$2"
    shift 2
    local repo_root
    repo_root="$(autoship_repo_root)"
    bash "$repo_root/hooks/capture-failure.sh" "$category" "$issue_id" "$@" 2>/dev/null || true
  }
fi

REPO_ROOT=$(autoship_repo_root) || exit 1
cd "$REPO_ROOT"

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
MAX=$(jq -r '.config.maxConcurrentAgents // .max_concurrent_agents // empty' "$STATE_FILE" 2>/dev/null || true)
if [[ -z "$MAX" && -f "$AUTOSHIP_DIR/config.json" ]]; then
  MAX=$(jq -r '.maxConcurrentAgents // .max_agents // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)
fi
MAX="${MAX:-15}"
# Validate MAX is numeric
if [[ ! "$MAX" =~ ^[0-9]+$ ]]; then
  MAX=15
fi
DRY_RUN="${AUTOSHIP_RUNNER_DRY_RUN:-false}"

active_count() {
  local count=0
  if [[ -d "$WORKSPACES_DIR" ]]; then
    count=$((grep -Rsl '^RUNNING$' "$WORKSPACES_DIR"/*/status 2>/dev/null || true) | wc -l | tr -d ' ')
  fi
  printf '%s\n' "$count"
}

config_value() {
  local key="$1" default="$2"
  local value=""
  if [[ -f "$STATE_FILE" ]]; then
    value=$(jq -r --arg key "$key" '.config[$key] // empty' "$STATE_FILE" 2>/dev/null || true)
  fi
  if [[ -z "$value" && -f "$AUTOSHIP_DIR/config.json" ]]; then
    value=$(jq -r --arg key "$key" '.[$key] // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)
  fi
  printf '%s\n' "${value:-$default}"
}

repo_is_rust() {
  [[ -f "$REPO_ROOT/Cargo.toml" ]] || find "$REPO_ROOT" -maxdepth 2 -name Cargo.toml -print -quit 2>/dev/null | grep -q .
}

should_isolate_cargo_target() {
  local threshold
  threshold=$(config_value cargoTargetIsolationThreshold 8)
  [[ "$MAX" =~ ^[0-9]+$ && "$threshold" =~ ^[0-9]+$ ]] || return 1
  repo_is_rust || return 1
  (( MAX > threshold ))
}

run_worker() {
  local model="$1"
  local cargo_target_dir=""
  if should_isolate_cargo_target; then
    cargo_target_dir="$PWD/target-isolated"
  fi
  if [[ -n "$cargo_target_dir" ]]; then
    env \
      -u OPENCODE -u OPENCODE_CLIENT -u OPENCODE_PID -u OPENCODE_PROCESS_ROLE -u OPENCODE_RUN_ID -u OPENCODE_SERVER_PASSWORD -u OPENCODE_SERVER_USERNAME \
      CARGO_TARGET_DIR="$cargo_target_dir" \
      opencode run --model "$model" "$(cat AUTOSHIP_PROMPT.md)"
  else
    env \
      -u OPENCODE -u OPENCODE_CLIENT -u OPENCODE_PID -u OPENCODE_PROCESS_ROLE -u OPENCODE_RUN_ID -u OPENCODE_SERVER_PASSWORD -u OPENCODE_SERVER_USERNAME \
      opencode run --model "$model" "$(cat AUTOSHIP_PROMPT.md)"
  fi
}

is_billing_or_quota_failure() {
  local log_file="${1:-AUTOSHIP_RUNNER.log}"
  [[ -f "$log_file" ]] || return 1
  grep -Eiq 'insufficient balance|billing|quota|rate limit|credit' "$log_file"
}

annotate_session_failure() {
  local log_file="${1:-AUTOSHIP_RUNNER.log}"
  [[ -f "$log_file" ]] || return 1
  if grep -Fqi 'Session not found' "$log_file"; then
    cat >> "$log_file" <<'EOF'

AutoShip diagnostic: OpenCode returned Session not found while running a worker.
This usually means the current OpenCode CLI/server session cannot start nested `opencode run` jobs for this environment or model. Try restarting OpenCode, confirming the selected model is enabled with `opencode models`, and rerunning the workspace.
EOF
  fi
}

base_ref_for_workspace() {
  for ref in origin/master origin/main master main HEAD~1; do
    if git rev-parse --verify "$ref" >/dev/null 2>&1; then
      printf '%s\n' "$ref"
      return 0
    fi
  done
  return 1
}

auto_commit_workspace_changes() {
  local issue_key="$1"
  if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    return 0
  fi

  if git diff --quiet && git diff --cached --quiet && [[ -z "$(git ls-files --others --exclude-standard)" ]]; then
    return 0
  fi

  git add -A
  if git diff --cached --quiet; then
    return 0
  fi

  git \
    -c user.name="AutoShip" \
    -c user.email="autoship@local" \
    commit -m "autoship: ${issue_key} auto-commit" >> AUTOSHIP_RUNNER.log 2>&1
}

has_non_runtime_changes() {
  git status --porcelain | while IFS= read -r line; do
    path="${line#???}"
    case "$line" in R*|C*) path="${path#* -> }" ;; esac
    case "$path" in
      AUTOSHIP_PROMPT.md|AUTOSHIP_RESULT.md|AUTOSHIP_RUNNER.log|AUTOSHIP_VERIFICATION.log|BLOCKED_REASON.txt|model|role|routing-log.txt|started_at|status|worker.pid|target-isolated|target-isolated/*) ;;
      *) printf '%s\n' "$path" ;;
    esac
  done | grep -q .
}

salvage_truncated_worker() {
  local issue_key="$1"
  local repo_root="$2"
  local current=""
  [[ -f status ]] && current=$(tr -d '[:space:]' < status)
  case "$current" in COMPLETE|BLOCKED|STUCK) return 0 ;; esac
  git rev-parse --is-inside-work-tree >/dev/null 2>&1 || return 1
  has_non_runtime_changes || return 1
  auto_commit_workspace_changes "$issue_key"
  if [[ ! -s AUTOSHIP_RESULT.md ]]; then
    {
      printf 'Implemented issue %s with salvaged worker changes.\n\n' "$issue_key"
      printf 'The worker exited without a terminal AutoShip status after modifying implementation files. AutoShip committed the changed files for normal verification.\n'
    } > AUTOSHIP_RESULT.md
  fi
  echo "COMPLETE" > status
  autoship_capture_failure salvaged_truncation "$issue_key" "error_summary=worker exited without terminal status but non-runtime changes were committed"
  return 0
}

is_test_path() {
  case "$1" in
    tests/*|test/*|*/tests/*|*/test/*|__tests__/*|*/__tests__/*|*.test.*|*.spec.*|*_test.*|*.snap|snapshots/*|*/snapshots/*) return 0 ;;
    *) return 1 ;;
  esac
}

production_additions_count() {
  local base_ref="$1"
  local count=0 file
  while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    if ! is_test_path "$file"; then
      count=$((count + 1))
    fi
  done < <(git diff --name-only "$base_ref"...HEAD 2>/dev/null || git diff --name-only "$base_ref" HEAD 2>/dev/null)
  printf '%s\n' "$count"
}

reject_tests_only_complete() {
  local issue_key="$1"
  local repo_root="$2"
  local current base_ref prod_changes
  current=$(tr -d '[:space:]' < status 2>/dev/null || true)
  [[ "$current" == "COMPLETE" ]] || return 0
  if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    return 0
  fi
  base_ref=$(base_ref_for_workspace || true)
  [[ -n "$base_ref" ]] || return 0
  if git diff --quiet "$base_ref"...HEAD 2>/dev/null || git diff --quiet "$base_ref" HEAD 2>/dev/null; then
    return 0
  fi
  prod_changes=$(production_additions_count "$base_ref")
  if [[ "$prod_changes" == "0" ]]; then
    printf '%s\n' "REJECT: tests-only diff" >> AUTOSHIP_RUNNER.log
    printf 'STUCK\n' > status
    autoship_capture_failure tests_only "$issue_key" "error_summary=worker reported COMPLETE with tests-only diff"
  fi
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
  error_msg=$(tail -5 AUTOSHIP_RUNNER.log 2>/dev/null || echo "worker exited without terminal status")
  autoship_capture_failure stuck "$wid" "error_summary=$error_msg"
      echo "STUCK" > status
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
  autoship_state_set set-running "$(basename "$dir")" agent="$model" model="$model" role="$role"

  if [[ "$DRY_RUN" == "true" ]]; then
    echo "DRY_RUN start $(basename "$dir") with $model"
  else
    (
      cd "$dir"
      if command -v opencode >/dev/null 2>&1; then
        if run_worker "$model" > AUTOSHIP_RUNNER.log 2>&1; then
          auto_commit_workspace_changes "$issue_id"
          reject_tests_only_complete "$issue_id" "$REPO_ROOT"
          salvage_truncated_worker "$issue_id" "$REPO_ROOT" || mark_stuck_unless_terminal "$issue_id" "$REPO_ROOT"
        else
          if is_billing_or_quota_failure AUTOSHIP_RUNNER.log; then
            record_model_failure "$model" AUTOSHIP_RUNNER.log
            fallback_model=$(select_free_fallback_model "$model" || true)
            if [[ -n "$fallback_model" ]]; then
              printf '%s\n' "$fallback_model" > model
              autoship_state_set set-running "$issue_id" agent="$fallback_model" model="$fallback_model" role="$role"
              if run_worker "$fallback_model" >> AUTOSHIP_RUNNER.log 2>&1; then
                auto_commit_workspace_changes "$issue_id"
                reject_tests_only_complete "$issue_id" "$REPO_ROOT"
                salvage_truncated_worker "$issue_id" "$REPO_ROOT" || mark_stuck_unless_terminal "$issue_id" "$REPO_ROOT"
              else
                echo "STUCK" > status
                error_msg=$(tail -5 AUTOSHIP_RUNNER.log 2>/dev/null || echo "fallback worker run failed")
                autoship_capture_failure model_failure "$issue_id" "error_summary=$error_msg"
              fi
            else
              echo "STUCK" > status
              error_msg=$(tail -5 AUTOSHIP_RUNNER.log 2>/dev/null || echo "worker run failed")
              autoship_capture_failure model_failure "$issue_id" "error_summary=$error_msg"
            fi
          else
            annotate_session_failure AUTOSHIP_RUNNER.log || true
            echo "STUCK" > status
            error_msg=$(tail -5 AUTOSHIP_RUNNER.log 2>/dev/null || echo "worker run failed")
            autoship_capture_failure model_failure "$issue_id" "error_summary=$error_msg"
          fi
        fi
      else
        echo "opencode CLI not found" > AUTOSHIP_RUNNER.log
        echo "STUCK" > status
        autoship_capture_failure model_failure "$issue_id" "error_summary=opencode CLI not found"
      fi
    ) &
    printf '%s\n' "$!" > "$dir/worker.pid"
    echo "Started $(basename "$dir") with $model"
  fi
  started=$((started + 1))
done

echo "Runner started $started workspace(s)"
