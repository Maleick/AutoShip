#!/usr/bin/env bash
set -euo pipefail

ISSUE_KEY="${1:?Issue key required}"
TARGET_BRANCH="${2:-autoship/${ISSUE_KEY}}"
AUTOSHIP_DIR=".autoship"
WORKSPACE="${AUTOSHIP_DIR}/workspaces/${ISSUE_KEY}"

if [[ ! "$ISSUE_KEY" =~ ^issue-[0-9]+$ ]]; then
  echo "Error: issue key must match issue-<number>" >&2
  exit 1
fi

if [[ ! "$TARGET_BRANCH" =~ ^autoship/issue-[0-9]+$ ]]; then
  echo "Error: target branch must match autoship/issue-<number>" >&2
  exit 1
fi

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

if [[ -L "$AUTOSHIP_DIR" || -L "$AUTOSHIP_DIR/workspaces" ]]; then
  echo "Error: refusing symlinked AutoShip workspace parent" >&2
  exit 1
fi
mkdir -p "$(dirname "$WORKSPACE")"
REAL_AUTOSHIP_DIR="$(cd "$AUTOSHIP_DIR" && pwd -P)"
REAL_WORKSPACES_DIR="$(cd "$AUTOSHIP_DIR/workspaces" && pwd -P)"
case "$REAL_WORKSPACES_DIR" in
  "$REAL_AUTOSHIP_DIR"/*) ;;
  *)
    echo "Error: workspaces directory must remain under .autoship" >&2
    exit 1
    ;;
esac

git fetch origin master --quiet 2>/dev/null || git fetch origin main --quiet 2>/dev/null || true

BASE_REF="origin/master"
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="origin/main"
fi
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="HEAD"
fi

if ! git worktree add -B "$TARGET_BRANCH" "$WORKSPACE" "$BASE_REF" >/dev/null 2>&1; then
  if [[ -L "$WORKSPACE" ]]; then
    echo "Error: refusing to remove symlinked workspace: $WORKSPACE" >&2
    exit 1
  fi
  if [[ -L "$AUTOSHIP_DIR" || -L "$AUTOSHIP_DIR/workspaces" ]]; then
    echo "Error: refusing symlinked AutoShip workspace parent" >&2
    exit 1
  fi
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
mkdir -p "$WORKSPACE/config"
for runtime_file in model-routing.json config.json state.json routing.json; do
  if [[ -f "$AUTOSHIP_DIR/$runtime_file" ]]; then
    cp "$AUTOSHIP_DIR/$runtime_file" "$WORKSPACE/.autoship/$runtime_file"
  fi
  if [[ -f "config/$runtime_file" ]]; then
    cp "config/$runtime_file" "$WORKSPACE/config/$runtime_file"
  fi
done

printf '%s\n' "$REPO_ROOT/$WORKSPACE"
