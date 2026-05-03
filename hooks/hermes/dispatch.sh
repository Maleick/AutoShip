#!/usr/bin/env bash
# Hermes agent dispatch — create worktree and queue issue for Hermes cron execution
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load shared utilities if available
if [[ -f "$SCRIPT_DIR/../lib/common.sh" ]]; then
  source "$SCRIPT_DIR/../lib/common.sh"
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
ISSUE_KEY="issue-${ISSUE_NUM}"
WORKSPACE_PATH="$AUTOSHIP_DIR/workspaces/$ISSUE_KEY"
REPO="${HERMES_TARGET_REPO:-Maleick/TextQuest}"

# Check if Hermes is available
HERMES_AVAILABLE=false
if command -v hermes &>/dev/null; then
  HERMES_AVAILABLE=true
fi

if [[ "$HERMES_AVAILABLE" != true ]]; then
  mkdir -p "$WORKSPACE_PATH"
  printf 'BLOCKED\n' > "$WORKSPACE_PATH/status"
  printf 'Hermes CLI not found. Install with: npm install -g hermes-agent\n' > "$WORKSPACE_PATH/BLOCKED_REASON.txt"
  autoship_state_set set-blocked "$ISSUE_KEY" reason="hermes CLI not found"
  echo "BLOCKED $ISSUE_KEY: hermes CLI not found"
  exit 0
fi

# Check max concurrent (Hermes default: 3)
MAX=3
running=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "running")] | length' "$STATE_FILE" 2>/dev/null || echo 0)
if [[ ! "$running" =~ ^[0-9]+$ ]]; then
  running=0
fi

cap_note=""
if (( running >= MAX )); then
  cap_note="CAP_REACHED: $running active / $MAX max; workspace will remain queued"
fi

TITLE=$(gh issue view "$ISSUE_NUM" --repo "$REPO" --json title --jq '.title' 2>/dev/null || echo "Issue $ISSUE_NUM")
BODY=$(gh issue view "$ISSUE_NUM" --repo "$REPO" --json body --jq '.body' 2>/dev/null || echo "")
LABELS=$(gh issue view "$ISSUE_NUM" --repo "$REPO" --json labels --jq '[.labels[].name] | join(",")' 2>/dev/null || echo "")

# Hermes uses the provider/model from config.yaml — no per-issue model selection
MODEL="hermes/default"
ROLE="implementer"

if [[ "$DRY_RUN" == true ]]; then
  echo "Dry run: would dispatch issue #$ISSUE_NUM to Hermes ($TASK_TYPE)"
  echo "Prompt path: $WORKSPACE_PATH/HERMES_PROMPT.md"
  echo "Worktree path: $WORKSPACE_PATH"
  echo "Status path: $WORKSPACE_PATH/status"
  exit 0
fi

# Create worktree using shared hook (Hermes uses autoship prefix for compatibility)
FULL_WORKSPACE_PATH=$(bash "$SCRIPT_DIR/../opencode/create-worktree.sh" "$ISSUE_KEY" "autoship/issue-${ISSUE_NUM}")
mkdir -p "$WORKSPACE_PATH"
date -u +%Y-%m-%dT%H:%M:%SZ > "$WORKSPACE_PATH/started_at"
printf 'QUEUED\n' > "$WORKSPACE_PATH/status"
printf '%s\n' "$MODEL" > "$WORKSPACE_PATH/model"
printf '%s\n' "$ROLE" > "$WORKSPACE_PATH/role"

# Write Hermes-specific prompt
cat > "$WORKSPACE_PATH/HERMES_PROMPT.md" <<EOF
# Hermes Agent Prompt

## Issue #$ISSUE_NUM: $TITLE

## Labels
$LABELS

## Task Type
$TASK_TYPE

## Selected Model
$MODEL (inherited from ~/.hermes/config.yaml)

## Specialized Role
$ROLE

## Body
$BODY

## Instructions
- Work only in this worktree: $FULL_WORKSPACE_PATH
- Implement the issue per its acceptance criteria.
- Run relevant project checks before finishing.
- Commit changes on branch hermes/issue-$ISSUE_NUM.
- Write HERMES_RESULT.md in the worktree.
- Write COMPLETE, BLOCKED, or STUCK to $FULL_WORKSPACE_PATH/status.

## PR Title
Use this conventional PR title when creating a PR:
$(bash "$SCRIPT_DIR/../opencode/pr-title.sh" --issue "$ISSUE_NUM" --title "$TITLE" --labels "$LABELS")

## Hermes-Specific Notes
- Hermes runs with toolsets: terminal, file, web, delegation
- One phase per cron run — if interrupted, resume on next run
- Use [SILENT] for no-op phases
- Cargo check before cargo test (orchestrator is Windows-only)
EOF

autoship_state_set set-queued "$ISSUE_KEY" agent="$MODEL" model="$MODEL" role="$ROLE" task_type="$TASK_TYPE"

echo "Queued issue #$ISSUE_NUM for Hermes ($TASK_TYPE, role=$ROLE)"
[[ -n "$cap_note" ]] && echo "$cap_note"
echo "Worktree: $FULL_WORKSPACE_PATH"
echo "Dispatch via: hermes cronjob or delegate_task"
