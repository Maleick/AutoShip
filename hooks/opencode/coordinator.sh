#!/usr/bin/env bash
# AutoShip Coordinator — starts an OpenCode session that manages subagent workers
# Replaces fire-and-forget `opencode run` with task-tool subagent dispatch.
#
# Usage:
#   bash hooks/opencode/coordinator.sh [--dry-run] [--once]
#
# Options:
#   --dry-run   Log what would be done but don't start coordinator
#   --once      Process one batch of workspaces then exit

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
AUTOSHIP_DIR="${AUTOSHIP_DIR:-.autoship}"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
WORKSPACES_DIR="$REPO_ROOT/$AUTOSHIP_DIR/workspaces"
STATE_FILE="$REPO_ROOT/$AUTOSHIP_DIR/state.json"
CONFIG_FILE="$REPO_ROOT/$AUTOSHIP_DIR/config.json"
LOG_FILE="$REPO_ROOT/$AUTOSHIP_DIR/coordinator.log"
COORDINATOR_PROMPT="$REPO_ROOT/$AUTOSHIP_DIR/COORDINATOR_PROMPT.md"
COORDINATOR_MODEL="${AUTOSHIP_COORDINATOR_MODEL:-opencode/big-pickle}"

DRY_RUN=false
ONCE=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=true; shift ;;
    --once) ONCE=true; shift ;;
    -h|--help)
      echo "Usage: $(basename "$0") [--dry-run] [--once]"
      exit 0 ;;
    *) echo "Unknown: $1"; exit 1 ;;
  esac
done

log() { echo "$(date -u '+%Y-%m-%dT%H:%M:%SZ') $1" | tee -a "$LOG_FILE"; }

# Resolve max concurrency
MAX_CONCURRENT=5
if [[ -f "$CONFIG_FILE" ]]; then
  config_max=$(jq -r '.max_workers // .maxConcurrent // .max_concurrent // 5' "$CONFIG_FILE" 2>/dev/null || echo 5)
  [[ "$config_max" =~ ^[0-9]+$ && "$config_max" -gt 0 ]] && MAX_CONCURRENT=$config_max
fi

log "Coordinator starting (model=$COORDINATOR_MODEL, max_concurrent=$MAX_CONCURRENT)"

# Scan for QUEUED workspaces
queued=()
if [[ -d "$WORKSPACES_DIR" ]]; then
  for ws_dir in "$WORKSPACES_DIR"/*/; do
    [[ -d "$ws_dir" ]] || continue
    s=$(tr -d '[:space:]' <"$ws_dir/status" 2>/dev/null || true)
    [[ -f "$ws_dir/AUTOSHIP_PROMPT.md" && "$s" == "QUEUED" ]] && queued+=("$(basename "$ws_dir")")
  done
fi

if [[ ${#queued[@]} -eq 0 ]]; then
  log "No QUEUED workspaces. Exiting."
  exit 0
fi

log "Found ${#queued[@]} QUEUED: ${queued[*]}"

batch=("${queued[@]:0:$MAX_CONCURRENT}")
[[ "$DRY_RUN" == "true" ]] && { log "DRY-RUN: would start for ${batch[*]}"; exit 0; }

# Generate coordinator prompt
{
  cat <<PROMPT_HEADER
You are the AutoShip Coordinator for $REPO_ROOT.

Process queued workspaces by spawning implementer subagents via \`task\`.

## Instructions

For each workspace:
1. Read its \`AUTOSHIP_PROMPT.md\` and \`model\` file
2. Set status: \`echo "RUNNING" > <workspace>/status\`
3. Write started_at: \`date -u +%Y-%m-%dT%H:%M:%SZ > <workspace>/started_at\`
4. Call \`task(subagent_type="general", prompt=<prompt>)\`
5. On return, write task_id: \`echo "<task_id>" > <workspace>/task_id\`
6. Read workspace/status — COMPLETE or STUCK
7. Report summary when all done

## Workspaces

PROMPT_HEADER

  for issue_key in "${batch[@]}"; do
    echo "- $WORKSPACES_DIR/$issue_key"
  done

  echo ""
  echo "Concurrency cap: $MAX_CONCURRENT"
  echo "Spare multiple subagents concurrently by calling task multiple times in one message."
} > "$COORDINATOR_PROMPT"

log "Starting coordinator with model=$COORDINATOR_MODEL"
cd "$REPO_ROOT"
opencode run --model "$COORDINATOR_MODEL" "$(cat "$COORDINATOR_PROMPT")" 2>&1 | tee -a "$LOG_FILE"

coordinator_exit=$?
log "Coordinator exited with code $coordinator_exit"

[[ "$ONCE" == "false" ]] && { log "Re-scanning..."; exec bash "$0" --once; }
exit $coordinator_exit
