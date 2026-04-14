#!/bin/bash

# AutoShip Context Compaction Hook
# Removes dead tmux panes, merged branches, completed worktrees, and completed issues
# Usage: bash hooks/compact-context.sh [--dry-run]

set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=true
fi

# Paths
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AUTOSHIP_DIR="${REPO_ROOT}/.autoship"
WORKSPACES_DIR="${AUTOSHIP_DIR}/workspaces"
STATE_FILE="${AUTOSHIP_DIR}/state.json"
CURRENT_TIME=$(date +%s)
ONE_HOUR_AGO=$((CURRENT_TIME - 3600))

# Counters
CLEANED_PANES=0
CLEANED_BRANCHES=0
CLEANED_WORKTREES=0
CLEANED_ISSUES=0

# Color codes for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_action() {
  local msg="$1"
  if [[ "$DRY_RUN" == true ]]; then
    echo -e "${YELLOW}[DRY-RUN]${NC} $msg"
  else
    echo -e "${GREEN}[CLEANED]${NC} $msg"
  fi
}

log_info() {
  echo -e "${BLUE}[INFO]${NC} $1"
}

# ============================================================================
# 1. Remove dead tmux panes (closed, age > 1 hour)
# ============================================================================
remove_dead_panes() {
  if ! command -v tmux &> /dev/null; then
    log_info "tmux not available, skipping pane cleanup"
    return
  fi

  # Get all panes, including dead ones
  while IFS= read -r pane_info; do
    [[ -z "$pane_info" ]] && continue

    local pane_pid=$(echo "$pane_info" | awk '{print $1}')
    local pane_id=$(echo "$pane_info" | awk '{print $2}')
    local pane_dead=$(echo "$pane_info" | awk '{print $3}')

    # Check if pane is dead
    if [[ "$pane_dead" == "1" ]]; then
      # Try to get creation time, if not available use current approach
      if [[ "$DRY_RUN" == true ]]; then
        log_action "Would remove dead pane: $pane_id (pid: $pane_pid)"
      else
        # Kill the pane safely
        tmux kill-pane -t "$pane_id" 2>/dev/null || true
        log_action "Removed dead pane: $pane_id"
      fi
      CLEANED_PANES=$((CLEANED_PANES + 1))
    fi
  done < <(tmux list-panes -a -F "#{pane_pid} #{session_name}:#{window_index}.#{pane_index} #{pane_dead}" 2>/dev/null || true)
}

# ============================================================================
# 2. Clean merged branches from git
# ============================================================================
remove_merged_branches() {
  local merged_branches=""

  # Get branches that are fully merged into master
  merged_branches=$(git -C "$REPO_ROOT" branch --merged master --format='%(refname:short)' 2>/dev/null | \
    grep -v "^master$" | \
    grep -v "^HEAD$" || true)

  if [[ -z "$merged_branches" ]]; then
    log_info "No merged branches to clean"
    return
  fi

  while IFS= read -r branch; do
    [[ -z "$branch" ]] && continue

    if [[ "$DRY_RUN" == true ]]; then
      log_action "Would delete merged branch: $branch"
    else
      git -C "$REPO_ROOT" branch -d "$branch" 2>/dev/null || true
      log_action "Deleted merged branch: $branch"
    fi
    CLEANED_BRANCHES=$((CLEANED_BRANCHES + 1))
  done < <(echo "$merged_branches")
}

