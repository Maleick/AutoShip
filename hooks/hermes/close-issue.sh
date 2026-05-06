#!/usr/bin/env bash
# Auto-close issue after successful completion
set -euo pipefail

ISSUE_NUM="${1:?Issue number required}"
REPO="${HERMES_TARGET_REPO:-Maleick/TextQuest}"

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
