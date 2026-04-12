#!/usr/bin/env bash
set -euo pipefail

# sweep-stale.sh — Scan .beacon/workspaces for stale worktrees and clean them up.
# A worktree is considered stale if its corresponding issue is in a terminal state
# (merged, blocked, approved) and should be cleaned up automatically.

# Locate repo root
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

# Resolve sibling scripts
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

WORKSPACES_DIR=".beacon/workspaces"
STATE_FILE=".beacon/state.json"

# Check if workspaces directory exists
if [[ ! -d "$WORKSPACES_DIR" ]]; then
  echo "No workspaces directory found at $WORKSPACES_DIR"
  exit 0
fi

# Check if state file exists
if [[ ! -f "$STATE_FILE" ]]; then
  echo "No state file found at $STATE_FILE; skipping sweep"
  exit 0
fi

# Verify jq is available
if ! command -v jq >/dev/null 2>&1; then
  echo "Warning: jq not available; skipping sweep"
  exit 0
fi

# Terminal states that indicate a worktree can be cleaned up
# merged: PR was merged, work is done
TERMINAL_STATES="merged"

# Active pipeline states that must never be swept automatically
# running: agent is executing
# claimed: issue has been claimed and is about to start
# verifying: reviewer is checking the work
# approved: work passed review, waiting for merge
# blocked: partial work/logs exist; operator must resolve manually
PROTECTED_STATES="running claimed verifying approved blocked"

# --- Error Recovery #3: Stale worktree detection ---
# Also remove worktrees for issues that have no active tmux pane AND are not
# in a running state in state.json. These are orphaned from crashed sessions.
is_pane_active() {
  local pane_id="$1"
  [[ -z "$pane_id" ]] && return 1
  command -v tmux >/dev/null 2>&1 || return 1
  tmux list-panes -a -F '#{pane_id}' 2>/dev/null | grep -q "^${pane_id}$"
}

LOG_FILE=".beacon/poll.log"

# Iterate over worktree directories
CLEANED_COUNT=0
shopt -s nullglob
for worktree_dir in "$WORKSPACES_DIR"/*/; do
  # Extract issue key from directory name (e.g., ".beacon/workspaces/issue-16" → "issue-16")
  ISSUE_KEY=$(basename "$worktree_dir")

  # Look up the issue state in state.json
  ISSUE_STATE=$(jq -r --arg id "$ISSUE_KEY" '.issues[$id].state // "unknown"' "$STATE_FILE" 2>/dev/null) || ISSUE_STATE="unknown"

  # Check if this issue is in a terminal state
  IS_TERMINAL=0
  for state in $TERMINAL_STATES; do
    if [[ "$ISSUE_STATE" == "$state" ]]; then
      IS_TERMINAL=1
      break
    fi
  done

  if [[ $IS_TERMINAL -eq 1 ]]; then
    echo "Stale worktree detected: $ISSUE_KEY (state: $ISSUE_STATE)"
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] sweep-stale: removing terminal worktree $ISSUE_KEY (state=$ISSUE_STATE)" >> "$LOG_FILE" 2>/dev/null || true

    # Call cleanup-worktree.sh to handle the cleanup
    bash "$SCRIPT_DIR/cleanup-worktree.sh" "$ISSUE_KEY" 2>/dev/null || {
      echo "Warning: failed to clean up $ISSUE_KEY"
    }

    CLEANED_COUNT=$((CLEANED_COUNT + 1))
    continue
  fi

  # Also sweep orphaned worktrees: directory exists but issue is NOT in an active pipeline
  # state and its pane is dead. Protected states are never swept automatically.
  IS_PROTECTED=0
  for pstate in $PROTECTED_STATES; do
    if [[ "$ISSUE_STATE" == "$pstate" ]]; then
      IS_PROTECTED=1
      break
    fi
  done

  if [[ $IS_PROTECTED -eq 0 ]]; then
    PANE_ID=$(jq -r --arg id "$ISSUE_KEY" '.issues[$id].pane_id // empty' "$STATE_FILE" 2>/dev/null) || PANE_ID=""
    if ! is_pane_active "$PANE_ID"; then
      echo "Orphaned worktree detected: $ISSUE_KEY (state=$ISSUE_STATE, no active pane)"
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] sweep-stale: removing orphaned worktree $ISSUE_KEY (state=$ISSUE_STATE, pane_id=${PANE_ID:-none})" >> "$LOG_FILE" 2>/dev/null || true

      bash "$SCRIPT_DIR/cleanup-worktree.sh" "$ISSUE_KEY" 2>/dev/null || {
        echo "Warning: failed to clean up orphaned worktree $ISSUE_KEY"
      }
      CLEANED_COUNT=$((CLEANED_COUNT + 1))
    fi
  fi
done
shopt -u nullglob

if [[ $CLEANED_COUNT -gt 0 ]]; then
  echo "Swept and cleaned $CLEANED_COUNT stale worktree(s)"
else
  echo "No stale worktrees found"
fi
