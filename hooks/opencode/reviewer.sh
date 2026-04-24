#!/usr/bin/env bash
set -euo pipefail

ISSUE_KEY="${1:?Issue key required}"
WORKTREE_PATH="${2:?Worktree path required}"
RESULT_PATH="${3:-$WORKTREE_PATH/AUTOSHIP_RESULT.md}"
TEST_COMMAND="${4:-none}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MODEL=$(bash "$SCRIPT_DIR/select-model.sh" --role reviewer)
[[ -n "$MODEL" ]] || MODEL="openai/gpt-5.5"

if [[ ! -e "$RESULT_PATH" ]]; then
  echo "VERDICT: FAIL — AUTOSHIP_RESULT.md missing"
  exit 1
fi

if ! command -v opencode >/dev/null 2>&1; then
  echo "VERDICT: FAIL — opencode CLI not found"
  exit 1
fi

PROMPT_FILE=$(mktemp)
trap 'rm -f "$PROMPT_FILE"' EXIT

cat > "$PROMPT_FILE" <<EOF
You are the AutoShip reviewer. Verify this completed worker output.

Issue key: $ISSUE_KEY
Worktree: $WORKTREE_PATH
Result path: $RESULT_PATH
Test command: $TEST_COMMAND

Required checks:
1. Read AUTOSHIP_RESULT.md.
2. Review git diff against the base branch.
3. Run the test command if it is not "none".
4. Return exactly one verdict line: VERDICT: PASS or VERDICT: FAIL.

Be strict: partial implementation is FAIL.
EOF

(
  cd "$WORKTREE_PATH"
  opencode run --model "$MODEL" "$(cat "$PROMPT_FILE")"
) || {
  echo "VERDICT: FAIL — reviewer model failed"
  if [[ -x "$SCRIPT_DIR/../../../capture-failure.sh" ]]; then
    REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
    bash "$REPO_ROOT/hooks/capture-failure.sh" reviewer_rejection "$ISSUE_KEY" "error_summary=reviewer model failed with non-zero exit" 2>/dev/null || true
  fi
  exit 1
}
