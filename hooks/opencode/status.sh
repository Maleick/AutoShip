#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) REPO_ROOT="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

AUTOSHIP_DIR="$REPO_ROOT/.autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"

if [[ ! -f "$STATE_FILE" ]]; then
  cat <<'EOF'
═══════════════════════════════════════════
              AUTOSHIP STATUS
═══════════════════════════════════════════
No active AutoShip session.

Run /autoship to start orchestration.
═══════════════════════════════════════════
EOF
  exit 0
fi

max=$(jq -r '.config.maxConcurrentAgents // .max_concurrent_agents // empty' "$STATE_FILE")
if [[ -z "$max" && -f "$AUTOSHIP_DIR/config.json" ]]; then
  max=$(jq -r '.maxConcurrentAgents // .max_agents // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)
fi
max="${max:-15}"
repo=$(jq -r '.repo // "unknown"' "$STATE_FILE")
active=0
if [[ -d "$WORKSPACES_DIR" ]]; then
  active=$((grep -Rsl '^RUNNING$' "$WORKSPACES_DIR"/*/status 2>/dev/null || true) | wc -l | tr -d ' ')
fi
queued=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "queued")] | length' "$STATE_FILE")
completed=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "completed" or (.value.state // .value.status) == "approved")] | length' "$STATE_FILE")
blocked=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "blocked")] | length' "$STATE_FILE")
stuck=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "stuck")] | length' "$STATE_FILE")
queue_depth=0
if [[ -f "$AUTOSHIP_DIR/event-queue.json" ]]; then
  queue_depth=$(jq 'length' "$AUTOSHIP_DIR/event-queue.json" 2>/dev/null || echo 0)
fi

workspace_counts=$(mktemp)
trap 'rm -f "$workspace_counts"' EXIT
if [[ -d "$WORKSPACES_DIR" ]]; then
  for dir in "$WORKSPACES_DIR"/*/; do
    [[ -d "$dir" ]] || continue
    if [[ -f "$dir/status" ]]; then
      tr -d '[:space:]' < "$dir/status"
      printf '\n'
    else
      printf 'UNKNOWN\n'
    fi
  done > "$workspace_counts"
else
  : > "$workspace_counts"
fi

running_ws=$(grep -c '^RUNNING$' "$workspace_counts" 2>/dev/null || true)
queued_ws=$(grep -c '^QUEUED$' "$workspace_counts" 2>/dev/null || true)
complete_ws=$(grep -c '^COMPLETE$' "$workspace_counts" 2>/dev/null || true)
blocked_ws=$(grep -c '^BLOCKED$' "$workspace_counts" 2>/dev/null || true)
stuck_ws=$(grep -c '^STUCK$' "$workspace_counts" 2>/dev/null || true)

cat <<EOF
═══════════════════════════════════════════
              AUTOSHIP STATUS
═══════════════════════════════════════════
Repo:        $repo
Queue depth: $queue_depth

AGENTS ($active active / $max max)
───────────────────────────────────────────
Running:   $active
Queued:    $queued
Completed: $completed
Blocked:   $blocked
Stuck:     $stuck

WORKSPACES
───────────────────────────────────────────
RUNNING:   $running_ws
QUEUED:    $queued_ws
COMPLETE:  $complete_ws
BLOCKED:   $blocked_ws
STUCK:     $stuck_ws
═══════════════════════════════════════════
EOF
