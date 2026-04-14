#!/usr/bin/env bash
set -euo pipefail

# cleanup-worktree.sh — Clean up worktree, branch, labels, and state after merge.
# Usage: cleanup-worktree.sh <issue-key>
# Example: cleanup-worktree.sh issue-16

ISSUE_KEY="${1:-}"
if [[ -z "$ISSUE_KEY" ]]; then
  echo "Usage: $0 <issue-key>" >&2
  echo "Example: $0 issue-16" >&2
  exit 1
fi

# Validate ISSUE_KEY to prevent path traversal
if [[ ! "$ISSUE_KEY" =~ ^issue-[0-9]+[a-z0-9-]*$ ]]; then
  echo "Error: invalid ISSUE_KEY: $ISSUE_KEY" >&2
  exit 1
fi

# Locate repo root
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

# Resolve sibling scripts
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Paths
WORKTREE_PATH=".autoship/workspaces/$ISSUE_KEY"
AUTOSHIP_BRANCH="autoship/$ISSUE_KEY"
ISSUE_NUM=$(echo "${ISSUE_KEY#issue-}" | sed 's/[^0-9].*//')

# Resolve repo slug from state.json or git remote (needed for archive and GitHub cleanup)
REPO_SLUG=""
if [[ -f ".autoship/state.json" ]]; then
  REPO_SLUG=$(jq -r '.repo // empty' ".autoship/state.json" 2>/dev/null) || true
