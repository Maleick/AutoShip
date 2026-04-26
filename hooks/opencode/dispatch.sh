#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DRY_RUN=false
POSITIONAL=()

for arg in "$@"; do
  if [[ "$arg" == "--dry-run" ]]; then
    DRY_RUN=true
  else
    POSITIONAL+=("$arg")
  fi
done

ISSUE_NUM="${POSITIONAL[0]:?Issue number required}"
TASK_TYPE="${POSITIONAL[1]:-medium_code}"
MODEL_OVERRIDE="${POSITIONAL[2]:-}"

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
ROUTING_FILE="$AUTOSHIP_DIR/model-routing.json"
ISSUE_KEY="issue-${ISSUE_NUM}"
WORKSPACE_PATH="$AUTOSHIP_DIR/workspaces/$ISSUE_KEY"
ITEM_RECORD="$SCRIPT_DIR/item-record.sh"

if [[ -f "$STATE_FILE" ]] && jq -e --arg key "$ISSUE_KEY" '(.issues[$key].terminal_failure // false) == true or (.issues[$key].retry_eligible // true) == false' "$STATE_FILE" >/dev/null 2>&1; then
  mkdir -p "$WORKSPACE_PATH"
  printf 'BLOCKED\n' > "$WORKSPACE_PATH/status"
  reason=$(jq -r --arg key "$ISSUE_KEY" '.issues[$key].escalation_reason // "retry limit reached"' "$STATE_FILE" 2>/dev/null || echo "retry limit reached")
  printf '%s\n' "$reason" > "$WORKSPACE_PATH/BLOCKED_REASON.txt"
  echo "BLOCKED $ISSUE_KEY: $reason"
  exit 0
fi

max_agents=$(jq -r '.config.maxConcurrentAgents // .max_concurrent_agents // empty' "$STATE_FILE" 2>/dev/null || true)
if [[ -z "$max_agents" && -f "$AUTOSHIP_DIR/config.json" ]]; then
  max_agents=$(jq -r '.maxConcurrentAgents // .max_agents // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)
fi
max_agents="${max_agents:-15}"
running=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "running")] | length' "$STATE_FILE" 2>/dev/null || echo 0)
cap_note=""
if (( running >= max_agents )); then
  cap_note="CAP_REACHED: $running active / $max_agents max; workspace will remain queued"
fi

TITLE=$(gh issue view "$ISSUE_NUM" --json title --jq '.title' 2>/dev/null || echo "Issue $ISSUE_NUM")
BODY=$(gh issue view "$ISSUE_NUM" --json body --jq '.body' 2>/dev/null || echo "")
LABELS=$(gh issue view "$ISSUE_NUM" --json labels --jq '[.labels[].name] | join(",")' 2>/dev/null || echo "")
SANITIZED_BODY="$BODY"
if [[ -x "$SCRIPT_DIR/sanitize-issue.sh" ]]; then
  SANITIZED_BODY=$(bash "$SCRIPT_DIR/sanitize-issue.sh" sanitize "$ISSUE_NUM" "$BODY" 2>/dev/null || printf '%s' "$BODY")
fi
CRITERIA_JSON='{}'
if [[ -x "$SCRIPT_DIR/extract-criteria.sh" ]]; then
  CRITERIA_JSON=$(bash "$SCRIPT_DIR/extract-criteria.sh" extract "$BODY" 2>/dev/null || echo '{}')
