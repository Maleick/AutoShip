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
  echo "Removing beacon labels from issue $ISSUE_NUM"
  
  # Remove each beacon label
  for label in "beacon:in-progress" "beacon:blocked" "beacon:paused" "beacon:done"; do
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
