# cleanup-worktree-opencode.sh — Clean up worktree after merge

set -euo pipefail

ISSUE_KEY="${1:-}"
[[ -z "$ISSUE_KEY" ]] && echo "Usage: $0 <issue-key>" >&2 && exit 1

AUTOSHIP_DIR=".autoship"
WORKSPACE_DIR="$AUTOSHIP_DIR/workspaces/$ISSUE_KEY"
STATE_FILE="$AUTOSHIP_DIR/state.json"
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

# Validate issue key format
if [[ ! "$ISSUE_KEY" =~ ^issue-[0-9]+ ]]; then
  echo "Error: Invalid issue key format: $ISSUE_KEY"
  exit 1
fi

# Extract issue number
ISSUE_NUM="${ISSUE_KEY#issue-}"

# Archive result file
if [[ -f "$WORKSPACE_DIR/AUTOSHIP_RESULT.md" ]]; then
  mkdir -p "$AUTOSHIP_DIR/results"
  cp "$WORKSPACE_DIR/AUTOSHIP_RESULT.md" "$AUTOSHIP_DIR/results/${ISSUE_KEY}.md"
  echo "Archived result to $AUTOSHIP_DIR/results/${ISSUE_KEY}.md"
fi

# Remove worktree
if [[ -d "$WORKSPACE_DIR" ]]; then
  git worktree remove "$WORKSPACE_DIR" --force 2>/dev/null || true
  echo "Removed worktree: $WORKSPACE_DIR"
fi

# Delete branch
BRANCH="autoship/$ISSUE_KEY"
if git branch --list "$BRANCH" >/dev/null 2>&1; then
  git branch -D "$BRANCH" 2>/dev/null || true
  echo "Deleted branch: $BRANCH"
fi

# Update state
if [[ -f "$STATE_FILE" ]]; then
  jq --arg key "$ISSUE_KEY" \
    '.issues[$key].state = "merged" |
     .issues[$key].completed_at = (now | todate)' \
    "$STATE_FILE" > "$STATE_FILE.tmp" && mv "$STATE_FILE.tmp" "$STATE_FILE"
fi

# Remove GitHub labels
gh issue edit "$ISSUE_NUM" --remove-label "autoship:in-progress" 2>/dev/null || true
gh issue edit "$ISSUE_NUM" --add-label "autoship:done" 2>/dev/null || true

# Close issue
gh issue close "$ISSUE_NUM" 2>/dev/null || true

echo "Cleanup complete for $ISSUE_KEY"
