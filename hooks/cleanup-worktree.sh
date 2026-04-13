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

# Locate repo root
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

# Resolve sibling scripts
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Paths
WORKTREE_PATH=".beacon/workspaces/$ISSUE_KEY"
BEACON_BRANCH="beacon/$ISSUE_KEY"

# Extract issue number early (needed for archive)
ISSUE_NUM_EARLY="${ISSUE_KEY#issue-}"

# Archive BEACON_RESULT.md before removing worktree
RESULT_FILE="$WORKTREE_PATH/BEACON_RESULT.md"
# Fallback: if not in worktree, check parent repo root (Phase-1 / no-worktree agents write here)
if [[ ! -f "$RESULT_FILE" ]] && [[ -f "BEACON_RESULT.md" ]]; then
  echo "Warning: BEACON_RESULT.md not found in worktree, falling back to repo root" >> .beacon/poll.log
  RESULT_FILE="BEACON_RESULT.md"
fi
# Content validation — must start with "# Result: #<N>" to be a valid BEACON_RESULT
_VALID_RESULT=false
if [[ -f "$RESULT_FILE" ]]; then
  if head -1 "$RESULT_FILE" | grep -qE '^# Result: #[0-9]+'; then
    _VALID_RESULT=true
  else
    echo "Warning: $RESULT_FILE failed content validation (first line: $(head -1 "$RESULT_FILE"))" >> .beacon/poll.log
    echo "Warning: skipping archival for $ISSUE_KEY — BEACON_RESULT.md content invalid" >&2
  fi
fi
if [[ "$_VALID_RESULT" == "true" ]]; then
  mkdir -p .beacon/results
  # Fetch issue title for the slug (best-effort)
  ISSUE_TITLE=""
  if command -v gh >/dev/null 2>&1; then
    # Try to get repo slug from state or remote for the title fetch
    _STATE=".beacon/state.json"
    _REPO=""
    if [[ -f "$_STATE" ]]; then
      _REPO=$(jq -r '.repo // empty' "$_STATE" 2>/dev/null) || true
    fi
    if [[ -z "$_REPO" ]]; then
      _REMOTE=$(git remote get-url origin 2>/dev/null) || true
      if [[ -n "$_REMOTE" ]]; then
        _REPO=$(echo "$_REMOTE" | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
      fi
    fi
    if [[ -n "$_REPO" ]]; then
      ISSUE_TITLE=$(gh issue view "$ISSUE_NUM_EARLY" --repo "$_REPO" --json title --jq '.title' 2>/dev/null) || ISSUE_TITLE=""
    fi
  fi
  SLUG=$(echo "$ISSUE_TITLE" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '-' | sed 's/-*$//' | cut -c1-40)
  ARCHIVE_NAME="${ISSUE_NUM_EARLY}${SLUG:+-${SLUG}}.md"
  cp "$RESULT_FILE" ".beacon/results/${ARCHIVE_NAME}"
  echo "Archived BEACON_RESULT.md to .beacon/results/${ARCHIVE_NAME}"
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

# Check if beacon branch exists locally
if git rev-parse --verify "$BEACON_BRANCH" >/dev/null 2>&1; then
  echo "Deleting local branch $BEACON_BRANCH"
  git branch -D "$BEACON_BRANCH" 2>/dev/null || true
fi

# Update state file
echo "Updating state for $ISSUE_KEY to merged"
bash "$SCRIPT_DIR/update-state.sh" "set-merged" "$ISSUE_KEY" || true

# Extract issue number from issue-key (e.g., "issue-16" → "16")
ISSUE_NUM="${ISSUE_KEY#issue-}"

# Get repo slug from state.json or git remote
STATE_FILE=".beacon/state.json"
REPO_SLUG=""
if [[ -f "$STATE_FILE" ]]; then
  REPO_SLUG=$(jq -r '.repo // empty' "$STATE_FILE" 2>/dev/null) || true
fi

# Fallback: derive from git remote
if [[ -z "$REPO_SLUG" ]]; then
  REMOTE_URL=$(git remote get-url origin 2>/dev/null) || true
  if [[ -n "$REMOTE_URL" ]]; then
    REPO_SLUG=$(echo "$REMOTE_URL" | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
  fi
fi

# Remove beacon labels from the GitHub issue
if [[ -n "$REPO_SLUG" ]] && command -v gh >/dev/null 2>&1; then
  echo "Removing transitional beacon labels from issue $ISSUE_NUM"

  # Remove only transitional labels — beacon:done is kept as permanent audit trail
  for label in "beacon:in-progress" "beacon:blocked" "beacon:paused"; do
    gh issue edit "$ISSUE_NUM" --remove-label "$label" --repo "$REPO_SLUG" 2>/dev/null || true
  done
  
  # Check if issue is still open and close it if needed
  ISSUE_STATE=$(gh issue view "$ISSUE_NUM" --repo "$REPO_SLUG" --json state --jq '.state' 2>/dev/null) || ISSUE_STATE=""
  
  if [[ "$ISSUE_STATE" == "OPEN" ]]; then
    echo "Closing issue $ISSUE_NUM on GitHub"
    gh issue close "$ISSUE_NUM" --repo "$REPO_SLUG" --comment "Closed by Beacon worktree cleanup after merge." 2>/dev/null || {
      echo "Warning: failed to close issue $ISSUE_NUM"
    }
  fi
else
  echo "Warning: could not determine repo slug or gh CLI not available; skipping GitHub cleanup"
fi

echo "Cleanup complete for $ISSUE_KEY"
