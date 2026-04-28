#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
QUOTA_FILE="$REPO_ROOT/.autoship/quota.json"
STATE_FILE="$REPO_ROOT/.autoship/state.json"
THRESHOLD="${AUTOSHIP_QUOTA_PAUSE_THRESHOLD:-95}"

quota_raw=$(jq -r '.quota_pct // .usage_pct // -1' "$QUOTA_FILE" 2>/dev/null || echo -1)
if [[ "$quota_raw" =~ ^-?[0-9]+([.][0-9]+)?$ ]]; then
  quota="$quota_raw"
else
  quota="-1"
fi

if [[ "$THRESHOLD" =~ ^-?[0-9]+([.][0-9]+)?$ ]]; then
  threshold="$THRESHOLD"
else
  threshold="95"
fi

if awk -v quota="$quota" -v threshold="$threshold" 'BEGIN { exit !(quota >= threshold && quota >= 0) }'; then
  tmp=$(mktemp)
  jq '.paused = true | .pause_reason = "quota guardrail"' "$STATE_FILE" > "$tmp" && mv "$tmp" "$STATE_FILE"
  echo "AutoShip paused: quota ${quota}% >= ${threshold}%"
  exit 1
fi
echo "Quota OK: ${quota}%"
