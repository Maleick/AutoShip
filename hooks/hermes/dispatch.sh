#!/usr/bin/env bash
# Hermes agent dispatch — create worktree, write prompt, and dispatch via delegate_task
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load shared utilities if available
if [[ -f "$SCRIPT_DIR/../lib/common.sh" ]]; then
  # shellcheck disable=SC1091
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

# ── Auto-sync: pull latest plugin code before dispatch ──
if [[ -z "${AUTOSHIP_NO_SYNC:-}" ]]; then
  # Only sync if we have a valid git remote (skip temp/policy-test repos)
  if git rev-parse --verify HEAD >/dev/null 2>&1 && git remote get-url origin >/dev/null 2>&1; then
    sync_gap=$(git log --oneline HEAD..origin/main 2>/dev/null | wc -l | tr -d ' ')
    if [[ "$sync_gap" =~ ^[0-9]+$ && "$sync_gap" -gt 0 ]]; then
      echo "[autoship-sync] $sync_gap commit(s) behind origin/main — pulling..."
      git pull origin main >/dev/null 2>&1 || echo "[autoship-sync] WARN: git pull failed, continuing with local code"
    fi
  fi
fi

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
ISSUE_KEY="issue-${ISSUE_NUM}"
WORKSPACE_PATH="$AUTOSHIP_DIR/workspaces/$ISSUE_KEY"
# Auto-detect target repo: default to the current repo (AutoShip) unless explicitly overridden.
# This ensures AutoShip improvements are dispatched to AutoShip, not TextQuest.
if [[ -n "${HERMES_TARGET_REPO:-}" ]]; then
  REPO="$HERMES_TARGET_REPO"
else
  # Derive owner/repo from current repo's origin remote
  CURRENT_REMOTE="$(git remote get-url origin 2>/dev/null || true)"
  if [[ "$CURRENT_REMOTE" =~ github\.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
    REPO_NAME="${BASH_REMATCH[2]}"
    REPO_NAME="${REPO_NAME%.git}"
    REPO="${BASH_REMATCH[1]}/${REPO_NAME}"
  else
    # Fallback to legacy default for backward compatibility
    REPO="Maleick/TextQuest"
  fi
fi
BASE_BRANCH="${HERMES_BASE_BRANCH:-}"
if [[ -z "$BASE_BRANCH" ]]; then
  BASE_BRANCH=$(gh repo view "$REPO" --json defaultBranchRef --jq '.defaultBranchRef.name' 2>/dev/null || true)
fi
if [[ -z "$BASE_BRANCH" ]]; then
  BASE_BRANCH=$(git symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##' || true)
fi
BASE_BRANCH="${BASE_BRANCH:-main}"

is_git_worktree_path() {
  local path="$1"
  [[ -n "$path" ]] && git -C "$path" rev-parse --is-inside-work-tree >/dev/null 2>&1
}

# Read Hermes max concurrent from config.yaml
MAX=20
if [[ -f "$HOME/.hermes/config.yaml" ]]; then
  config_max=$(grep 'max_concurrent_children' "$HOME/.hermes/config.yaml" | awk '{print $2}' | tr -d '"')
  if [[ "$config_max" =~ ^[0-9]+$ ]]; then
    MAX="$config_max"
  fi
fi

# Check if Hermes is available
HERMES_AVAILABLE=false
if command -v hermes &>/dev/null; then
  HERMES_AVAILABLE=true
fi

if [[ "$HERMES_AVAILABLE" != true ]]; then
  mkdir -p "$WORKSPACE_PATH"
  printf 'BLOCKED\n' >"$WORKSPACE_PATH/status"
  printf 'Hermes CLI not found. Install with: npm install -g hermes-agent\n' >"$WORKSPACE_PATH/BLOCKED_REASON.txt"
  autoship_state_set set-blocked "$ISSUE_KEY" reason="hermes CLI not found"
  echo "BLOCKED $ISSUE_KEY: hermes CLI not found"
  exit 0
fi

running=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "running")] | length' "$STATE_FILE" 2>/dev/null || echo 0)
if [[ ! "$running" =~ ^[0-9]+$ ]]; then
  running=0
fi

cap_note=""
if ((running >= MAX)); then
  cap_note="CAP_REACHED: $running active / $MAX max; workspace will remain queued"
fi

TITLE=$(gh issue view "$ISSUE_NUM" --repo "$REPO" --json title --jq '.title' 2>/dev/null || echo "Issue $ISSUE_NUM")
BODY=$(gh issue view "$ISSUE_NUM" --repo "$REPO" --json body --jq '.body' 2>/dev/null || echo "")
LABELS=$(gh issue view "$ISSUE_NUM" --repo "$REPO" --json labels --jq '[.labels[].name] | join(",")' 2>/dev/null || echo "")

# Determine model using routing config or override
if [[ -n "$MODEL_OVERRIDE" ]]; then
  MODEL="$MODEL_OVERRIDE"
else
  # Pass issue title and labels to intelligent router
  LABELS_JSON=$(echo "$LABELS" | python3 -c "import sys,json; print(json.dumps(sys.stdin.read().strip().split(',')))" 2>/dev/null || echo "[]")
  MODEL_OUTPUT="$(bash "$SCRIPT_DIR/model-router.sh" "$TITLE" "$LABELS_JSON" "$TASK_TYPE" 2>/dev/null || echo "kimi-k2.6")"
  MODEL="$MODEL_OUTPUT"
fi
ROLE="implementer"

# Validate model is non-empty
if [[ -z "$MODEL" ]]; then
  echo "Error: model selection returned empty" >&2
  exit 1
fi

# Log model selection
mkdir -p "$AUTOSHIP_DIR/logs"
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) issue=$ISSUE_NUM model=$MODEL" >>"$AUTOSHIP_DIR/logs/model-selection.log"

