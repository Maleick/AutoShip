#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
STATE_FILE="$REPO_ROOT/.autoship/state.json"
source "$REPO_ROOT/hooks/opencode/log.sh" 2>/dev/null || true

prs=$(gh pr list --label autoship --state open --json number,statusCheckRollup --limit 100 2>/dev/null || echo '[]')
echo "$prs" | jq -c '.[]' | while IFS= read -r pr; do
  num=$(jq -r '.number' <<< "$pr")
  conclusion=$(jq -r '[.statusCheckRollup[]? | .conclusion // .status // empty] | join(",")' <<< "$pr")
  if [[ -z "$conclusion" ]]; then
    autoship_log ci_pending "pr-$num" "no checks reported" 2>/dev/null || true
    echo "PR #$num: pending"
  elif grep -Eq 'FAILURE|failure|ERROR|error|CANCELLED|cancelled' <<< "$conclusion"; then
    autoship_log ci_failed "pr-$num" "$conclusion" 2>/dev/null || true
    echo "PR #$num: failed checks: $conclusion"
  elif grep -Eq 'PENDING|pending|QUEUED|queued|IN_PROGRESS|in_progress|STARTED|started' <<< "$conclusion"; then
    autoship_log ci_pending "pr-$num" "$conclusion" 2>/dev/null || true
    echo "PR #$num: pending checks: $conclusion"
  else
    autoship_log ci_passed "pr-$num" "$conclusion" 2>/dev/null || true
    echo "PR #$num: checks passed"
  fi
done
