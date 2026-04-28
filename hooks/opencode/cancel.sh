#!/usr/bin/env bash
set -euo pipefail
ISSUE_KEY="issue-${1#issue-}"
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
mkdir -p "$REPO_ROOT/.autoship/workspaces/$ISSUE_KEY"
printf 'BLOCKED\n' > "$REPO_ROOT/.autoship/workspaces/$ISSUE_KEY/status"
printf 'Cancelled by operator\n' > "$REPO_ROOT/.autoship/workspaces/$ISSUE_KEY/BLOCKED_REASON.txt"
bash "$REPO_ROOT/hooks/update-state.sh" set-blocked "$ISSUE_KEY" reason="cancelled by operator" >/dev/null 2>&1 || true
echo "Cancelled $ISSUE_KEY"
