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

mark_stuck_unless_terminal() {
  local current=""
  [[ -f status ]] && current=$(tr -d '[:space:]' < status)
  case "$current" in
    COMPLETE|BLOCKED|STUCK) ;;
    *) echo "STUCK" > status ;;
  esac
}

started=0
for dir in "$WORKSPACES_DIR"/*/; do
  [[ -d "$dir" ]] || continue
  status_file="$dir/status"
  prompt_file="$dir/AUTOSHIP_PROMPT.md"
  model_file="$dir/model"
  [[ -f "$status_file" && -f "$prompt_file" ]] || continue
  status=$(tr -d '[:space:]' < "$status_file")
  [[ "$status" == "QUEUED" ]] || continue

  active=$(active_count)
  if (( active >= MAX )); then
    echo "CAP_REACHED: $active active / $MAX max"
    break
  fi

  model="opencode/nemotron-3-super-free"
  [[ -f "$model_file" ]] && model=$(cat "$model_file")
  echo "RUNNING" > "$status_file"
  bash "$REPO_ROOT/hooks/update-state.sh" set-running "$(basename "$dir")" agent="$model" 2>/dev/null || true

  if [[ "$DRY_RUN" == "true" ]]; then
    echo "DRY_RUN start $(basename "$dir") with $model"
  else
    (
      cd "$dir"
      if command -v opencode >/dev/null 2>&1; then
        if run_worker "$model" > AUTOSHIP_RUNNER.log 2>&1; then
          mark_stuck_unless_terminal
        else
          echo "STUCK" > status
        fi
      else
        echo "opencode CLI not found" > AUTOSHIP_RUNNER.log
        echo "STUCK" > status
      fi
    ) &
    echo "Started $(basename "$dir") with $model"
  fi
  started=$((started + 1))
done

echo "Runner started $started workspace(s)"
