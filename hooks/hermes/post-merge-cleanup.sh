#!/usr/bin/env bash
# Post-merge cleanup: remove worktrees after PR is merged
set -euo pipefail

ISSUE_NUM="${1:?Issue number required}"
REPO="${HERMES_TARGET_REPO:-Maleick/TextQuest}"

echo "=== Post-merge cleanup for issue #$ISSUE_NUM ==="

# 1. Remove local worktree
TARGET_REPO="${HERMES_TARGET_REPO_PATH:-$HOME/Projects/TextQuest}"
wt_path="${TARGET_REPO}.worktrees/issue-${ISSUE_NUM}"
if [[ -d "$wt_path" ]]; then
  echo "Removing worktree: $wt_path"
  cd "$TARGET_REPO"
  git worktree remove "$wt_path" --force 2>/dev/null || rm -rf "$wt_path" 2>/dev/null || true
  git worktree prune
  echo "✅ Worktree removed"
else
  echo "No worktree found at $wt_path"
fi

# 2. Remove AutoShip workspace
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo "$HOME/Projects/AutoShip")
ws_path="${REPO_ROOT}/.autoship/workspaces/issue-${ISSUE_NUM}"
if [[ -d "$ws_path" ]]; then
  echo "Removing workspace: $ws_path"
  rm -rf "$ws_path"
  echo "✅ Workspace removed"
fi

# 3. Clean up branch
branch="autoship/issue-${ISSUE_NUM}"
cd "$TARGET_REPO"
if git branch | grep -q "$branch"; then
  git branch -D "$branch" 2>/dev/null || true
  echo "✅ Local branch removed"
fi

# 4. Update issue labels
gh issue edit "$ISSUE_NUM" --repo "$REPO" --remove-label autoship:ready --add-label autoship:complete 2>/dev/null || true

echo "=== Cleanup complete ==="