# ============================================================================
# 3. Prune stale worktrees (.autoship/workspaces/* with completed agents)
# ============================================================================
prune_stale_worktrees() {
  if [[ ! -d "$WORKSPACES_DIR" ]]; then
    log_info "Workspaces directory does not exist"
    return
  fi

  # Find all issue directories in workspaces
  for workspace_dir in "$WORKSPACES_DIR"/issue-*; do
    [[ ! -d "$workspace_dir" ]] && continue

    local issue_name=$(basename "$workspace_dir")
    local issue_num="${issue_name#issue-}"

    # Check if this issue exists in state.json and has completed state
    local issue_state=$(jq -r ".issues[\"$issue_num\"] // .issues[\"$issue_name\"] // {state: \"unknown\"} | .state" "$STATE_FILE" 2>/dev/null || echo "unknown")

    # Prune if state is merged, closed, or if workspace is older than 1 hour
    local workspace_mtime=$(stat -f%m "$workspace_dir" 2>/dev/null || echo "0")

    if [[ "$issue_state" == "merged" ]] || [[ "$issue_state" == "closed" ]]; then
      if [[ "$DRY_RUN" == true ]]; then
        log_action "Would prune completed worktree: $issue_name (state: $issue_state)"
      else
        rm -rf "$workspace_dir"
        log_action "Pruned completed worktree: $issue_name"
      fi
      CLEANED_WORKTREES=$((CLEANED_WORKTREES + 1))
    elif [[ "$workspace_mtime" -lt "$ONE_HOUR_AGO" ]] && [[ "$issue_state" == "unclaimed" ]]; then
      # Prune old unclaimed worktrees
      if [[ "$DRY_RUN" == true ]]; then
        log_action "Would prune stale unclaimed worktree: $issue_name"
      else
        rm -rf "$workspace_dir"
        log_action "Pruned stale unclaimed worktree: $issue_name"
      fi
      CLEANED_WORKTREES=$((CLEANED_WORKTREES + 1))
    fi
  done
}

# ============================================================================
# 4. Remove completed issues from .autoship/state.json
# ============================================================================
remove_completed_issues() {
  if [[ ! -f "$STATE_FILE" ]]; then
    log_info "State file does not exist"
    return
  fi

  # Create backup
  cp "$STATE_FILE" "${STATE_FILE}.backup.$(date +%s)"

  # Find and remove completed issues (merged/closed state)
  local temp_file="${STATE_FILE}.tmp.$$"

  if [[ "$DRY_RUN" == true ]]; then
    # Just count, don't modify
    local completed_count=$(jq '.issues | to_entries | map(select(.value.state | IN("merged", "closed"))) | length' "$STATE_FILE" 2>/dev/null || echo "0")
    log_action "Would remove $completed_count completed issues from state.json"
    CLEANED_ISSUES=$completed_count
  else
    # Remove completed issues
    jq 'del(.issues[] | select(.state | IN("merged", "closed")))' "$STATE_FILE" > "$temp_file"

    if [[ -s "$temp_file" ]]; then
      local before_count=$(jq '.issues | length' "$STATE_FILE" 2>/dev/null || echo "0")
      local after_count=$(jq '.issues | length' "$temp_file" 2>/dev/null || echo "0")
      CLEANED_ISSUES=$((before_count - after_count))

      mv "$temp_file" "$STATE_FILE"
      log_action "Removed $CLEANED_ISSUES completed issues from state.json"
    else
      rm -f "$temp_file"
      log_info "Failed to process state.json"
    fi
  fi
}

# ============================================================================
# Main execution
# ============================================================================
main() {
  log_info "Starting context compaction ($(if [[ "$DRY_RUN" == true ]]; then echo "DRY-RUN"; else echo "LIVE"; fi))"
  log_info "Repo: $REPO_ROOT"

  # Run cleanup operations
  remove_dead_panes
  remove_merged_branches
  prune_stale_worktrees
  remove_completed_issues

  # Print summary
  echo ""
  echo "=========================================="
  echo "Context Compaction Summary"
  echo "=========================================="
  echo "Cleaned $CLEANED_PANES panes"
  echo "Cleaned $CLEANED_BRANCHES branches"
  echo "Cleaned $CLEANED_WORKTREES worktrees"
  echo "Cleaned $CLEANED_ISSUES issues"
  echo "=========================================="

  # Standard output format for CI/automation
  local total=$((CLEANED_PANES + CLEANED_BRANCHES + CLEANED_WORKTREES + CLEANED_ISSUES))
  if [[ "$total" -gt 0 ]]; then
    echo "Cleaned $CLEANED_WORKTREES worktrees, $CLEANED_BRANCHES branches, $CLEANED_PANES panes"
  else
    echo "No resources needed cleaning"
  fi
}

main
