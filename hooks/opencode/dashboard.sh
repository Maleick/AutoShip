#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
STATE_FILE="$REPO_ROOT/.autoship/state.json"
LOG_FILE="$REPO_ROOT/.autoship/logs/events.jsonl"
MODEL_HISTORY="$REPO_ROOT/.autoship/model-history.json"

echo "AutoShip Dashboard"
echo "=================="
if [[ -f "$STATE_FILE" ]]; then
  jq -r '"Repo: \(.repo // "unknown")\nQueued: \([.issues[]? | select(.state == "queued")] | length)\nRunning: \([.issues[]? | select(.state == "running")] | length)\nCompleted: \([.issues[]? | select(.state == "completed")] | length)\nBlocked: \([.issues[]? | select(.state == "blocked")] | length)"' "$STATE_FILE"
fi
if [[ -f "$MODEL_HISTORY" ]]; then
  echo
  echo "Model failures:"
  jq -r 'to_entries[] | "- \(.key): \(.value.fail // 0)"' "$MODEL_HISTORY" 2>/dev/null || true
fi
if [[ -f "$LOG_FILE" ]]; then
  echo
  echo "Recent events:"
  tail -20 "$LOG_FILE" | jq -r '"- \(.timestamp) \(.event) \(.issue): \(.message)"' 2>/dev/null || tail -20 "$LOG_FILE"
fi
