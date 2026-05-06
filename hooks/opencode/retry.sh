#!/usr/bin/env bash
set -euo pipefail

ISSUE_KEY="issue-${1#issue-}"
ATTEMPT="${2:-1}"

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
WORKSPACE_DIR="$REPO_ROOT/.autoship/workspaces/$ISSUE_KEY"

mkdir -p "$WORKSPACE_DIR"

# Validate and normalize attempt to base-10 integer
if [[ ! "$ATTEMPT" =~ ^[0-9]+$ ]]; then
  ATTEMPT=1
else
  ATTEMPT=$((10#$ATTEMPT))
fi

# Clamp to supported retry range: 1..5
if [[ "$ATTEMPT" -lt 1 ]]; then
  ATTEMPT=1
elif [[ "$ATTEMPT" -gt 5 ]]; then
  ATTEMPT=5
fi

# Exponential backoff: 1min, 2min, 4min, 8min, 16min
delay_seconds=$((60 * (2 ** (ATTEMPT - 1))))

# Add jitter: random 0-30% variation to prevent thundering herd
jitter=$((delay_seconds * (RANDOM % 30) / 100))
delay_seconds=$((delay_seconds + jitter))

retry_after_epoch=$(($(date +%s) + delay_seconds))
retry_after=$(date -u -r "$retry_after_epoch" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d "@${retry_after_epoch}" +%Y-%m-%dT%H:%M:%SZ)

printf 'QUEUED\n' >"$WORKSPACE_DIR/status"
printf '%s\n' "$retry_after" >"$WORKSPACE_DIR/retry_after"

bash "$REPO_ROOT/hooks/update-state.sh" set-queued "$ISSUE_KEY" retry=true retry_after="$retry_after" retry_attempt="$ATTEMPT" >/dev/null 2>&1 || true

echo "Queued retry for $ISSUE_KEY (attempt $ATTEMPT, retry after $retry_after, delay ${delay_seconds}s)"
