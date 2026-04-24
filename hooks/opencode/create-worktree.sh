#!/usr/bin/env bash
set -euo pipefail

ISSUE_KEY="${1:?Issue key required}"
TARGET_BRANCH="${2:-autoship/${ISSUE_KEY}}"
AUTOSHIP_DIR=".autoship"
WORKSPACE="${AUTOSHIP_DIR}/workspaces/${ISSUE_KEY}"

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

mkdir -p "$(dirname "$WORKSPACE")"

BASE_REF="origin/master"
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="origin/main"
fi
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="HEAD"
fi

git fetch origin master --quiet 2>/dev/null || git fetch origin main --quiet 2>/dev/null || true

if ! git worktree add -B "$TARGET_BRANCH" "$WORKSPACE" "$BASE_REF" 2>/dev/null; then
  git worktree remove --force "$WORKSPACE" 2>/dev/null || true
  git worktree add -B "$TARGET_BRANCH" "$WORKSPACE" "$BASE_REF"
fi

printf '%s\n' "$REPO_ROOT/$WORKSPACE"
