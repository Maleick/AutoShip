#!/usr/bin/env bash
# Bulk rebase open AutoShip PRs onto origin/master.
# Skip on conflict, label blocked.
set -uo pipefail

PARENT=/Users/maleick/Projects/TextQuest
cd "$PARENT"
git fetch origin master >/dev/null 2>&1
git fetch origin '+refs/heads/autoship/*:refs/remotes/origin/autoship/*' >/dev/null 2>&1

rebased=0
conflicted=0
skipped=0

for N in "$@"; do
  BRANCH=$(gh pr view "$N" --json headRefName -q .headRefName 2>/dev/null)
  if [[ -z "$BRANCH" ]]; then
    echo "SKIP $N: PR not found"
    ((skipped++)); continue
  fi
  WS="/tmp/rebase-$N"

  rm -rf "$WS"
  # Create local tracking branch if missing
  git rev-parse --verify "refs/heads/$BRANCH" >/dev/null 2>&1 || \
    git branch -f "$BRANCH" "origin/$BRANCH" 2>/dev/null
  if ! git worktree add "$WS" "$BRANCH" 2>/dev/null; then
    echo "SKIP $N: worktree add failed"
    ((skipped++)); continue
  fi

  if git -C "$WS" rebase -X ours origin/master >/dev/null 2>&1; then
    if git -C "$WS" push --force-with-lease origin "$BRANCH" >/dev/null 2>&1; then
      echo "REBASED $N"
      ((rebased++))
    else
      echo "PUSH-FAIL $N"
      ((skipped++))
    fi
  else
    git -C "$WS" rebase --abort 2>/dev/null
    echo "CONFLICT $N"
    gh pr edit "$N" --add-label "autoship:blocked" 2>/dev/null
    ((conflicted++))
  fi

  git worktree remove "$WS" --force 2>/dev/null
  rm -rf "$WS"
done

echo "---"
echo "Rebased: $rebased | Conflicted (labeled blocked): $conflicted | Skipped: $skipped"
