#!/usr/bin/env bash
# Clean up an AutoShip worktree after merge.

set -euo pipefail

ISSUE_KEY="${1:-}"
[[ -z "$ISSUE_KEY" ]] && echo "Usage: $0 <issue-key>" >&2 && exit 1

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

if [[ ! "$ISSUE_KEY" =~ ^issue-[0-9]+$ ]]; then
  echo "Error: Invalid issue key format: $ISSUE_KEY"
  exit 1
fi

AUTOSHIP_DIR=".autoship"
AUTOSHIP_ROOT="$REPO_ROOT/$AUTOSHIP_DIR"
WORKSPACES_ROOT="$AUTOSHIP_ROOT/workspaces"
WORKSPACE_DIR="$WORKSPACES_ROOT/$ISSUE_KEY"
STATE_FILE="$AUTOSHIP_ROOT/state.json"

ISSUE_NUM="${ISSUE_KEY#issue-}"

if [[ -d "$WORKSPACE_DIR" ]]; then
  WORKSPACE_REAL=$(cd "$WORKSPACE_DIR" && pwd -P)
  case "$WORKSPACE_REAL" in
    "$WORKSPACES_ROOT"/*) ;;
    *)
      echo "Error: Worktree path escapes workspaces root: $WORKSPACE_DIR" >&2
      exit 1
      ;;
  esac

  RESULT_FILE="$WORKSPACE_REAL/AUTOSHIP_RESULT.md"
  if [[ -f "$RESULT_FILE" ]]; then
    if [[ -L "$RESULT_FILE" ]]; then
      echo "Error: Refusing to archive symlinked result file: $RESULT_FILE" >&2
      exit 1
    fi

    mkdir -p "$AUTOSHIP_ROOT/results"
    cp "$RESULT_FILE" "$AUTOSHIP_ROOT/results/${ISSUE_KEY}.md"
    echo "Archived result to $AUTOSHIP_ROOT/results/${ISSUE_KEY}.md"
  fi

  git worktree prune >/dev/null 2>&1 || true
  git worktree remove "$WORKSPACE_REAL" --force 2>/dev/null || true
  echo "Removed worktree: $WORKSPACE_REAL"
fi

BRANCH="autoship/$ISSUE_KEY"
if [[ -n "$(git branch --list "$BRANCH")" ]]; then
  git branch -D "$BRANCH"
  echo "Deleted branch: $BRANCH"
fi

if [[ -f "$STATE_FILE" ]]; then
  jq --arg key "$ISSUE_KEY" \
    '.issues[$key].state = "merged" |
     .issues[$key].completed_at = (now | todate)' \
    "$STATE_FILE" >"$STATE_FILE.tmp" && mv "$STATE_FILE.tmp" "$STATE_FILE"
fi

gh issue edit "$ISSUE_NUM" --remove-label "autoship:in-progress" 2>/dev/null || true
gh issue edit "$ISSUE_NUM" --add-label "autoship:done" 2>/dev/null || true

gh issue close "$ISSUE_NUM" 2>/dev/null || true

echo "Cleanup complete for $ISSUE_KEY"
