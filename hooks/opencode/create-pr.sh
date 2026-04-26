#!/usr/bin/env bash
set -euo pipefail

ISSUE_KEY="${1:?Issue key required}"
WORKTREE_PATH="${2:?Worktree path required}"
RESULT_PATH="${3:-$WORKTREE_PATH/AUTOSHIP_RESULT.md}"
MODE="${AUTOSHIP_PR_MODE:-dry-run}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}

canonical_dir() {
  cd "$1" && pwd -P
}

canonical_file() {
  local dir base
  dir=$(dirname "$1")
  base=$(basename "$1")
  printf '%s/%s\n' "$(canonical_dir "$dir")" "$base"
}

is_runtime_artifact() {
  case "$1" in
    .autoship|.autoship/*|AUTOSHIP_PROMPT.md|AUTOSHIP_RESULT.md|AUTOSHIP_RUNNER.log|BLOCKED_REASON.txt|model|role|routing-log.txt|started_at|status|worker.pid)
      return 0
      ;;
  esac
  return 1
}

implementation_changes() {
  git -C "$WORKTREE_PATH" status --porcelain | while IFS= read -r line; do
    path="${line#???}"
    case "$line" in
      R*|C*) path="${path#* -> }" ;;
    esac
    if ! is_runtime_artifact "$path"; then
      printf '%s\n' "$path"
    fi
  done
}

if [[ ! -d "$WORKTREE_PATH" ]]; then
  echo "VERDICT: FAIL - worktree missing"
  exit 1
fi
if [[ ! -s "$RESULT_PATH" ]]; then
  echo "VERDICT: FAIL - AUTOSHIP_RESULT.md missing"
  exit 1
fi

REAL_WORKTREE=$(canonical_dir "$WORKTREE_PATH")
REAL_RESULT=$(canonical_file "$RESULT_PATH")
case "$REAL_RESULT" in
  "$REAL_WORKTREE"/*) ;;
  *)
    echo "VERDICT: FAIL - path outside worktree"
    exit 1
    ;;
esac

CHANGED_PATHS=$(implementation_changes)
if [[ -z "$CHANGED_PATHS" ]]; then
  echo "VERDICT: FAIL - git diff is empty"
  exit 1
fi

ISSUE_NUMBER="${ISSUE_KEY#issue-}"
TITLE=$(bash "$SCRIPT_DIR/pr-title.sh" --issue "$ISSUE_NUMBER")
BODY_FILE=$(mktemp)
trap 'rm -f "$BODY_FILE"' EXIT
CRITERIA_FILE="$WORKTREE_PATH/acceptance-criteria.json"
if [[ -x "$SCRIPT_DIR/pr-body.sh" ]]; then
  bash "$SCRIPT_DIR/pr-body.sh" "$ISSUE_NUMBER" "$RESULT_PATH" "$CRITERIA_FILE" > "$BODY_FILE"
else
  {
    printf '## Summary\n'
    cat "$RESULT_PATH"
    printf '\n\nCloses #%s\n\n' "$ISSUE_NUMBER"
    printf 'Dispatched by AutoShip.\n'
  } > "$BODY_FILE"
fi

if [[ "$MODE" != "live" && "${AUTOSHIP_ENABLE_PR_CREATE:-false}" != "true" ]]; then
  bash "$REPO_ROOT/hooks/update-state.sh" set-completed "$ISSUE_KEY" pr_mode=dry-run pr_title="$TITLE" 2>/dev/null || true
  echo "DRY_RUN: would create PR for $ISSUE_KEY"
  echo "Title: $TITLE"
  echo "Body file: $BODY_FILE"
  exit 0
fi

(
  cd "$WORKTREE_PATH"
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    git add -- "$path"
  done <<< "$CHANGED_PATHS"
  if ! git diff --cached --quiet; then
    git commit -m "$TITLE" -m "Closes #$ISSUE_NUMBER" -m "Dispatched by AutoShip."
  fi
)

PR_URL=$(gh pr create \
  --title "$TITLE" \
  --body-file "$BODY_FILE" \
  --label autoship \
  --head "autoship/issue-$ISSUE_NUMBER")

issue_meta=$(gh issue view "$ISSUE_NUMBER" --json labels,milestone 2>/dev/null || echo '{}')
labels=$(jq -r '[.labels[].name? | select(. != "agent:ready")] | join(",")' <<< "$issue_meta" 2>/dev/null || true)
milestone=$(jq -r '.milestone.title // empty' <<< "$issue_meta" 2>/dev/null || true)
if [[ -n "$labels" ]]; then
  gh pr edit "$PR_URL" --add-label "$labels" >/dev/null 2>&1 || true
fi
if [[ -n "$milestone" ]]; then
  gh pr edit "$PR_URL" --milestone "$milestone" >/dev/null 2>&1 || true
fi

PR_NUMBER=$(printf '%s\n' "$PR_URL" | grep -Eo '[0-9]+$' | tail -1 || true)
if [[ -n "$PR_NUMBER" ]]; then
  bash "$REPO_ROOT/hooks/update-state.sh" set-completed "$ISSUE_KEY" pr_mode=live pr_number="$PR_NUMBER" 2>/dev/null || true
else
  bash "$REPO_ROOT/hooks/update-state.sh" set-completed "$ISSUE_KEY" pr_mode=live 2>/dev/null || true
fi

printf '%s\n' "$PR_URL"
