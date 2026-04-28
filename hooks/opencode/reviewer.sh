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
4. For each acceptance criterion, state whether it was met.
5. Return a JSON object matching schema/reviewer-decision.json and exactly one verdict line: VERDICT: PASS or VERDICT: FAIL.

Be strict: partial implementation is FAIL.
EOF

set +e
reviewer_output=$(
  cd "$WORKTREE_PATH"
  opencode run --model "$MODEL" "$(cat "$PROMPT_FILE")" 2>&1
)
reviewer_status=$?
set -e

printf '%s\n' "$reviewer_output"

verdict_line=$(printf '%s\n' "$reviewer_output" | grep -E '^VERDICT: (PASS|FAIL)([^A-Z]|$)' | head -1 || true)
error_summary=""

json_block=$(printf '%s\n' "$reviewer_output" | sed -n '/^{/,/^}/p' | head -200 || true)
if [[ -n "$json_block" ]]; then
  if ! echo "$json_block" | jq -e '(.verdict == "PASS" or .verdict == "FAIL") and (.summary | type == "string")' >/dev/null 2>&1; then
    error_summary="reviewer JSON did not match required schema"
    verdict_line=""
  fi
fi

if [[ $reviewer_status -ne 0 ]]; then
  error_summary="reviewer model failed with non-zero exit"
elif [[ -z "$verdict_line" ]]; then
  error_summary="missing or malformed reviewer verdict"
elif [[ "$verdict_line" == VERDICT:\ PASS* ]]; then
  exit 0
else
  error_summary="reviewer returned FAIL verdict"
fi

REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
if [[ -x "$REPO_ROOT/hooks/capture-failure.sh" ]]; then
  tmp_log=$(mktemp)
  printf '%s\n' "$reviewer_output" > "$tmp_log"
  AUTOSHIP_FAILURE_LOG="$tmp_log" bash "$REPO_ROOT/hooks/capture-failure.sh" reviewer_rejection "$ISSUE_KEY" "error_summary=$error_summary" 2>/dev/null || true
  rm -f "$tmp_log"
fi

echo "VERDICT: FAIL — $error_summary"
exit 1