fi
FAILURE_CONTEXT=""
latest_failure=$(ls -t "$AUTOSHIP_DIR/failures"/*-"$ISSUE_KEY".json 2>/dev/null | head -1 || true)
if [[ -n "$latest_failure" ]]; then
  FAILURE_CONTEXT=$(jq -r '"Previous failure: " + (.failure_category // "unknown") + " - " + (.error_summary // "")' "$latest_failure" 2>/dev/null || true)
fi

resolve_model() {
  local task_type="$1"
  local issue_num="$2"
  local override="$3"
  if [[ -n "$override" ]]; then
    printf '%s\n' "$override"
    return 0
  fi
  if [[ -f "$ROUTING_FILE" ]]; then
    ROUTING_LOG=$(bash "$SCRIPT_DIR/select-model.sh" --log "$task_type" "$issue_num" 2>/dev/null || echo "")
    local selected_model
    selected_model=$(bash "$SCRIPT_DIR/select-model.sh" "$task_type" "$issue_num" 2>/dev/null || echo "")
    printf '%s\n' "$selected_model"
    return 0
  fi
  printf '%s\n' ""
}

resolve_role() {
  case "$1" in
    docs|documentation) printf '%s\n' docs ;;
    review|code_review) printf '%s\n' reviewer ;;
    test|tests|ci_fix) printf '%s\n' tester ;;
    release) printf '%s\n' release ;;
    simplify|refactor) printf '%s\n' simplifier ;;
    plan|planning) printf '%s\n' planner ;;
    lead|orchestration|coordination) printf '%s\n' lead ;;
    *) printf '%s\n' implementer ;;
  esac
}

MODEL=$(resolve_model "$TASK_TYPE" "$ISSUE_NUM" "$MODEL_OVERRIDE")
ROLE=$(resolve_role "$TASK_TYPE")
if [[ -z "$MODEL" ]]; then
  mkdir -p "$WORKSPACE_PATH"
  printf 'BLOCKED\n' > "$WORKSPACE_PATH/status"
  printf 'No configured OpenCode model is available for task type %s. Run hooks/opencode/setup.sh to choose models.\n' "$TASK_TYPE" > "$WORKSPACE_PATH/BLOCKED_REASON.txt"
  bash "$REPO_ROOT/hooks/update-state.sh" set-blocked "$ISSUE_KEY" reason="no configured OpenCode model for $TASK_TYPE" 2>/dev/null || true
  echo "BLOCKED $ISSUE_KEY: no configured OpenCode model for $TASK_TYPE"
  exit 0
fi

if [[ "$DRY_RUN" == true ]]; then
  echo "Dry run: would dispatch issue #$ISSUE_NUM to $MODEL ($TASK_TYPE)"
  echo "Prompt path: $WORKSPACE_PATH/AUTOSHIP_PROMPT.md"
  echo "Worktree path: $WORKSPACE_PATH"
  echo "Status path: $WORKSPACE_PATH/status"
  exit 0
fi

FULL_WORKSPACE_PATH=$(bash "$SCRIPT_DIR/create-worktree.sh" "$ISSUE_KEY" "autoship/issue-${ISSUE_NUM}")
mkdir -p "$WORKSPACE_PATH"
if [[ -x "$ITEM_RECORD" ]]; then
  bash "$ITEM_RECORD" init "$ISSUE_NUM" "$TITLE" >/dev/null 2>&1 || true
  bash "$ITEM_RECORD" append "$ISSUE_NUM" queued 1 "$MODEL" "dispatched as $TASK_TYPE" >/dev/null 2>&1 || true
fi
date -u +%Y-%m-%dT%H:%M:%SZ > "$WORKSPACE_PATH/started_at"
printf 'QUEUED\n' > "$WORKSPACE_PATH/status"
printf '%s\n' "$MODEL" > "$WORKSPACE_PATH/model"
printf '%s\n' "$ROLE" > "$WORKSPACE_PATH/role"
printf '%s\n' "$CRITERIA_JSON" > "$WORKSPACE_PATH/acceptance-criteria.json"
bash "$SCRIPT_DIR/worktree-checksum.sh" checksum "$FULL_WORKSPACE_PATH" > "$WORKSPACE_PATH/shasum.before" 2>/dev/null || true

cat > "$WORKSPACE_PATH/AUTOSHIP_PROMPT.md" <<EOF
# AutoShip Agent Prompt

## Issue #$ISSUE_NUM: $TITLE

## Labels
$LABELS

## Task Type
$TASK_TYPE

## Selected Model
$MODEL

## Specialized Role
$ROLE

## Body
$(bash "$SCRIPT_DIR/sanitize-issue.sh" wrap "$SANITIZED_BODY" 2>/dev/null || printf '%s' "$SANITIZED_BODY")

## Normalized Acceptance Criteria
$CRITERIA_JSON

## Previous Failure Context
${FAILURE_CONTEXT:-No previous failure context.}

## Instructions
- Work only in this worktree: $FULL_WORKSPACE_PATH
- Implement the issue per its acceptance criteria.
- Run relevant project checks before finishing.
- Do not repeat the same failing command. If a command fails, read the error, change approach, and try a simpler supported command or inspect the relevant file directly.
- Commit changes on branch autoship/issue-$ISSUE_NUM.
- Write AUTOSHIP_RESULT.md in the worktree.
- Write COMPLETE, BLOCKED, or STUCK to $FULL_WORKSPACE_PATH/status.

## PR Title
Use this conventional PR title when creating a PR:
$(bash "$SCRIPT_DIR/pr-title.sh" --issue "$ISSUE_NUM" --title "$TITLE" --labels "$LABELS")
EOF

bash "$REPO_ROOT/hooks/update-state.sh" set-queued "$ISSUE_KEY" agent="$MODEL" model="$MODEL" role="$ROLE" task_type="$TASK_TYPE" 2>/dev/null || true

echo "Queued issue #$ISSUE_NUM for $MODEL ($TASK_TYPE, role=$ROLE)"
[[ -n "$cap_note" ]] && echo "$cap_note"
echo "Worktree: $FULL_WORKSPACE_PATH"
echo "Run: bash hooks/opencode/runner.sh"
