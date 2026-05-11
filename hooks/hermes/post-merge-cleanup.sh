#!/usr/bin/env bash
# Post-merge cleanup: remove worktrees after PR is merged
set -euo pipefail

ISSUE_NUM="${1:?Issue number required}"
# Resolve target repo: env → config.json → auto-detect → error
# Env var takes precedence for one-shot overrides; config is the persistent default.
AUTOSHIP_DIR="${AUTOSHIP_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || echo "$HOME/Projects/AutoShip")/.autoship}"
REPO=""
if [[ -n "${HERMES_TARGET_REPO:-}" ]]; then
  REPO="$HERMES_TARGET_REPO"
fi
if [[ -z "$REPO" && -f "$AUTOSHIP_DIR/config.json" ]]; then
  REPO="$(jq -r '.repo // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)"
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

# Derive target repo path from repo name
if [[ "$REPO" == */* ]]; then
  REPO_NAME="${REPO#*/}"
  TARGET_REPO="${HERMES_TARGET_REPO_PATH:-$HOME/Projects/$REPO_NAME}"
else
  TARGET_REPO="${HERMES_TARGET_REPO_PATH:-}"
fi
if [[ -z "$TARGET_REPO" ]]; then
  # Derive from current repo root (the repo where this hook is running)
  TARGET_REPO=$(git rev-parse --show-toplevel 2>/dev/null || echo "")
fi
if [[ -z "$TARGET_REPO" ]]; then
  echo "Error: HERMES_TARGET_REPO_PATH not set and could not derive target repo path" >&2
  exit 1
fi

echo "=== Post-merge cleanup for issue #$ISSUE_NUM ==="

# 1. Remove local worktree
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
