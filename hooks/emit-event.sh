#!/usr/bin/env bash
# emit-event.sh — Atomically append one event to .autoship/event-queue.json
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
#   AUTOSHIP_ROOT — repo root (default: current directory)

set -euo pipefail

EVENT="${1:?usage: emit-event.sh '<json-event-string>'}"
REPO_ROOT="${AUTOSHIP_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || echo ".")}"
REPO_ROOT=$(cd "$REPO_ROOT" && pwd -P)
AUTOSHIP_DIR="$REPO_ROOT/.autoship"
QUEUE="$AUTOSHIP_DIR/event-queue.json"
LOCK="$AUTOSHIP_DIR/event-queue.lock"

# Refuse to operate on symlinked AutoShip state paths to prevent clobbering arbitrary files
if [[ -L "$AUTOSHIP_DIR" || -L "$QUEUE" || -L "$LOCK" ]]; then
  echo "Refusing to write through symlinked AutoShip state path: $AUTOSHIP_DIR" >&2
  exit 1
fi

# Ensure queue exists as a valid JSON array
if [[ ! -f "$QUEUE" ]] || ! jq -e 'type == "array"' "$QUEUE" >/dev/null 2>&1; then
  QUEUE_TMP=$(mktemp "${QUEUE}.tmp.XXXXXX")
  printf '[]\n' > "$QUEUE_TMP"
  mv "$QUEUE_TMP" "$QUEUE"
fi
touch "$LOCK"

# flock (Linux) / lockf (macOS BSD) fallback
if command -v flock >/dev/null 2>&1; then
  QUEUE_TMP=$(mktemp "${QUEUE}.tmp.XXXXXX")
  (
    flock -x 9
    jq --argjson evt "$EVENT" '. + [$evt]' "$QUEUE" > "$QUEUE_TMP" \
      && mv "$QUEUE_TMP" "$QUEUE"
  ) 9>"$LOCK"
elif command -v lockf >/dev/null 2>&1; then
  # Pass paths/event as positional args ($1,$2,$3) to avoid shell injection from special chars
  QUEUE_TMP=$(mktemp "${QUEUE}.tmp.XXXXXX")
  lockf -k "$LOCK" bash -c '
    evt="$1" queue="$2" qtmp="$3"
    jq --argjson evt "$evt" '"'"'. + [$evt]'"'"' "$queue" > "$qtmp" && mv "$qtmp" "$queue"
  ' _ "$EVENT" "$QUEUE" "$QUEUE_TMP"
else
  # No locking available — best-effort append
  QUEUE_TMP=$(mktemp "${QUEUE}.tmp.XXXXXX")
  jq --argjson evt "$EVENT" '. + [$evt]' "$QUEUE" > "$QUEUE_TMP" \
    && mv "$QUEUE_TMP" "$QUEUE"
fi
