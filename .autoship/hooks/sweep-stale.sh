#!/usr/bin/env bash
set -euo pipefail

# sweep-stale.sh — Scan .autoship/workspaces for stale worktrees and clean them up.
# A worktree is considered stale if its corresponding issue is in a terminal state
# (merged, blocked, approved) and should be cleaned up automatically.
#
# Fix (issue #2224): Before closing any issue marked 'merged' in state.json,
# verify the linked PR is actually merged on GitHub via `gh pr view`.
# This prevents false-positive cleanup on session restart when state.json
# was written 'merged' without a real GitHub merge occurring.

# Locate repo root
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

# Resolve sibling scripts — prefer project-level hooks, fall back to plugin cache
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Find cleanup-worktree.sh: check project hooks override first, then plugin cache
find_sibling_script() {
  local script_name="$1"
  # Project-level override
  local project_hooks="$REPO_ROOT/.autoship/hooks/$script_name"
  [[ -f "$project_hooks" ]] && { echo "$project_hooks"; return; }
  # Plugin cache (same directory as caller script)
  local sibling="$SCRIPT_DIR/$script_name"
  [[ -f "$sibling" ]] && { echo "$sibling"; return; }
  echo ""
}

WORKSPACES_DIR=".autoship/workspaces"
STATE_FILE=".autoship/state.json"

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

