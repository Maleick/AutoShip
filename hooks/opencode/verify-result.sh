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

run_test_command() {
  local command_string="$1"
  local -a command_parts=()
  local -a env_parts=()
  read -r -a command_parts <<<"$command_string"
  if [[ ${#command_parts[@]} -eq 0 ]]; then
    fail "test command is empty"
  fi
  local part
  for part in "${command_parts[@]}"; do
    if [[ ! "$part" =~ ^[A-Za-z0-9_./:=@%+,-]+$ ]]; then
      fail "test command contains unsupported characters"
    fi
  done
  while [[ ${#command_parts[@]} -gt 0 && "${command_parts[0]}" =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; do
    env_parts+=("${command_parts[0]}")
    command_parts=("${command_parts[@]:1}")
  done
  if [[ ${#command_parts[@]} -eq 0 ]]; then
    fail "test command is missing executable"
  fi
  if [[ ${#env_parts[@]} -gt 0 ]]; then
    if ! (cd "$GIT_WORKTREE" && env "${env_parts[@]}" "${command_parts[@]}") >>"$LOG_PATH" 2>&1; then
      fail "test command failed"
    fi
  elif ! (cd "$GIT_WORKTREE" && "${command_parts[@]}") >>"$LOG_PATH" 2>&1; then
    fail "test command failed"
  fi
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
  run_test_command "$TEST_COMMAND"
fi

if [[ -f "$SCRIPT_DIR/policy-verify.sh" ]]; then
  if ! bash "$SCRIPT_DIR/policy-verify.sh" "$GIT_WORKTREE" >>"$LOG_PATH" 2>&1; then
    fail "policy verification failed"
  fi
else
  fail "policy verification script missing"
fi

reviewer_output=$(mktemp)
if ! bash "$SCRIPT_DIR/reviewer.sh" "$ISSUE_KEY" "$WORKTREE_PATH" "$RESULT_PATH" "$TEST_COMMAND" >"$reviewer_output" 2>&1; then
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
