#!/usr/bin/env bash
# AutoShip auto-prune — monitor disk usage and prune when thresholds exceeded
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Default thresholds (override via env vars)
MAX_WORKTREE_SIZE_GB="${AUTOSHIP_MAX_WORKTREE_SIZE_GB:-2}"      # Max size per worktree
MAX_TOTAL_WORKTREES_GB="${AUTOSHIP_MAX_TOTAL_WORKTREES_GB:-10}" # Max total for all worktrees
MAX_WORKSPACE_COUNT="${AUTOSHIP_MAX_WORKSPACE_COUNT:-20}"       # Max .autoship workspaces
MAX_WORKSPACE_AGE_DAYS="${AUTOSHIP_MAX_WORKSPACE_AGE_DAYS:-7}"  # Auto-remove after N days

TARGET_REPO="${HERMES_TARGET_REPO_PATH:-/Users/maleick/Projects/TextQuest}"
WORKTREE_BASE="${TARGET_REPO}.worktrees"
AUTOSHIP_DIR="${AUTOSHIP_DIR:-/Users/maleick/Projects/AutoShip/.autoship}"

log() {
  echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

# Check if threshold exceeded
threshold_exceeded() {
  local current="$1"
  local threshold="$2"
  local name="$3"

  if (($(echo "$current > $threshold" | bc -l 2>/dev/null || echo "0"))); then
    log "⚠️ THRESHOLD EXCEEDED: $name = ${current}GB (limit: ${threshold}GB)"
    return 0
  fi
  return 1
}

# Get directory size in GB
get_size_gb() {
  local dir="$1"
  if [[ -d "$dir" ]]; then
    local raw
    raw=$(du -sh "$dir" 2>/dev/null | awk '{print $1}')
    case "$raw" in
      *G) echo "${raw%G}" ;;
      *M) echo "$(echo "${raw%M} * 0.001" | bc -l 2>/dev/null || echo "0")" ;;
      *K) echo "$(echo "${raw%K} * 0.000001" | bc -l 2>/dev/null || echo "0")" ;;
      *) echo "$raw" ;;
    esac
  else
    echo "0"
  fi
}

# Prune oversized worktrees
prune_oversized_worktrees() {
  log "Checking worktree sizes (limit: ${MAX_WORKTREE_SIZE_GB}GB each)..."

  local pruned=0
  for wt in "$WORKTREE_BASE"/issue-*; do
    [[ -d "$wt" ]] || continue
    local issue_num
    issue_num=$(basename "$wt" | sed 's/issue-//')
    # Only process numeric issue directories
    if [[ ! "$issue_num" =~ ^[0-9]+$ ]]; then
      log "  Skipping non-numeric worktree: $wt"
      continue
    fi

    local size_gb
    size_gb=$(get_size_gb "$wt")
    if (($(echo "$size_gb > $MAX_WORKTREE_SIZE_GB" | bc -l 2>/dev/null || echo "0"))); then
      log "Pruning oversized worktree: $wt (${size_gb}GB > ${MAX_WORKTREE_SIZE_GB}GB)"

      # Check if active (has RUNNING or QUEUED status)
      local issue_num=$(basename "$wt" | sed 's/issue-//')
      local ws_status=""
      if [[ -f "$AUTOSHIP_DIR/workspaces/issue-$issue_num/status" ]]; then
        ws_status=$(cat "$AUTOSHIP_DIR/workspaces/issue-$issue_num/status" 2>/dev/null || echo "UNKNOWN")
      fi

      if [[ "$ws_status" == "RUNNING" ]]; then
        log "  ⚠️ Worktree is RUNNING, skipping: $wt"
        continue
      fi

      # Safe to remove
      cd "$TARGET_REPO" && git worktree remove "$wt" --force 2>/dev/null || rm -rf "$wt" 2>/dev/null || true
      ((pruned++)) || true
    fi
  done

  log "Pruned $pruned oversized worktrees"
  return $pruned
}

# Prune old workspaces
prune_old_workspaces() {
  log "Checking workspace age (limit: ${MAX_WORKSPACE_AGE_DAYS} days)..."

  local pruned=0
  local cutoff_epoch=$(($(date +%s) - MAX_WORKSPACE_AGE_DAYS * 86400))

  for ws in "$AUTOSHIP_DIR"/workspaces/issue-*; do
    [[ -d "$ws" ]] || continue

    local mtime_epoch=$(stat -f%m "$ws" 2>/dev/null || stat -c%Y "$ws" 2>/dev/null || echo "0")
    if [[ "$mtime_epoch" -lt "$cutoff_epoch" ]]; then
      local issue_num=$(basename "$ws" | sed 's/issue-//')
      local status=$(cat "$ws/status" 2>/dev/null || echo "UNKNOWN")

      if [[ "$status" == "RUNNING" ]]; then
        log "  Skipping RUNNING workspace: issue-$issue_num"
        continue
      fi

      log "Pruning old workspace: issue-$issue_num (${status}, age > ${MAX_WORKSPACE_AGE_DAYS} days)"
      rm -rf "$ws"
      ((pruned++)) || true
    fi
  done

  log "Pruned $pruned old workspaces"
  return $pruned
}

