#!/usr/bin/env bash
# Dependency graph: lib/common.sh (optional), pr-title.sh, update-state.sh
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
    bash "$repo_root/hooks/update-state.sh" "$action" "$issue_key" "$@" 2>/dev/null || true
  }
fi

ISSUE_KEY="${1:?Issue key required}"
WORKTREE_PATH="${2:?Worktree path required}"
RESULT_PATH="${3:-$WORKTREE_PATH/AUTOSHIP_RESULT.md}"
MODE="${AUTOSHIP_PR_MODE:-dry-run}"

REPO_ROOT=$(autoship_repo_root) || exit 1

canonical_dir() {
  cd "$1" && pwd -P
}

canonical_file() {
  readlink -f -- "$1"
}

is_runtime_artifact() {
  case "$1" in
    .autoship | .autoship/* | AUTOSHIP_PROMPT.md | AUTOSHIP_RESULT.md | AUTOSHIP_RUNNER.log | BLOCKED_REASON.txt | model | role | routing-log.txt | started_at | status | worker.pid)
      return 0
      ;;
  esac
  return 1
}

implementation_changes() {
  git -C "$WORKTREE_PATH" status --porcelain | while IFS= read -r line; do
    path="${line#???}"
    case "$line" in
      R* | C*) path="${path#* -> }" ;;
    esac
    if ! is_runtime_artifact "$path"; then
      printf '%s\n' "$path"
    fi
  done
}

base_ref_for_worktree() {
  for ref in origin/main origin/master HEAD~1 main master; do
    if git -C "$WORKTREE_PATH" rev-parse --verify "$ref" >/dev/null 2>&1; then
      printf '%s\n' "$ref"
      return 0
    fi
  done
  return 1
}

committed_implementation_changes() {
  local base_ref path
  base_ref=$(base_ref_for_worktree || true)
  [[ -n "$base_ref" ]] || return 0
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    if ! is_runtime_artifact "$path"; then
      printf '%s\n' "$path"
    fi
  done < <(git -C "$WORKTREE_PATH" diff --name-only "$base_ref"...HEAD 2>/dev/null || git -C "$WORKTREE_PATH" diff --name-only "$base_ref" HEAD 2>/dev/null || true)
}

if [[ ! -d "$WORKTREE_PATH" ]]; then
  echo "VERDICT: FAIL - worktree missing"
  exit 1
fi
if [[ ! -s "$RESULT_PATH" ]]; then
  echo "VERDICT: FAIL - AUTOSHIP_RESULT.md missing"
  exit 1
fi
if [[ -L "$RESULT_PATH" ]]; then
  echo "VERDICT: FAIL - AUTOSHIP_RESULT.md must not be a symlink"
  exit 1
fi

REAL_WORKTREE=$(canonical_dir "$WORKTREE_PATH")
REAL_RESULT=$(canonical_file "$RESULT_PATH") || {
  echo "VERDICT: FAIL - cannot resolve AUTOSHIP_RESULT.md"
  exit 1
}
case "$REAL_RESULT" in
  "$REAL_WORKTREE"/*) ;;
  *)
    echo "VERDICT: FAIL - path outside worktree"
    exit 1
    ;;
esac

CHANGED_PATHS=$(implementation_changes)
COMMITTED_PATHS=$(committed_implementation_changes)
if [[ -z "$CHANGED_PATHS" && -z "$COMMITTED_PATHS" ]]; then
  echo "VERDICT: FAIL - git diff is empty"
  exit 1
fi

ISSUE_NUMBER="${ISSUE_KEY#issue-}"
# Determine target repo from state file or default to current repo
REPO_SLUG=$(jq -r '.repo // empty' "$REPO_ROOT/.autoship/state.json" 2>/dev/null || echo "")
if [[ -z "$REPO_SLUG" ]]; then
  REPO_SLUG=$(git remote get-url origin 2>/dev/null | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
fi
TITLE=$(bash "$SCRIPT_DIR/pr-title.sh" --issue "$ISSUE_NUMBER" --repo "$REPO_SLUG")
BODY_FILE=$(mktemp)
trap 'rm -f "$BODY_FILE"' EXIT
CRITERIA_FILE="$WORKTREE_PATH/acceptance-criteria.json"
if [[ -x "$SCRIPT_DIR/pr-body.sh" ]]; then
  bash "$SCRIPT_DIR/pr-body.sh" "$ISSUE_NUMBER" "$RESULT_PATH" "$CRITERIA_FILE" >"$BODY_FILE"
else
  {
    printf '## Summary\n'
    cat "$RESULT_PATH"
    printf '\n\nCloses #%s\n\n' "$ISSUE_NUMBER"
    printf 'Dispatched by AutoShip.\n'
  } >"$BODY_FILE"
fi

if [[ "$MODE" != "live" && "${AUTOSHIP_ENABLE_PR_CREATE:-false}" != "true" ]]; then
  autoship_state_set set-completed "$ISSUE_KEY" pr_mode=dry-run pr_title="$TITLE"
  echo "DRY_RUN: would create PR for $ISSUE_KEY"
  echo "Title: $TITLE"
  echo "Body file: $BODY_FILE"
  exit 0
fi

(
  cd "$WORKTREE_PATH"
  if [[ -n "$CHANGED_PATHS" ]]; then
    while IFS= read -r path; do
      [[ -n "$path" ]] || continue
      git add -- "$path"
    done <<<"$CHANGED_PATHS"
    if ! git diff --cached --quiet; then
      git commit -m "$TITLE" -m "Closes #$ISSUE_NUMBER" -m "Dispatched by AutoShip."
    fi
  fi
)

PR_URL=$(gh pr create \
  --title "$TITLE" \
  --body-file "$BODY_FILE" \
  --label autoship \
  --head "autoship/issue-$ISSUE_NUMBER")

issue_meta=$(gh issue view "$ISSUE_NUMBER" --json labels,milestone 2>/dev/null || echo '{}')
labels=$(jq -r '[.labels[].name? | select(. != "agent:ready")] | join(",")' <<<"$issue_meta" 2>/dev/null || true)
milestone=$(jq -r '.milestone.title // empty' <<<"$issue_meta" 2>/dev/null || true)
if [[ -n "$labels" ]]; then
  gh pr edit "$PR_URL" --add-label "$labels" >/dev/null 2>&1 || true
fi
if [[ -n "$milestone" ]]; then
  gh pr edit "$PR_URL" --milestone "$milestone" >/dev/null 2>&1 || true
fi

PR_NUMBER=$(printf '%s\n' "$PR_URL" | grep -Eo '[0-9]+$' | tail -1 || true)
if [[ -n "$PR_NUMBER" ]]; then
  autoship_state_set set-completed "$ISSUE_KEY" pr_mode=live pr_number="$PR_NUMBER"
else
  autoship_state_set set-completed "$ISSUE_KEY" pr_mode=live
fi

printf '%s\n' "$PR_URL"
