#!/usr/bin/env bash
# Dependency graph: lib/common.sh (optional), select-model.sh, create-worktree.sh, pr-title.sh, update-state.sh
# Leaf callers: update-state.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load shared utilities if available; inline fallback for standalone/test use.
if [[ -f "$SCRIPT_DIR/lib/common.sh" ]]; then
  source "$SCRIPT_DIR/lib/common.sh"
else
  autoship_repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || {
      echo "Error: not inside a git repository" >&2
      return 1
    }
  }
  autoship_state_set() {
    local action="$1" issue_key="$2"
    shift 2
    local repo_root
    repo_root="$(autoship_repo_root)"
    bash "$repo_root/hooks/update-state.sh" "$action" "$issue_key" "$@"
  }
fi

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
# Validate issue number is numeric
if [[ ! "$ISSUE_NUM" =~ ^[0-9]+$ ]]; then
  echo "Error: issue number must be numeric, got: $ISSUE_NUM" >&2
  exit 1
fi
TASK_TYPE="${POSITIONAL[1]:-medium_code}"
MODEL_OVERRIDE="${POSITIONAL[2]:-}"

REPO_ROOT=$(autoship_repo_root) || exit 1
cd "$REPO_ROOT"

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
ROUTING_FILE="$AUTOSHIP_DIR/model-routing.json"
ISSUE_KEY="issue-${ISSUE_NUM}"
WORKSPACE_PATH="$AUTOSHIP_DIR/workspaces/$ISSUE_KEY"

if [[ -f "$STATE_FILE" ]] && jq -e --arg key "$ISSUE_KEY" '(.issues[$key].terminal_failure // false) == true or (.issues[$key].retry_eligible // true) == false' "$STATE_FILE" >/dev/null 2>&1; then
  mkdir -p "$WORKSPACE_PATH"
  printf 'BLOCKED\n' >"$WORKSPACE_PATH/status"
  reason=$(jq -r --arg key "$ISSUE_KEY" '.issues[$key].escalation_reason // "retry limit reached"' "$STATE_FILE" 2>/dev/null || echo "retry limit reached")
  printf '%s\n' "$reason" >"$WORKSPACE_PATH/BLOCKED_REASON.txt"
  echo "BLOCKED $ISSUE_KEY: $reason"
  exit 0
fi

max_agents=$(jq -r '.config.maxConcurrentAgents // .max_concurrent_agents // empty' "$STATE_FILE" 2>/dev/null || true)
if [[ -z "$max_agents" && -f "$AUTOSHIP_DIR/config.json" ]]; then
  max_agents=$(jq -r '.maxConcurrentAgents // .max_agents // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)
fi
max_agents="${max_agents:-20}"
# Validate max_agents is numeric
if [[ ! "$max_agents" =~ ^[0-9]+$ ]]; then
  max_agents=20
fi

# Check system resources and potentially reduce concurrency cap
if [[ -x "$SCRIPT_DIR/resource-monitor.sh" ]]; then
  resource_info=$(bash "$SCRIPT_DIR/resource-monitor.sh" "$max_agents" 2>/dev/null || echo '{"load_status":"ok","recommended_max_concurrent":'$max_agents'}')
  resource_status=$(echo "$resource_info" | jq -r '.load_status // "ok"')
  recommended_max=$(echo "$resource_info" | jq -r '.recommended_max_concurrent // '$max_agents)
  if [[ "$resource_status" != "ok" && "$recommended_max" =~ ^[0-9]+$ && "$recommended_max" -lt "$max_agents" ]]; then
    resource_label=$(printf '%s' "$resource_status" | tr '[:lower:]' '[:upper:]')
    echo "RESOURCE_${resource_label}: CPU/MEM load high, reducing concurrency from $max_agents to $recommended_max" >&2
    max_agents="$recommended_max"
  fi
fi

running=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "running")] | length' "$STATE_FILE" 2>/dev/null || echo 0)
# Validate running is numeric
if [[ ! "$running" =~ ^[0-9]+$ ]]; then
  running=0
fi
cap_note=""
if ((running >= max_agents)); then
  cap_note="CAP_REACHED: $running active / $max_agents max; workspace will remain queued"
fi

TITLE=$(gh issue view "$ISSUE_NUM" --json title --jq '.title' 2>/dev/null || echo "Issue $ISSUE_NUM")
BODY=$(gh issue view "$ISSUE_NUM" --json body --jq '.body' 2>/dev/null || echo "")
LABELS=$(gh issue view "$ISSUE_NUM" --json labels --jq '[.labels[].name] | join(",")' 2>/dev/null || echo "")

