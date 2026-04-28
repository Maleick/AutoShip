#!/usr/bin/env bash
set -euo pipefail
ISSUE_KEY="issue-${1#issue-}"
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
mkdir -p "$REPO_ROOT/.autoship/workspaces/$ISSUE_KEY"
printf 'QUEUED\n' > "$REPO_ROOT/.autoship/workspaces/$ISSUE_KEY/status"
bash "$REPO_ROOT/hooks/update-state.sh" set-queued "$ISSUE_KEY" retry=true >/dev/null 2>&1 || true
echo "Queued retry for $ISSUE_KEY"
