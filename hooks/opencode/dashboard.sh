#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
STATE_FILE="$REPO_ROOT/.autoship/state.json"
LOG_FILE="$REPO_ROOT/.autoship/logs/events.jsonl"
MODEL_HISTORY="$REPO_ROOT/.autoship/model-history.json"
METRICS_FILE="$REPO_ROOT/.autoship/metrics.json"

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
if [[ -f "$METRICS_FILE" ]]; then
  echo
  echo "Metrics:"
  jq -r '
    "Dispatched: \(.aggregated.total_dispatched // 0) | Completed: \(.aggregated.total_completed // 0) | Failed: \(.aggregated.total_failed // 0) | Blocked: \(.aggregated.total_blocked // 0)" +
    "\nAvg completion: \((.aggregated.avg_completion_time_ms // 0) / 1000 | tostring) sec | Total tokens: \(.aggregated.total_tokens_used // 0)"
  ' "$METRICS_FILE" 2>/dev/null || true
  echo
  echo "Model performance:"
  jq -r '
    .models | to_entries[] |
    "- \(.key): success=\(.value.success // 0) fail=\(.value.fail // 0) " +
    "avg_time=\(if (.value.total_duration_ms // 0) > 0 and (.value.success // 0) > 0 then (.value.total_duration_ms / .value.success / 1000 | tostring + " sec") else "N/A" end) " +
    "tokens=\(.value.total_tokens // 0)"
  ' "$METRICS_FILE" 2>/dev/null || true
fi
if [[ -f "$LOG_FILE" ]]; then
  echo
  echo "Recent events:"
  tail -20 "$LOG_FILE" | jq -r '"- \(.timestamp) \(.event) \(.issue): \(.message)"' 2>/dev/null || tail -20 "$LOG_FILE"
fi