# --- Fix #2224: PR merge verification ---
# Returns 0 (true) if the PR is confirmed merged on GitHub, 1 otherwise.
# If gh is not available or PR number is unknown, returns 1 (safe default: don't sweep).
verify_pr_merged() {
  local issue_key="$1"

  # Require gh CLI
  if ! command -v gh >/dev/null 2>&1; then
    echo "  [sweep-stale] gh CLI not available; skipping PR verification for $issue_key — will not sweep"
    return 1
  fi

  # Extract PR number from state.json (field may be pr_number or pr)
  local pr_number
  pr_number=$(jq -r --arg id "$issue_key" '
    .issues[$id]
    | (.pr_number // .pr)
    | if . == null or . == "" then "none" else tostring end
  ' "$STATE_FILE" 2>/dev/null) || pr_number="none"

  if [[ -z "$pr_number" || "$pr_number" == "none" || "$pr_number" == "null" ]]; then
    echo "  [sweep-stale] $issue_key: state=merged but no PR number in state.json — skipping sweep (re-queue for operator review)"
    return 1
  fi

  # Query GitHub for real PR state
  local gh_state
  gh_state=$(gh pr view "$pr_number" --json state --jq '.state' 2>/dev/null) || {
    echo "  [sweep-stale] $issue_key: gh pr view $pr_number failed (network/auth?) — skipping sweep"
    return 1
  }

  if [[ "$gh_state" == "MERGED" ]]; then
    echo "  [sweep-stale] $issue_key: PR #$pr_number confirmed MERGED on GitHub — safe to sweep"
    return 0
  else
    echo "  [sweep-stale] $issue_key: PR #$pr_number GitHub state='$gh_state' (not MERGED) — skipping sweep"
    return 1
  fi
}

LOG_FILE=".autoship/poll.log"

write_swept_state() {
  local issue_key="$1"
  local tmp_state state_mode state_uid state_gid

  state_mode=$(stat -c '%a' "$STATE_FILE" 2>/dev/null || stat -f '%Lp' "$STATE_FILE" 2>/dev/null || true)
  state_uid=$(stat -c '%u' "$STATE_FILE" 2>/dev/null || stat -f '%u' "$STATE_FILE" 2>/dev/null || true)
  state_gid=$(stat -c '%g' "$STATE_FILE" 2>/dev/null || stat -f '%g' "$STATE_FILE" 2>/dev/null || true)
  tmp_state=$(mktemp "${STATE_FILE}.tmp.XXXXXX" 2>/dev/null) || return 0
  if [[ -n "$state_mode" ]]; then
    chmod "$state_mode" "$tmp_state" 2>/dev/null || true
  fi
  if [[ -n "$state_uid" && -n "$state_gid" ]]; then
    chown "${state_uid}:${state_gid}" "$tmp_state" 2>/dev/null || true
  fi
  jq --arg key "$issue_key" '.issues[$key].swept = true' "$STATE_FILE" > "$tmp_state" &&
    mv "$tmp_state" "$STATE_FILE" 2>/dev/null || true
  rm -f "$tmp_state" 2>/dev/null || true
}

mark_issue_swept() {
  local issue_key="$1"

  if command -v flock >/dev/null 2>&1; then
    exec 9<"$STATE_FILE"
    flock -x 9
    write_swept_state "$issue_key"
    exec 9>&-
  elif command -v lockf >/dev/null 2>&1; then
    lockf -k "$STATE_FILE" bash -c '
      state_file="$1" issue_key="$2"
      state_mode=$(stat -c '"'"'%a'"'"' "$state_file" 2>/dev/null || stat -f '"'"'%Lp'"'"' "$state_file" 2>/dev/null || true)
      state_uid=$(stat -c '"'"'%u'"'"' "$state_file" 2>/dev/null || stat -f '"'"'%u'"'"' "$state_file" 2>/dev/null || true)
      state_gid=$(stat -c '"'"'%g'"'"' "$state_file" 2>/dev/null || stat -f '"'"'%g'"'"' "$state_file" 2>/dev/null || true)
      tmp_state=$(mktemp "${state_file}.tmp.XXXXXX" 2>/dev/null) || exit 0
      if [[ -n "$state_mode" ]]; then
        chmod "$state_mode" "$tmp_state" 2>/dev/null || true
      fi
      if [[ -n "$state_uid" && -n "$state_gid" ]]; then
        chown "${state_uid}:${state_gid}" "$tmp_state" 2>/dev/null || true
      fi
      jq --arg key "$issue_key" '"'"'.issues[$key].swept = true'"'"' "$state_file" > "$tmp_state" &&
        mv "$tmp_state" "$state_file" 2>/dev/null || true
      rm -f "$tmp_state" 2>/dev/null || true
    ' _ "$STATE_FILE" "$issue_key"
  else
    write_swept_state "$issue_key"
  fi
}

# Locate cleanup-worktree.sh — needed for both terminal and orphan sweep paths
CLEANUP_SCRIPT=$(find_sibling_script "cleanup-worktree.sh")
if [[ -z "$CLEANUP_SCRIPT" ]]; then
  # Fall back to plugin cache path via known directory structure
  CLEANUP_SCRIPT="$SCRIPT_DIR/cleanup-worktree.sh"
fi

# Iterate over worktree directories
CLEANED_COUNT=0
SKIPPED_UNVERIFIED=0
shopt -s nullglob
for worktree_dir in "$WORKSPACES_DIR"/*/; do
  # Extract issue key from directory name (e.g., ".autoship/workspaces/issue-16" → "issue-16")
  ISSUE_KEY=$(basename "$worktree_dir")

  # Check if this issue was already swept in this cycle; skip if so
  swept=$(jq -r --arg key "$ISSUE_KEY" '.issues[$key].swept // false' "$STATE_FILE" 2>/dev/null) || swept="false"
  [[ "$swept" == "true" ]] && continue

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
    echo "Terminal state detected: $ISSUE_KEY (state: $ISSUE_STATE) — verifying GitHub PR before sweep"

    # Fix #2224: verify PR is actually merged on GitHub before sweeping
    if ! verify_pr_merged "$ISSUE_KEY"; then
      echo "  Skipping cleanup of $ISSUE_KEY — PR not confirmed merged on GitHub"
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] sweep-stale: SKIP $ISSUE_KEY (state=merged but PR not confirmed via gh)" >> "$LOG_FILE" 2>/dev/null || true
      SKIPPED_UNVERIFIED=$((SKIPPED_UNVERIFIED + 1))
      continue
    fi

    echo "Stale worktree confirmed: $ISSUE_KEY (state: $ISSUE_STATE, PR verified merged)"
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] sweep-stale: removing terminal worktree $ISSUE_KEY (state=$ISSUE_STATE, PR verified)" >> "$LOG_FILE" 2>/dev/null || true

    # Call cleanup-worktree.sh to handle the cleanup
    bash "$CLEANUP_SCRIPT" "$ISSUE_KEY" 2>/dev/null || {
      echo "Warning: failed to clean up $ISSUE_KEY"
    }

    # Write swept sentinel to prevent re-processing
    mark_issue_swept "$ISSUE_KEY"

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

      bash "$CLEANUP_SCRIPT" "$ISSUE_KEY" 2>/dev/null || {
        echo "Warning: failed to clean up orphaned worktree $ISSUE_KEY"
      }

      # Write swept sentinel to prevent re-processing
      mark_issue_swept "$ISSUE_KEY"

      CLEANED_COUNT=$((CLEANED_COUNT + 1))
    fi
  fi
done
shopt -u nullglob

if [[ $CLEANED_COUNT -gt 0 || $SKIPPED_UNVERIFIED -gt 0 ]]; then
  echo "Swept and cleaned $CLEANED_COUNT stale worktree(s); skipped $SKIPPED_UNVERIFIED unverified"
else
  echo "No stale worktrees found"
fi
