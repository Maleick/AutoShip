#!/usr/bin/env bash
# emit-event.sh — Atomically append one event to .beacon/event-queue.json
#
# Usage:
#   bash hooks/emit-event.sh '<json-event-object>'
#
# Example:
#   bash hooks/emit-event.sh '{"type":"verify","issue":"issue-42","tokens_used":1200}'
#
# The event is appended to event-queue.json using flock to prevent
# concurrent writes from corrupting the JSON array.
#
# Environment:
#   BEACON_ROOT — repo root (default: current directory)

set -euo pipefail

EVENT="${1:?usage: emit-event.sh '<json-event-string>'}"
REPO_ROOT="${BEACON_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || echo ".")}"
QUEUE="${REPO_ROOT}/.beacon/event-queue.json"
LOCK="${REPO_ROOT}/.beacon/event-queue.lock"

# Ensure queue exists as a valid JSON array
if [[ ! -f "$QUEUE" ]] || ! jq -e 'type == "array"' "$QUEUE" >/dev/null 2>&1; then
  echo "[]" > "$QUEUE"
fi
touch "$LOCK"

# flock (Linux) / lockf (macOS BSD) fallback
if command -v flock >/dev/null 2>&1; then
  (
    flock -x 9
    jq --argjson evt "$EVENT" '. + [$evt]' "$QUEUE" > "${QUEUE}.tmp" \
      && mv "${QUEUE}.tmp" "$QUEUE"
  ) 9>"$LOCK"
elif command -v lockf >/dev/null 2>&1; then
  # Pass paths/event as positional args ($1,$2,$3) to avoid shell injection from special chars
  lockf -k "$LOCK" bash -c '
    evt="$1" queue="$2" qtmp="$3"
    jq --argjson evt "$evt" '"'"'. + [$evt]'"'"' "$queue" > "$qtmp" && mv "$qtmp" "$queue"
  ' _ "$EVENT" "$QUEUE" "${QUEUE}.tmp"
else
  # No locking available — best-effort append
  jq --argjson evt "$EVENT" '. + [$evt]' "$QUEUE" > "${QUEUE}.tmp" \
    && mv "${QUEUE}.tmp" "$QUEUE"
fi
