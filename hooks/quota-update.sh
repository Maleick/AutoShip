#!/usr/bin/env bash
set -euo pipefail

# quota-update.sh — Compatibility wrapper for OpenCode-only AutoShip.
# OpenCode provider/model availability is detected by hooks/opencode/setup.sh and
# hooks/detect-tools.sh. This hook keeps older calls harmless.

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
QUOTA_FILE="$REPO_ROOT/.autoship/quota.json"
STATE_FILE="$REPO_ROOT/.autoship/state.json"

ensure_init() {
  mkdir -p "$(dirname "$QUOTA_FILE")"
  if [[ ! -f "$QUOTA_FILE" ]]; then
    jq -n '{opencode: {available: true, quota_pct: -1, quota_source: "provider", dispatches: 0}}' >"$QUOTA_FILE"
  fi
}

cmd="${1:-check}"
shift

case "$cmd" in
  init | refresh)
    rm -f "$QUOTA_FILE"
    ensure_init
    echo "OpenCode provider status initialized"
    ;;
  check)
    ensure_init
    jq '.' "$QUOTA_FILE"
    ;;
  decrement)
    ensure_init
    state_dispatches=0
    if [[ -f "$STATE_FILE" ]]; then
      state_dispatches=$(jq -r '.stats.session_dispatched // .stats.total_dispatched_all_time // 0' "$STATE_FILE" 2>/dev/null || echo 0)
    fi
    [[ "$state_dispatches" =~ ^[0-9]+$ ]] || state_dispatches=0
    jq --argjson state_dispatches "$state_dispatches" \
      '.opencode.dispatches = (((.opencode.dispatches // 0) as $current | if $current > $state_dispatches then $current else $state_dispatches end) + 1)' \
      "$QUOTA_FILE" >"$QUOTA_FILE.tmp" && mv "$QUOTA_FILE.tmp" "$QUOTA_FILE"
    echo "Recorded OpenCode dispatch"
    ;;
  stuck)
    ensure_init
    echo "OpenCode worker stuck event recorded by workspace status"
    ;;
  set | reset | advisor-call)
    ensure_init
    echo "No provider quota mutation required for OpenCode-only AutoShip"
    ;;
  *)
    echo "Usage: $0 {init|check|refresh|decrement|stuck|set|reset|advisor-call}" >&2
    exit 2
    ;;
esac