fi
if [[ -z "$REPO_SLUG" ]]; then
  REMOTE_URL=$(git remote get-url origin 2>/dev/null) || true
  if [[ -n "$REMOTE_URL" ]]; then
    REPO_SLUG=$(echo "$REMOTE_URL" | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
  fi
fi

# Archive AUTOSHIP_RESULT.md before removing worktree
RESULT_FILE="$WORKTREE_PATH/AUTOSHIP_RESULT.md"
# Fallback: Phase-1 / no-worktree agents write to the repo root
if [[ ! -f "$RESULT_FILE" ]] && [[ -f "AUTOSHIP_RESULT.md" ]]; then
  echo "Warning: AUTOSHIP_RESULT.md not found in worktree, falling back to repo root" >> .autoship/poll.log
  RESULT_FILE="AUTOSHIP_RESULT.md"
fi

# SECURITY: Check for symlink escape vulnerability
if [[ -L "$RESULT_FILE" ]]; then
  echo "SECURITY ERROR: $RESULT_FILE is a symlink — rejecting to prevent escape" >> .autoship/poll.log
  bash "$(cat .autoship/hooks_dir)/update-state.sh" set-blocked "$ISSUE_NUM" escalation_reason="symlink_in_result_file" 2>/dev/null || true
  echo "ERROR: Result file is a symlink — blocked for security review" >&2
  exit 1
fi

# Validate realpath stays within worktree bounds
_REAL_PATH=$(cd "$(dirname "$RESULT_FILE")" && pwd -P && cd - > /dev/null)
_REAL_PATH="$_REAL_PATH/$(basename "$RESULT_FILE")"
_WORKTREE_REAL=$(cd "$WORKTREE_PATH" 2>/dev/null && pwd -P)
if [[ -n "$_WORKTREE_REAL" ]] && [[ ! "$_REAL_PATH" =~ ^"$_WORKTREE_REAL" ]]; then
  echo "SECURITY ERROR: $RESULT_FILE resolves outside worktree bounds" >> .autoship/poll.log
  echo "  Expected path within: $_WORKTREE_REAL" >> .autoship/poll.log
  echo "  Actual resolved path: $_REAL_PATH" >> .autoship/poll.log
  bash "$(cat .autoship/hooks_dir)/update-state.sh" set-blocked "$ISSUE_NUM" escalation_reason="path_escape_in_result_file" 2>/dev/null || true
  echo "ERROR: Result file path escapes worktree — blocked for security review" >&2
  exit 1
fi

# Validate — must start with "# Result: #<N> —" AND have ## Status section
# Also validate file is fresh (written AFTER agent started, not stale from previous attempt)
_VALID_RESULT=false
if [[ -f "$RESULT_FILE" ]]; then
  _FIRST_LINE=$(head -1 "$RESULT_FILE")
  _HAS_STATUS=$(grep -q '^## Status:' "$RESULT_FILE" && echo "true" || echo "false")
  _FILE_SIZE=$(wc -c < "$RESULT_FILE" 2>/dev/null || echo "0")

  # Check file freshness: must be written AFTER agent started (prevent stale files from previous runs)
  _FILE_MTIME=$(stat -f%m "$RESULT_FILE" 2>/dev/null || stat -c%Y "$RESULT_FILE" 2>/dev/null || echo "0")
  _AGENT_STARTED=$(jq -r --arg id "$ISSUE_NUM" '.issues[$id].agent_started_at // "0"' "$STATE_FILE" 2>/dev/null || echo "0")
  _IS_FRESH="true"
  if [[ "$_AGENT_STARTED" != "0" ]] && (( _FILE_MTIME < ${_AGENT_STARTED%.*} )); then
    _IS_FRESH="false"
  fi

  if echo "$_FIRST_LINE" | grep -qE '^# Result: #[0-9]+ —' && [[ "$_HAS_STATUS" == "true" ]] && (( _FILE_SIZE > 100 )) && [[ "$_IS_FRESH" == "true" ]]; then
    _VALID_RESULT=true
  else
    # ESCALATE: Mark issue as BLOCKED and queue urgent verify event
    echo "CRITICAL: $RESULT_FILE validation FAILED for $ISSUE_KEY" >> .autoship/poll.log
    echo "  Expected: First line matching '^# Result: #[0-9]+ —', contains '## Status:', >100 bytes, fresh (>= agent_started_at)" >> .autoship/poll.log
    echo "  Actual: First line: '$_FIRST_LINE'" >> .autoship/poll.log
    echo "  Has Status section: $_HAS_STATUS, File size: $_FILE_SIZE bytes, Fresh: $_IS_FRESH" >> .autoship/poll.log
    if [[ "$_IS_FRESH" == "false" ]]; then
      echo "  WARNING: File is STALE (mtime=$_FILE_MTIME < agent_started_at=${_AGENT_STARTED%.*})" >> .autoship/poll.log
      echo "  This suggests a result file from a PREVIOUS attempt, not current run." >> .autoship/poll.log
    fi
    echo "  Full file (first 10 lines):" >> .autoship/poll.log
    head -10 "$RESULT_FILE" | sed 's/^/    /' >> .autoship/poll.log

    # Mark as BLOCKED for escalation to Opus
    bash "$(cat .autoship/hooks_dir)/update-state.sh" set-blocked "$ISSUE_NUM" escalation_reason="invalid_result_format" 2>/dev/null || true
    echo "ERROR: Issue $ISSUE_KEY escalated to BLOCKED — result file validation failed. Manual review required." >&2
  fi
fi
if [[ "$_VALID_RESULT" == "true" ]]; then
  mkdir -p .autoship/results
  ISSUE_TITLE=""
  if command -v gh >/dev/null 2>&1 && [[ -n "$REPO_SLUG" ]]; then
    ISSUE_TITLE=$(gh issue view "$ISSUE_NUM" --repo "$REPO_SLUG" --json title --jq '.title' 2>/dev/null) || ISSUE_TITLE=""
  fi
  SLUG=$(echo "$ISSUE_TITLE" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '-' | sed 's/-*$//' | cut -c1-40)
  ARCHIVE_NAME="${ISSUE_NUM}${SLUG:+-${SLUG}}.md"
  cp "$RESULT_FILE" ".autoship/results/${ARCHIVE_NAME}"
  echo "Archived AUTOSHIP_RESULT.md to .autoship/results/${ARCHIVE_NAME}"
fi

# SAFETY: Verify PR merge in git history before cleanup (prevent race condition)
# If this is a merged PR cleanup, ensure the merge commit exists in origin before nuking worktree
if [[ "$ISSUE_STATE" == "merged" ]]; then
  git fetch origin main 2>/dev/null || true
  # Try to find the branch in git log (crude check, but prevents accidental orphan commits)
  if ! git log --oneline origin/main 2>/dev/null | grep -qi "$ISSUE_KEY\|#$ISSUE_NUM"; then
    echo "WARNING: Issue $ISSUE_KEY marked merged but commit not found in origin/main — delaying cleanup" >> .autoship/poll.log
    echo "WARN: PR merge not yet reflected in git history — skipping cleanup for $ISSUE_KEY" >&2
    exit 0  # Re-queue cleanup for next poll cycle
  fi
fi

# Check if worktree exists
if [[ ! -d "$WORKTREE_PATH" ]]; then
  echo "Warning: worktree at $WORKTREE_PATH does not exist, skipping removal"
else
  echo "Removing worktree at $WORKTREE_PATH"
  git worktree remove --force "$WORKTREE_PATH" 2>/dev/null || {
    # If worktree remove fails, try manual cleanup
    rm -rf "$WORKTREE_PATH" || true
  }
fi

# Check if autoship branch exists locally
if git rev-parse --verify "$AUTOSHIP_BRANCH" >/dev/null 2>&1; then
  echo "Deleting local branch $AUTOSHIP_BRANCH"
  git branch -D "$AUTOSHIP_BRANCH" 2>/dev/null || true
fi

# Update state file
echo "Updating state for $ISSUE_KEY to merged"
bash "$SCRIPT_DIR/update-state.sh" "set-merged" "$ISSUE_KEY" || true

# Remove autoship labels from the GitHub issue
if [[ -n "$REPO_SLUG" ]] && command -v gh >/dev/null 2>&1; then
  echo "Removing transitional autoship labels from issue $ISSUE_NUM"

  # Remove only transitional labels — autoship:done is kept as permanent audit trail
  for label in "autoship:in-progress" "autoship:blocked" "autoship:paused"; do
    gh issue edit "$ISSUE_NUM" --remove-label "$label" --repo "$REPO_SLUG" 2>/dev/null || true
  done
  
  # Check if issue is still open and close it if needed
  ISSUE_STATE=$(gh issue view "$ISSUE_NUM" --repo "$REPO_SLUG" --json state --jq '.state' 2>/dev/null) || ISSUE_STATE=""
  
  if [[ "$ISSUE_STATE" == "OPEN" ]]; then
    echo "Closing issue $ISSUE_NUM on GitHub"
    gh issue close "$ISSUE_NUM" --repo "$REPO_SLUG" --comment "Closed by AutoShip worktree cleanup after merge." 2>/dev/null || {
      echo "Warning: failed to close issue $ISSUE_NUM"
    }
  fi
else
  echo "Warning: could not determine repo slug or gh CLI not available; skipping GitHub cleanup"
fi

echo "Cleanup complete for $ISSUE_KEY"
