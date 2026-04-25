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

git fetch origin master --quiet 2>/dev/null || git fetch origin main --quiet 2>/dev/null || true

BASE_REF="origin/master"
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="origin/main"
fi
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="HEAD"
fi

if ! git worktree add -B "$TARGET_BRANCH" "$WORKSPACE" "$BASE_REF" >/dev/null 2>&1; then
  git worktree remove --force "$WORKSPACE" >/dev/null 2>&1 || true
  rm -rf "$WORKSPACE"
  git worktree add -B "$TARGET_BRANCH" "$WORKSPACE" "$BASE_REF" >/dev/null
fi

rm -f \
  "$WORKSPACE/AUTOSHIP_PROMPT.md" \
  "$WORKSPACE/AUTOSHIP_RESULT.md" \
  "$WORKSPACE/AUTOSHIP_RUNNER.log" \
  "$WORKSPACE/BLOCKED_REASON.txt" \
  "$WORKSPACE/model" \
  "$WORKSPACE/started_at" \
  "$WORKSPACE/status"

mkdir -p "$WORKSPACE/.autoship"
for runtime_file in model-routing.json config.json state.json routing.json; do
  if [[ -f "$AUTOSHIP_DIR/$runtime_file" ]]; then
    cp "$AUTOSHIP_DIR/$runtime_file" "$WORKSPACE/.autoship/$runtime_file"
  fi
done

printf '%s\n' "$REPO_ROOT/$WORKSPACE"
