#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/git_prune.sh [options]

Clean up merged PR branches and stale local branches.

Options:
  --base <branch>     Base branch to compare against (default: main, falls back to master)
  --days <n>          Consider local branches stale after n days without commits (default: 30)
  --protect <glob>    Protect branches matching this shell glob (repeatable)
  --no-default-protect
                      Disable the built-in protected globs
  --force-stale       Delete stale local branches even if unmerged and upstream still exists
  --apply             Execute deletes. Without this flag, runs in dry-run mode.
  --include-remote    Also attempt to delete merged remote branches (requires gh auth).
  -h, --help          Show this help text.

Examples:
  scripts/git_prune.sh
  scripts/git_prune.sh --apply
  scripts/git_prune.sh --protect 'feature/keep-*'
  scripts/git_prune.sh --apply --include-remote --days 45 --base main
USAGE
}

BASE_BRANCH=""
STALE_DAYS=30
APPLY=0
INCLUDE_REMOTE=0
FORCE_STALE=0
USE_DEFAULT_PROTECT=1
PROTECTED_GLOBS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base)
      BASE_BRANCH="${2:-}"
      shift 2
      ;;
    --days)
      STALE_DAYS="${2:-}"
      shift 2
      ;;
    --protect)
      PROTECTED_GLOBS+=("${2:-}")
      shift 2
      ;;
    --no-default-protect)
      USE_DEFAULT_PROTECT=0
      shift
      ;;
    --force-stale)
      FORCE_STALE=1
      shift
      ;;
    --apply)
      APPLY=1
      shift
      ;;
    --include-remote)
      INCLUDE_REMOTE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! [[ "$STALE_DAYS" =~ ^[0-9]+$ ]]; then
  echo "--days must be a non-negative integer." >&2
  exit 1
fi

if [[ "$USE_DEFAULT_PROTECT" -eq 1 ]]; then
  PROTECTED_GLOBS+=(
    "main"
    "master"
    "develop"
    "dev"
    "release/*"
    "hotfix/*"
    "codex/*"
    "copilot/*"
    "dependabot/*"
  )
fi

if [[ -z "$BASE_BRANCH" ]]; then
  if git show-ref --verify --quiet refs/heads/main; then
    BASE_BRANCH="main"
  elif git show-ref --verify --quiet refs/heads/master; then
    BASE_BRANCH="master"
  elif git symbolic-ref --quiet --short refs/remotes/origin/HEAD >/dev/null 2>&1; then
    BASE_BRANCH="$(git symbolic-ref --quiet --short refs/remotes/origin/HEAD | sed 's|^origin/||')"
  else
    echo "Could not auto-detect base branch. Pass --base <branch>." >&2
    exit 1
  fi
fi

if git show-ref --verify --quiet "refs/heads/$BASE_BRANCH"; then
  BASE_REF="$BASE_BRANCH"
elif git show-ref --verify --quiet "refs/remotes/origin/$BASE_BRANCH"; then
  BASE_REF="origin/$BASE_BRANCH"
else
  echo "Base branch '$BASE_BRANCH' was not found locally or on origin." >&2
  exit 1
fi

CURRENT_BRANCH="$(git branch --show-current)"
NOW_EPOCH="$(date +%s)"
CUTOFF_EPOCH=$((NOW_EPOCH - STALE_DAYS * 24 * 60 * 60))

if [[ -n "$CURRENT_BRANCH" ]]; then
  PROTECTED_GLOBS+=("$CURRENT_BRANCH")
fi
PROTECTED_GLOBS+=("$BASE_BRANCH")

run_or_echo() {
  if [[ "$APPLY" -eq 1 ]]; then
    "$@"
  else
    echo "[dry-run] $*"
  fi
}

is_protected_branch() {
  local branch="$1"
  local pattern
  for pattern in "${PROTECTED_GLOBS[@]}"; do
    [[ -z "$pattern" ]] && continue
    if [[ "$branch" == $pattern ]]; then
      return 0
    fi
  done
  return 1
}

branch_upstream() {
  git for-each-ref --format='%(upstream:short)' "refs/heads/$1"
}

upstream_is_gone() {
  local upstream="$1"
  [[ -z "$upstream" ]] && return 1
  ! git show-ref --verify --quiet "refs/remotes/$upstream"
}

branch_is_merged() {
  git merge-base --is-ancestor "$1" "$BASE_REF"
}

echo "== Syncing remotes =="
run_or_echo git fetch --prune --all

echo "Protected branch globs:"
for pattern in "${PROTECTED_GLOBS[@]}"; do
  echo "  - $pattern"
done

echo
echo "== Local merged branches (into $BASE_REF) =="
merged_found=0
while IFS= read -r branch; do
  [[ -z "$branch" ]] && continue
  if is_protected_branch "$branch"; then
    echo "Keeping protected merged branch: $branch"
    continue
  fi
  merged_found=1
  echo "Deleting merged local branch: $branch"
  run_or_echo git branch -d "$branch"
done < <(git for-each-ref --format='%(refname:short)' refs/heads --merged "$BASE_REF")

if [[ "$merged_found" -eq 0 ]]; then
  echo "No merged local branches to delete."
fi

echo
echo "== Local stale branches (>=$STALE_DAYS days old) =="
while IFS='|' read -r branch commit_epoch; do
  [[ -z "$branch" ]] && continue
  if is_protected_branch "$branch"; then
    echo "Keeping protected branch: $branch"
    continue
  fi
  if (( commit_epoch < CUTOFF_EPOCH )); then
    upstream="$(branch_upstream "$branch")"
    if branch_is_merged "$branch"; then
      stale_reason="merged into $BASE_REF"
    elif upstream_is_gone "$upstream"; then
      stale_reason="upstream missing (${upstream:-none})"
    elif [[ "$FORCE_STALE" -eq 1 ]]; then
      stale_reason="forced delete"
    else
      echo "Keeping stale branch with live history: $branch"
      continue
    fi
    last_commit_date="$(date -u -d "@$commit_epoch" +%Y-%m-%d 2>/dev/null || date -u -r "$commit_epoch" +%Y-%m-%d)"
    echo "Deleting stale local branch: $branch (last commit: $last_commit_date UTC, reason: $stale_reason)"
    run_or_echo git branch -D "$branch"
  fi
done < <(git for-each-ref --format='%(refname:short)|%(committerdate:unix)' refs/heads)

if [[ "$INCLUDE_REMOTE" -eq 1 ]]; then
  echo
  echo "== Remote merged PR branches =="
  if ! command -v gh >/dev/null 2>&1; then
    echo "gh CLI not found; skipping remote PR cleanup."
  else
    mapfile -t REMOTE_BRANCHES < <(gh pr list --state merged --limit 200 --json headRefName -q '.[].headRefName' 2>/dev/null || true)
    if [[ "${#REMOTE_BRANCHES[@]}" -eq 0 ]]; then
      echo "No merged PR branches returned by gh pr list (or gh auth missing)."
    else
      for branch in "${REMOTE_BRANCHES[@]}"; do
        [[ -z "$branch" ]] && continue
        if is_protected_branch "$branch"; then
          echo "Keeping protected remote branch: origin/$branch"
          continue
        fi
        echo "Deleting remote branch origin/$branch"
        run_or_echo git push origin --delete "$branch"
      done
    fi
  fi
fi

echo
echo "Done. Mode: $([[ "$APPLY" -eq 1 ]] && echo apply || echo dry-run)."
