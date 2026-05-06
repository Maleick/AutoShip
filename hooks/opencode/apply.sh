#!/usr/bin/env bash
set -euo pipefail

ISSUE_KEY="issue-${1#issue-}"
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
WORKTREE="$REPO_ROOT/.autoship/workspaces/$ISSUE_KEY"
[[ -d "$WORKTREE" ]] || {
  echo "Missing worktree: $WORKTREE" >&2
  exit 1
}
AUTOSHIP_PR_MODE=live AUTOSHIP_ENABLE_PR_CREATE=true bash "$REPO_ROOT/hooks/opencode/create-pr.sh" "$ISSUE_KEY" "$WORKTREE"
