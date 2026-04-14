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
# Validate — must start with "# Result: #<N> —"
_VALID_RESULT=false
if [[ -f "$RESULT_FILE" ]]; then
  _FIRST_LINE=$(head -1 "$RESULT_FILE")
  if echo "$_FIRST_LINE" | grep -qE '^# Result: #[0-9]+ —'; then
    _VALID_RESULT=true
  else
    # Log detailed error for debugging
    echo "ERROR: $RESULT_FILE validation failed" >> .autoship/poll.log
    echo "  Expected first line format: # Result: #<issue-number> — <title>" >> .autoship/poll.log
    echo "  Actual first line: $_FIRST_LINE" >> .autoship/poll.log
    echo "  Full file content:" >> .autoship/poll.log
    head -5 "$RESULT_FILE" | sed 's/^/    /' >> .autoship/poll.log
    echo "Warning: skipping archival for $ISSUE_KEY — AUTOSHIP_RESULT.md format invalid. Check format: first line must be '# Result: #<number> —'" >&2
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
