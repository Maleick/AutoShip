#!/usr/bin/env bash
set -euo pipefail

AUTOSHIP_LOG_DIR="${AUTOSHIP_LOG_DIR:-.autoship/logs}"
AUTOSHIP_LOG_FILE="${AUTOSHIP_LOG_FILE:-$AUTOSHIP_LOG_DIR/events.jsonl}"

json_escape() {
  jq -Rsa . <<< "${1:-}"
}

autoship_log() {
  local event="${1:?event required}"
  local issue="${2:-}"
  local message="${3:-}"
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  mkdir -p "$(dirname "$AUTOSHIP_LOG_FILE")"
  jq -cn \
    --arg ts "$now" \
    --arg event "$event" \
    --arg issue "$issue" \
    --arg message "$message" \
    '{timestamp:$ts,event:$event,issue:$issue,message:$message}' >> "$AUTOSHIP_LOG_FILE"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  autoship_log "${1:-event}" "${2:-}" "${3:-}"
fi
