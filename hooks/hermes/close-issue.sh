#!/usr/bin/env bash
# Auto-close issue after successful completion
set -euo pipefail

ISSUE_NUM="${1:?Issue number required}"
# Resolve target repo: config.json → env → auto-detect → error
AUTOSHIP_DIR="${AUTOSHIP_DIR:-.autoship}"
REPO=""
if [[ -f "$AUTOSHIP_DIR/config.json" ]]; then
  REPO="$(jq -r '.repo // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)"
fi
if [[ -z "$REPO" && -n "${HERMES_TARGET_REPO:-}" ]]; then
  REPO="$HERMES_TARGET_REPO"
fi
if [[ -z "$REPO" ]]; then
  CURRENT_REMOTE="$(git remote get-url origin 2>/dev/null || true)"
  if [[ "$CURRENT_REMOTE" =~ github\.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
    REPO_NAME="${BASH_REMATCH[2]}"
    REPO_NAME="${REPO_NAME%.git}"
    REPO="${BASH_REMATCH[1]}/${REPO_NAME}"
  fi
fi
if [[ -z "$REPO" ]]; then
  echo "Error: HERMES_TARGET_REPO not set and could not derive repo from origin remote or config" >&2
  exit 1
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
