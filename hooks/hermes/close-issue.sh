#!/usr/bin/env bash
# Auto-close issue after successful completion
set -euo pipefail

ISSUE_NUM="${1:?Issue number required}"
# Auto-detect target repo: default to current repo unless explicitly overridden
if [[ -n "${HERMES_TARGET_REPO:-}" ]]; then
  REPO="$HERMES_TARGET_REPO"
else
  CURRENT_REMOTE="$(git remote get-url origin 2>/dev/null || true)"
  if [[ "$CURRENT_REMOTE" =~ github\.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
    REPO_NAME="${BASH_REMATCH[2]}"
    REPO_NAME="${REPO_NAME%.git}"
    REPO="${BASH_REMATCH[1]}/${REPO_NAME}"
  else
    REPO="Maleick/TextQuest"
  fi
fi

# Close with comment
gh issue close "$ISSUE_NUM" --repo "$REPO" --reason completed \
  --comment "✅ COMPLETED via AutoShip burn-down.

- Implementation finished and committed
- Branch pushed: autoship/issue-${ISSUE_NUM}
- PR opened with closing reference

Evidence in HERMES_RESULT.md in worktree." 2>/dev/null || {
  echo "Warning: Could not close issue #$ISSUE_NUM automatically"
  exit 0
}

echo "Closed issue #$ISSUE_NUM"