# Prune by total size
prune_by_total_size() {
  local total_gb=$(get_size_gb "$WORKTREE_BASE")

  if ! threshold_exceeded "$total_gb" "$MAX_TOTAL_WORKTREES_GB" "Total worktrees"; then
    return 0
  fi

  log "Total worktree size ${total_gb}GB exceeds ${MAX_TOTAL_WORKTREES_GB}GB, pruning oldest..."

  # Sort by modification time, oldest first
  local pruned=0
  for wt in $(ls -1td "$WORKTREE_BASE"/issue-* 2>/dev/null | tail -r); do
    [[ -d "$wt" ]] || continue

    local issue_num=$(basename "$wt" | sed 's/issue-//')
    local status=""
    if [[ -f "$AUTOSHIP_DIR/workspaces/issue-$issue_num/status" ]]; then
      status=$(cat "$AUTOSHIP_DIR/workspaces/issue-$issue_num/status" 2>/dev/null)
    fi

    if [[ "$status" == "RUNNING" ]]; then
      continue
    fi

    log "Removing worktree to free space: $wt"
    cd "$TARGET_REPO" && git worktree remove "$wt" --force 2>/dev/null || rm -rf "$wt" 2>/dev/null || true
    ((pruned++)) || true

    # Check if we're under threshold now
    total_gb=$(get_size_gb "$WORKTREE_BASE")
    if ! threshold_exceeded "$total_gb" "$MAX_TOTAL_WORKTREES_GB" "Total worktrees"; then
      break
    fi
  done

  log "Pruned $pruned worktrees to get under ${MAX_TOTAL_WORKTREES_GB}GB"
}

# Prune by workspace count
prune_by_workspace_count() {
  # Count actual directories, not files inside them
  local count=$(find "$AUTOSHIP_DIR"/workspaces -maxdepth 1 -type d -name "issue-*" 2>/dev/null | wc -l | tr -d ' ')

  if [[ "$count" -le "$MAX_WORKSPACE_COUNT" ]]; then
    return 0
  fi

  log "Workspace count $count exceeds $MAX_WORKSPACE_COUNT, pruning oldest..."

  local to_remove=$((count - MAX_WORKSPACE_COUNT))
  local removed=0

  # Sort by mtime oldest first, only directories
  for ws in $(find "$AUTOSHIP_DIR"/workspaces -maxdepth 1 -type d -name "issue-*" -print0 2>/dev/null | xargs -0 stat -f "%m %N" 2>/dev/null | sort -n | head -$to_remove | awk '{print $2}'); do
    [[ -d "$ws" ]] || continue

    local issue_num=$(basename "$ws" | sed 's/issue-//')
    local status=$(cat "$ws/status" 2>/dev/null || echo "UNKNOWN")

    if [[ "$status" == "RUNNING" || "$status" == "QUEUED" ]]; then
      log "Skipping active workspace: issue-$issue_num (${status})"
      continue
    fi

    log "Removing workspace: issue-$issue_num (${status})"
    rm -rf "$ws"
    ((removed++)) || true
  done

  log "Removed $removed workspaces to get under $MAX_WORKSPACE_COUNT"
}

# Main auto-prune logic
main() {
  log "=== AutoShip Auto-Prune ==="
  log "Thresholds: worktree=${MAX_WORKTREE_SIZE_GB}GB, total=${MAX_TOTAL_WORKTREES_GB}GB, workspaces=${MAX_WORKSPACE_COUNT}, age=${MAX_WORKSPACE_AGE_DAYS}d"

  local total_pruned=0

  # Run all prune strategies
  prune_oversized_worktrees
  total_pruned=$((total_pruned + $?))

  prune_old_workspaces
  total_pruned=$((total_pruned + $?))

  prune_by_total_size

  prune_by_workspace_count

  # Final git prune
  cd "$TARGET_REPO" && git worktree prune 2>/dev/null || true

  # Report current state
  local current_total=$(get_size_gb "$WORKTREE_BASE")
  local current_count=$(ls -1 "$AUTOSHIP_DIR"/workspaces/issue-* 2>/dev/null | wc -l | tr -d ' ')

  log "=== Auto-Prune Complete ==="
  log "Current state: ${current_total}GB total, $current_count workspaces"
  log "Total items pruned this run: $total_pruned"

  # Return non-zero if we pruned anything (useful for cron notifications)
  if [[ "$total_pruned" -gt 0 ]]; then
    return 1
  fi
  return 0
}

# Run if called directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi
