#!/usr/bin/env bash
set -euo pipefail

PR_NUMBER="${1:-}"
ISSUE_KEY="${2:-}"

if [[ -z "$PR_NUMBER" || -z "$ISSUE_KEY" ]]; then
  echo "Usage: $0 <pr-number> <issue-key>" >&2
  exit 1
fi

if [[ ! "$PR_NUMBER" =~ ^[0-9]+$ ]]; then
  echo "Error: invalid PR number: $PR_NUMBER" >&2
  exit 1
fi

if [[ ! "$ISSUE_KEY" =~ ^issue-[0-9]+$ ]]; then
  echo "Error: invalid issue key: $ISSUE_KEY" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Do not pass --delete-branch here: gh attempts local branch deletion before
# AutoShip removes the issue worktree, which fails while that branch is checked out.
gh pr merge "$PR_NUMBER" --squash
bash "$SCRIPT_DIR/cleanup-worktree.sh" "$ISSUE_KEY"
