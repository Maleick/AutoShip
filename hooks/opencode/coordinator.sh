#!/usr/bin/env bash
# AutoShip Coordinator — entry point that runs the coordinator agent skill
# The coordinator agent handles scanning, dispatching, and monitoring.
#
# Usage:
#   bash hooks/opencode/coordinator.sh [--once]
#
# Options:
#   --once      Process queued workspaces then exit (no re-scan loop)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
AUTOSHIP_DIR="${AUTOSHIP_DIR:-.autoship}"
REPO_ROOT="${AUTOSHIP_REPO_ROOT:-$(cd "$SCRIPT_DIR/../../.." && pwd)}"
LOG_FILE="$REPO_ROOT/$AUTOSHIP_DIR/coordinator.log"
SKILL_FILE="$SCRIPT_DIR/../../skills/autoship-coordinator/SKILL.md"
COORDINATOR_MODEL="${AUTOSHIP_COORDINATOR_MODEL:-kimi-for-coding/k2p6}"

ONCE=false
[[ "${1:-}" == "--once" ]] && ONCE=true

log() { echo "$(date -u '+%Y-%m-%dT%H:%M:%SZ') $1" | tee -a "$LOG_FILE"; }

log "Coordinator starting (model=$COORDINATOR_MODEL)"
cd "$REPO_ROOT"
opencode run --model "$COORDINATOR_MODEL" "$(cat "$SKILL_FILE")" 2>&1 | tee -a "$LOG_FILE"

coordinator_exit=$?
log "Coordinator exited with code $coordinator_exit"

[[ "$ONCE" == "false" ]] && { log "Re-scanning..."; exec bash "$0" --once; }
exit $coordinator_exit