# Ensure workspace directory exists before writing model
mkdir -p "$WORKSPACE_PATH"

# Write model to workspace
printf '%s\n' "$MODEL" >"$WORKSPACE_PATH/model"

if [[ "$DRY_RUN" == true ]]; then
  echo "Dry run: would dispatch issue #$ISSUE_NUM to Hermes ($TASK_TYPE)"
  echo "Prompt path: $WORKSPACE_PATH/HERMES_PROMPT.md"
  echo "Worktree path: $WORKSPACE_PATH"
  echo "Status path: $WORKSPACE_PATH/status"
  exit 0
fi

# Determine target repo path for worktree creation
TARGET_REPO_PATH="${HERMES_TARGET_REPO_PATH:-}"
if [[ -z "$TARGET_REPO_PATH" ]]; then
  # Fallback: derive from HERMES_TARGET_REPO (owner/repo format)
  if [[ "$REPO" == */* ]]; then
    REPO_NAME="${REPO#*/}"
    for candidate in "$HOME/Projects/$REPO_NAME" "$HOME/projects/$REPO_NAME" "$HOME/$REPO_NAME"; do
      if is_git_worktree_path "$candidate"; then
        TARGET_REPO_PATH="$candidate"
        break
      fi
    done
  fi
fi
if ! is_git_worktree_path "$TARGET_REPO_PATH"; then
  echo "Error: target repo not found. Set HERMES_TARGET_REPO_PATH to the local clone path." >&2
  echo "Tried: $TARGET_REPO_PATH" >&2
  exit 1
fi

# Create worktree from target repo
TARGET_REPO_ROOT="$(cd "$TARGET_REPO_PATH" && git rev-parse --show-toplevel)"
cd "$TARGET_REPO_ROOT"
AUTOSHIP_WORKSPACE_ROOT="$REPO_ROOT/$AUTOSHIP_DIR"
mkdir -p "$AUTOSHIP_WORKSPACE_ROOT/workspaces"

# Fetch base branch in target repo
git fetch origin "$BASE_BRANCH" --quiet 2>/dev/null || true
BASE_REF="origin/$BASE_BRANCH"
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="origin/master"
fi
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="origin/main"
fi
if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  BASE_REF="HEAD"
fi

WORKSPACE_DIR="$AUTOSHIP_WORKSPACE_ROOT/workspaces/$ISSUE_KEY"
TARGET_BRANCH="autoship/issue-${ISSUE_NUM}"

# Remove stale worktree if exists
if git worktree list | grep -q "$WORKSPACE_DIR"; then
  git worktree remove --force "$WORKSPACE_DIR" >/dev/null 2>&1 || true
fi
rm -rf "$WORKSPACE_DIR"

# Create worktree from target repo
# Use branch-first creation first, then fallback to --force checkout.
if ! git worktree add -B "$TARGET_BRANCH" "$WORKSPACE_DIR" "$BASE_REF" 2>/dev/null; then
  echo "Warning: git worktree add failed for $WORKSPACE_DIR, retrying with force checkout..." >&2
  # Ensure the branch exists locally
  if ! git rev-parse --verify "$TARGET_BRANCH" >/dev/null 2>&1; then
    git branch "$TARGET_BRANCH" "$BASE_REF" >/dev/null 2>&1 || true
  fi
  # Try creating worktree with existing branch
  git worktree add --force "$WORKSPACE_DIR" "$TARGET_BRANCH" >/dev/null 2>&1 || {
    echo "Error: failed to create worktree for $ISSUE_KEY" >&2
    exit 1
  }
fi

# Return to AutoShip repo for state management
cd "$REPO_ROOT"

FULL_WORKSPACE_PATH="$WORKSPACE_DIR"
if [[ -z "$FULL_WORKSPACE_PATH" || ! -d "$FULL_WORKSPACE_PATH" ]]; then
  echo "Error: worktree path empty or missing after creation: '$FULL_WORKSPACE_PATH'" >&2
  exit 1
fi
date -u +%Y-%m-%dT%H:%M:%SZ >"$WORKSPACE_PATH/started_at"
printf 'QUEUED\n' >"$WORKSPACE_PATH/status"
printf '%s\n' "$ROLE" >"$WORKSPACE_PATH/role"

# Write Hermes-specific prompt with AutoShip constraints. Python avoids shell
# expansion of issue titles/bodies that may contain backticks or $().
HERMES_PROMPT_PATH="$WORKSPACE_PATH/HERMES_PROMPT.md" \
  HERMES_ISSUE_NUM="$ISSUE_NUM" \
  HERMES_TITLE="$TITLE" \
  HERMES_LABELS_VALUE="$LABELS" \
  HERMES_TASK_TYPE="$TASK_TYPE" \
  HERMES_MODEL="$MODEL" \
  HERMES_ROLE="$ROLE" \
  HERMES_BODY="$BODY" \
  HERMES_WORKTREE_PATH="$FULL_WORKSPACE_PATH" \
  HERMES_WORKSPACE_PATH="$WORKSPACE_PATH" \
  HERMES_BASE_BRANCH_VALUE="$BASE_BRANCH" \
  python3 - <<'PY'
import os
from pathlib import Path

issue_num = os.environ["HERMES_ISSUE_NUM"]
title = os.environ["HERMES_TITLE"]
labels = os.environ["HERMES_LABELS_VALUE"]
task_type = os.environ["HERMES_TASK_TYPE"]
model = os.environ["HERMES_MODEL"]
role = os.environ["HERMES_ROLE"]
body = os.environ["HERMES_BODY"]
worktree_path = os.environ["HERMES_WORKTREE_PATH"]
workspace_path = os.environ["HERMES_WORKSPACE_PATH"]
base_branch = os.environ["HERMES_BASE_BRANCH_VALUE"]
prompt = f"""# Hermes Agent Prompt — AutoShip Issue #{issue_num}

## Issue: {title}

## Labels
{labels}

## Task Type
{task_type}

## Model
{model} (inherited from ~/.hermes/config.yaml)

## Role
{role}

## Body
{body}

## CRITICAL: You MUST complete the full workflow

1. Implement the issue — edit code, add tests, update docs as needed
2. Run validation: `cargo fmt --check`, `cargo clippy --all-targets --all-features`, `cargo test --all --all-features` (or focused tests if full suite is too slow)
3. Commit source changes with a Conventional Commit message. Stage only implementation files and exclude AutoShip/Hermes runtime artifacts.
4. Push the branch: `git push origin $(git branch --show-current)`
5. Create a PR: `gh pr create --title "feat: {title}" --body "Closes #{issue_num}"`
6. Write `HERMES_RESULT.md` in the workspace root for orchestration, but do not stage or commit it. Include:
   - status: COMPLETE or BLOCKED
   - files_changed: list of files modified/created
   - validation_results: output of test commands
   - pr_url: the created PR URL
7. **MANDATORY FINAL STEP — Update status file**: `echo "COMPLETE" > {workspace_path}/status`
   - This step is REQUIRED. The runner cannot detect completion without it.
   - Write the status file LAST, after HERMES_RESULT.md and after the PR is created.
   - If you do not write this file, the runner will mark the issue STUCK and retry.

If you cannot complete, write HERMES_RESULT.md with status BLOCKED and reason, then write `echo "BLOCKED" > {workspace_path}/status`.

## Instructions
- Work only in this worktree: {worktree_path}
- Branch: autoship/issue-{issue_num}
- Implement per acceptance criteria.
- Run project checks: cargo fmt --check, cargo clippy --target x86_64-unknown-linux-gnu, cargo test --target x86_64-unknown-linux-gnu (or focused tests if full suite is too slow).
- Commit with conventional format: "feat|fix|docs|refactor(scope): description (#{issue_num})".
- Do not commit runtime artifacts: .autoship/, status, HERMES_PROMPT.md, HERMES_RESULT.md, runner.log, started_at, target-isolated/.
- **PUSH branch to origin**: `git push origin autoship/issue-{issue_num}`
- **CREATE PR via gh CLI**: `gh pr create --title "..." --body "Closes #{issue_num}" --base {base_branch} --head autoship/issue-{issue_num}`
- **DO NOT manually close the GitHub issue**. The `Closes #{issue_num}` PR body will close it only after the PR is merged. Manually closing issues while PRs are still open blocks follow-up automation.
- Write HERMES_RESULT.md in worktree root with: status (COMPLETE/BLOCKED/STUCK), files changed, validation results.
- Update {workspace_path}/status to COMPLETE, BLOCKED, or STUCK.
- If stuck at minute 8, stop and report STUCK with exact status.

## PR Title
PR_TITLE="AutoShip: {title} (#{issue_num})"

## Notes
- Hermes toolsets: terminal, file, web, delegation
- One phase per cron run — resume on next if interrupted
- Use [SILENT] for no-op phases
- Cargo check before cargo test (orchestrator is Windows-only, skip on macOS)
- Never claim Windows/live EQ validation unless actually performed
"""

Path(os.environ["HERMES_PROMPT_PATH"]).write_text(prompt, encoding="utf-8")
PY

autoship_state_set set-queued "$ISSUE_KEY" agent="$MODEL" model="$MODEL" role="$ROLE" task_type="$TASK_TYPE"

echo "Queued issue #$ISSUE_NUM for Hermes ($TASK_TYPE, role=$ROLE)"
[[ -n "$cap_note" ]] && echo "$cap_note"
echo "Worktree: $FULL_WORKSPACE_PATH"
echo "Prompt: $WORKSPACE_PATH/HERMES_PROMPT.md"

# If inside Hermes session, immediately dispatch with the production runner.
if [[ -n "${HERMES_SESSION_ID:-}" ]]; then
  echo "Hermes session detected — dispatching with Hermes runner..."
  bash "$SCRIPT_DIR/runner.sh" "$ISSUE_KEY"
fi
