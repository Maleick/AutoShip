#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
QUOTA_FILE="$REPO_ROOT/.autoship/quota.json"
STATE_FILE="$REPO_ROOT/.autoship/state.json"
THRESHOLD="${AUTOSHIP_QUOTA_PAUSE_THRESHOLD:-95}"

quota=$(jq -r '.quota_pct // .usage_pct // -1' "$QUOTA_FILE" 2>/dev/null || echo -1)
if awk "BEGIN { exit !($quota >= $THRESHOLD && $quota >= 0) }"; then
  tmp=$(mktemp)
  jq '.paused = true | .pause_reason = "quota guardrail"' "$STATE_FILE" > "$tmp" && mv "$tmp" "$STATE_FILE"
  echo "AutoShip paused: quota ${quota}% >= ${THRESHOLD}%"
  exit 1
fi
echo "Quota OK: ${quota}%"
