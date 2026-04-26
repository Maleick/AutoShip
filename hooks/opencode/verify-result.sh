#!/usr/bin/env bash
set -euo pipefail

ISSUE_KEY="${1:?Issue key required}"
WORKTREE_PATH="${2:?Worktree path required}"
TEST_COMMAND="${3:-none}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "FAIL: not inside a git repository" >&2
  exit 1
}

RESULT_PATH="$WORKTREE_PATH/AUTOSHIP_RESULT.md"
STARTED_PATH="$WORKTREE_PATH/started_at"
LOG_PATH="$WORKTREE_PATH/AUTOSHIP_VERIFICATION.log"

fail() {
  mkdir -p "$WORKTREE_PATH" 2>/dev/null || true
  printf 'FAIL: %s\n' "$1" | tee -a "$LOG_PATH" >&2
  exit 1
}

[[ -d "$WORKTREE_PATH" ]] || fail "worktree missing"
[[ -s "$RESULT_PATH" ]] || fail "AUTOSHIP_RESULT.md missing or empty"

real_worktree=$(cd "$WORKTREE_PATH" && pwd -P) || fail "cannot resolve worktree"
real_result=$(cd "$(dirname "$RESULT_PATH")" && printf '%s/%s\n' "$(pwd -P)" "$(basename "$RESULT_PATH")") || fail "cannot resolve result path"
case "$real_result" in
  "$real_worktree"/*) ;;
  *) fail "AUTOSHIP_RESULT.md is outside worktree" ;;
esac

if [[ -f "$STARTED_PATH" && ! "$RESULT_PATH" -nt "$STARTED_PATH" ]]; then
  fail "AUTOSHIP_RESULT.md is stale"
fi

GIT_WORKTREE="$WORKTREE_PATH"
if ! git -C "$GIT_WORKTREE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  GIT_WORKTREE="$REPO_ROOT"
fi

git -C "$GIT_WORKTREE" rev-parse --verify HEAD >/dev/null 2>&1 || fail "no git commit to verify"

if [[ -x "$SCRIPT_DIR/diff-size-guard.sh" ]]; then
  bash "$SCRIPT_DIR/diff-size-guard.sh" check "$GIT_WORKTREE" >> "$LOG_PATH" 2>&1 || fail "diff size guard failed"
fi

if [[ -f "$WORKTREE_PATH/shasum.before" && -x "$SCRIPT_DIR/worktree-checksum.sh" ]]; then
  bash "$SCRIPT_DIR/worktree-checksum.sh" checksum "$GIT_WORKTREE" > "$WORKTREE_PATH/shasum.after" 2>/dev/null || true
  if ! cmp -s "$WORKTREE_PATH/shasum.before" "$WORKTREE_PATH/shasum.after"; then
    printf 'scope-report: in-scope-changes\n' >> "$LOG_PATH"
  else
    fail "worker reported result without checksum changes"
  fi
fi

has_diff=false
if [[ -n "$(git -C "$GIT_WORKTREE" status --porcelain 2>/dev/null)" ]]; then
  has_diff=true
fi
if ! git -C "$GIT_WORKTREE" diff --quiet -- . 2>/dev/null; then
  has_diff=true
fi
if git -C "$GIT_WORKTREE" rev-parse --verify HEAD~1 >/dev/null 2>&1 && ! git -C "$GIT_WORKTREE" diff --quiet HEAD~1...HEAD -- . 2>/dev/null; then
  has_diff=true
fi
[[ "$has_diff" == true ]] || fail "git diff is empty"

if [[ -n "$TEST_COMMAND" && "$TEST_COMMAND" != "none" ]]; then
  if [[ -x "$SCRIPT_DIR/anti-flake.sh" ]]; then
    if ! (cd "$GIT_WORKTREE" && bash "$SCRIPT_DIR/anti-flake.sh" run "$TEST_COMMAND") >> "$LOG_PATH" 2>&1; then
      fail "test command failed"
    fi
  elif ! (cd "$GIT_WORKTREE" && eval "$TEST_COMMAND") >> "$LOG_PATH" 2>&1; then
    fail "test command failed"
  fi
fi

reviewer_output=$(mktemp)
if ! bash "$SCRIPT_DIR/reviewer.sh" "$ISSUE_KEY" "$WORKTREE_PATH" "$RESULT_PATH" "$TEST_COMMAND" > "$reviewer_output" 2>&1; then
  cp "$reviewer_output" "$LOG_PATH" 2>/dev/null || true
  rm -f "$reviewer_output"
  fail "reviewer failed"
fi

if ! grep -F 'VERDICT: PASS' "$reviewer_output" >/dev/null 2>&1; then
  cp "$reviewer_output" "$LOG_PATH" 2>/dev/null || true
  rm -f "$reviewer_output"
  fail "reviewer did not pass"
fi

cp "$reviewer_output" "$LOG_PATH" 2>/dev/null || true
rm -f "$reviewer_output"
printf 'PASS\n'