resolve_model() {
  local task_type="$1"
  local issue_num="$2"
  local override="$3"
  if [[ -n "$override" ]]; then
    printf '%s\n' "$override"
    return 0
  fi
  if [[ -f "$ROUTING_FILE" ]]; then
    local routing_log
    routing_log=$(bash "$SCRIPT_DIR/select-model.sh" --log "$task_type" "$issue_num" 2>/dev/null || echo "")
    local selected_model
    selected_model=$(bash "$SCRIPT_DIR/select-model.sh" "$task_type" "$issue_num" 2>/dev/null || echo "")
    if [[ -n "$routing_log" ]]; then
      mkdir -p "$WORKSPACE_PATH"
      log_file="$WORKSPACE_PATH/routing-log.txt"
      printf '%s\n' "$routing_log" >"$log_file"
    fi
    printf '%s\n' "$selected_model"
    return 0
  fi
  printf '%s\n' ""
}

resolve_role() {
  case "$1" in
    docs | documentation) printf '%s\n' docs ;;
    review | code_review) printf '%s\n' reviewer ;;
    test | tests | ci_fix) printf '%s\n' tester ;;
    release) printf '%s\n' release ;;
    simplify | refactor) printf '%s\n' simplifier ;;
    plan | planning) printf '%s\n' planner ;;
    lead | orchestration | coordination) printf '%s\n' lead ;;
    *) printf '%s\n' implementer ;;
  esac
}

MODEL=$(resolve_model "$TASK_TYPE" "$ISSUE_NUM" "$MODEL_OVERRIDE")
ROLE=$(resolve_role "$TASK_TYPE")

# A/B testing: deterministically assign issue to group and optionally override model
if [[ -x "$SCRIPT_DIR/ab-test.sh" ]]; then
  ab_group=$(bash "$SCRIPT_DIR/ab-test.sh" assign "$ISSUE_KEY" "$TASK_TYPE" 2>/dev/null || echo "")
  if [[ -n "$ab_group" && -f "$REPO_ROOT/.autoship/ab-test.json" ]]; then
    ab_enabled=$(jq -r '.enabled // false' "$REPO_ROOT/.autoship/ab-test.json" 2>/dev/null || echo "false")
    if [[ "$ab_enabled" == "true" && -z "$MODEL_OVERRIDE" ]]; then
      ab_model=$(bash "$SCRIPT_DIR/ab-test.sh" select-model "$TASK_TYPE" "$ab_group" 2>/dev/null || echo "")
      if [[ -n "$ab_model" ]]; then
        MODEL="$ab_model"
      fi
    fi
  fi
fi

if [[ -z "$MODEL" ]]; then
  mkdir -p "$WORKSPACE_PATH"
  printf 'BLOCKED\n' >"$WORKSPACE_PATH/status"
  printf 'No configured OpenCode model is available for task type %s. Run hooks/opencode/setup.sh to choose models.\n' "$TASK_TYPE" >"$WORKSPACE_PATH/BLOCKED_REASON.txt"
  autoship_state_set set-blocked "$ISSUE_KEY" reason="no configured OpenCode model for $TASK_TYPE"
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
date -u +%Y-%m-%dT%H:%M:%SZ >"$WORKSPACE_PATH/started_at"
printf 'QUEUED\n' >"$WORKSPACE_PATH/status"
printf '%s\n' "$MODEL" >"$WORKSPACE_PATH/model"
printf '%s\n' "$ROLE" >"$WORKSPACE_PATH/role"

cat >"$WORKSPACE_PATH/AUTOSHIP_PROMPT.md" <<EOF
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
$BODY

## Instructions
- Work only in this worktree: $FULL_WORKSPACE_PATH
- Implement the issue per its acceptance criteria.
- Run relevant project checks before finishing.
- Commit changes on branch autoship/issue-$ISSUE_NUM.
- Write AUTOSHIP_RESULT.md in the worktree.
- Write COMPLETE, BLOCKED, or STUCK to $FULL_WORKSPACE_PATH/status.

## PR Title
Use this conventional PR title when creating a PR:
$(bash "$SCRIPT_DIR/pr-title.sh" --issue "$ISSUE_NUM" --title "$TITLE" --labels "$LABELS")
EOF

autoship_state_set set-queued "$ISSUE_KEY" agent="$MODEL" model="$MODEL" role="$ROLE" task_type="$TASK_TYPE"
bash "$SCRIPT_DIR/metrics-collector.sh" record-dispatch "$ISSUE_KEY" "$MODEL" >/dev/null 2>&1 || true

echo "Queued issue #$ISSUE_NUM for $MODEL ($TASK_TYPE, role=$ROLE)"
[[ -n "$cap_note" ]] && echo "$cap_note"
echo "Worktree: $FULL_WORKSPACE_PATH"
echo "Run: bash hooks/opencode/runner.sh"
