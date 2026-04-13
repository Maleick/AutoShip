#!/usr/bin/env bash
set -euo pipefail

# quota-update.sh — Manage per-tool quota estimates via decay model.
#
# Since third-party CLIs (Codex, Gemini) expose no machine-readable quota API,
# we track estimated usage: start at 100%, subtract a complexity-weighted cost
# per dispatch, and auto-reset daily (subscription tools renew each day).
#
# Usage:
#   quota-update.sh init                          # Create quota.json, all tools at 100
#   quota-update.sh decrement <tool> <complexity> # Subtract cost estimate
#   quota-update.sh set <tool> <value>            # Manually set a quota value (0-100)
#   quota-update.sh reset [tool]                  # Reset one tool or all to 100
#   quota-update.sh check                         # Print JSON of current quotas
#   quota-update.sh refresh                       # Auto-reset tools that crossed midnight
#   quota-update.sh stuck <tool>                  # Increment tool_stuck_count, mark exhausted if >= 3
#   quota-update.sh advisor-call                  # Increment advisor_calls_today counter

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo ".")

QUOTA_FILE="$REPO_ROOT/.autoship/quota.json"

# Parse --dry-run flag from arguments (can appear anywhere)
DRY_RUN=0
FILTERED_ARGS=()
for arg in "$@"; do
  if [[ "$arg" == "--dry-run" ]]; then
    DRY_RUN=1
  else
    FILTERED_ARGS+=("$arg")
  fi
done
set -- "${FILTERED_ARGS[@]+"${FILTERED_ARGS[@]}"}"

# ---------------------------------------------------------------------------
# Cost estimates by complexity (percentage points subtracted per dispatch)
# ---------------------------------------------------------------------------
cost_for_complexity() {
  case "$1" in
    simple)  echo "3"  ;;
    medium)  echo "8"  ;;
    complex) echo "15" ;;
    *)       echo "5"  ;;  # default: medium-light
  esac
}

# ---------------------------------------------------------------------------
# Ensure quota.json exists with defaults
# ---------------------------------------------------------------------------
ensure_init() {
  if [[ ! -f "$QUOTA_FILE" ]]; then
    mkdir -p "$(dirname "$QUOTA_FILE")"
    cat > "$QUOTA_FILE" << 'EOF'
{
  "codex-spark": {"quota_pct": 100, "dispatches": 0, "reset_date": "", "tool_stuck_count": 0, "exhausted": false},
  "codex-gpt":   {"quota_pct": 100, "dispatches": 0, "reset_date": "", "tool_stuck_count": 0, "exhausted": false},
  "gemini":      {"quota_pct": 100, "dispatches": 0, "reset_date": "", "tool_stuck_count": 0, "exhausted": false},
  "advisor_calls_today": 0,
  "last_advisor_reset": ""
}
EOF
    # Stamp today's date on all entries
    TODAY=$(date -u +"%Y-%m-%d")
    TMP=$(mktemp)
    jq --arg d "$TODAY" '
      to_entries | map(if .value | type == "object" then .value.reset_date = $d else . end) | from_entries |
      .last_advisor_reset = $d
    ' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"
  fi
  # Upgrade existing file with missing fields (skip if already up to date)
  if ! jq -e 'has("advisor_calls_today")' "$QUOTA_FILE" >/dev/null 2>&1; then
    local TMP; TMP=$(mktemp)
    jq '
      to_entries | map(
        if .value | type == "object" then
          .value.tool_stuck_count |= (. // 0) |
          .value.exhausted |= (. // false)
        else . end
      ) | from_entries |
      if .advisor_calls_today == null then .advisor_calls_today = 0 else . end |
      if .last_advisor_reset == null then .last_advisor_reset = "" else . end
    ' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"
  fi
}

# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------

cmd_init() {
  rm -f "$QUOTA_FILE"
  ensure_init
  echo "Quota initialized: all tools at 100%"
}

cmd_advisor_call() {
  ensure_init
  if [[ $DRY_RUN -eq 1 ]]; then
    local cur; cur=$(jq -r '.advisor_calls_today' "$QUOTA_FILE")
    echo "[dry-run] Would increment advisor_calls_today: $cur → $((cur + 1))"
    return 0
  fi
  TMP=$(mktemp)
  jq '.advisor_calls_today += 1' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"
  echo "Incremented advisor_calls_today"
}

cmd_decrement() {
  local tool="${1:-}"
  local complexity="${2:-medium}"
  if [[ -z "$tool" ]]; then
    echo "Usage: $0 decrement <tool> <complexity>" >&2
    exit 1
  fi
  ensure_init
  local cost
  cost=$(cost_for_complexity "$complexity")

  # Check tool exists in quota file
  if ! jq -e --arg t "$tool" '.[$t]' "$QUOTA_FILE" >/dev/null 2>&1; then
    echo "Warning: unknown tool '$tool', skipping quota decrement" >&2
    return 0
  fi

  local cur_pct
  cur_pct=$(jq -r --arg t "$tool" '.[$t].quota_pct' "$QUOTA_FILE")
  local new_pct=$(( cur_pct - cost < 0 ? 0 : cur_pct - cost ))

  if [[ $DRY_RUN -eq 1 ]]; then
    echo "[dry-run] Would decrement $tool by ${cost}%: ${cur_pct}% → ${new_pct}%"
    if (( new_pct <= 0 )); then echo "[dry-run] QUOTA_EXHAUSTED $tool" >&2
    elif (( new_pct <= 20 )); then echo "[dry-run] QUOTA_LOW $tool (${new_pct}%)" >&2; fi
    return 0
  fi

  TMP=$(mktemp)
  jq --arg tool "$tool" --argjson cost "$cost" '
    .[$tool].quota_pct = ([.[$tool].quota_pct - $cost, 0] | max) |
    .[$tool].dispatches += 1
  ' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"
  echo "Decremented $tool by ${cost}% → ${new_pct}% remaining"

  # Emit warning if below thresholds
  if (( new_pct <= 0 )); then
    echo "QUOTA_EXHAUSTED $tool" >&2
  elif (( new_pct <= 20 )); then
    echo "QUOTA_LOW $tool (${new_pct}%)" >&2
  fi
}

cmd_stuck() {
  local tool="${1:-}"
  if [[ -z "$tool" ]]; then
    echo "Usage: $0 stuck <tool>" >&2
    exit 1
  fi
  ensure_init

  # Check tool exists in quota file
  if ! jq -e --arg t "$tool" '.[$t]' "$QUOTA_FILE" >/dev/null 2>&1; then
    echo "Warning: unknown tool '$tool', skipping stuck update" >&2
    return 0
  fi

  TMP=$(mktemp)
  jq --arg tool "$tool" '
    .[$tool].tool_stuck_count += 1 |
    if .[$tool].tool_stuck_count >= 3 then .[$tool].exhausted = true else . end
  ' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"

  local count exhausted
  count=$(jq -r --arg t "$tool" '.[$t].tool_stuck_count' "$QUOTA_FILE")
  exhausted=$(jq -r --arg t "$tool" '.[$t].exhausted' "$QUOTA_FILE")

  echo "Tool $tool stuck count: $count (exhausted: $exhausted)"

  if [[ "$exhausted" == "true" ]]; then
    echo "TOOL_DEGRADED $tool" >&2
    # Emit event to event-queue
    local event
    event=$(jq -n --arg tool "$tool" --arg ts "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
      '{type: "TOOL_DEGRADED", tool: $tool, timestamp: $ts}')
    bash "$REPO_ROOT/hooks/emit-event.sh" "$event"
  fi
}

cmd_set() {
  local tool="${1:-}"
  local value="${2:-}"
  if [[ -z "$tool" || -z "$value" ]]; then
    echo "Usage: $0 set <tool> <0-100>" >&2
    exit 1
  fi
  ensure_init
  TMP=$(mktemp)
  jq --arg tool "$tool" --argjson val "$value" '
    .[$tool].quota_pct = ([[$val, 0] | max, 100] | min)
  ' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"
  echo "Set $tool quota to ${value}%"
}

cmd_reset() {
  local tool="${1:-}"
  ensure_init
  TODAY=$(date -u +"%Y-%m-%d")
  if [[ $DRY_RUN -eq 1 ]]; then
    if [[ -n "$tool" ]]; then
      local cur; cur=$(jq -r --arg t "$tool" '.[$t].quota_pct' "$QUOTA_FILE" 2>/dev/null || echo "?")
      echo "[dry-run] Would reset $tool: ${cur}% → 100% (dispatches → 0, stuck → 0)"
    else
      echo "[dry-run] Would reset all tools to 100% and advisor_calls_today to 0:"
      jq -r 'to_entries[] | select(.value | type == "object") | "  \(.key): \(.value.quota_pct)% → 100%"' "$QUOTA_FILE" 2>/dev/null || true
    fi
    return 0
  fi
  TMP=$(mktemp)
  if [[ -n "$tool" ]]; then
    jq --arg t "$tool" --arg d "$TODAY" '
      .[$t].quota_pct = 100 | .[$t].dispatches = 0 | .[$t].reset_date = $d | .[$t].tool_stuck_count = 0 | .[$t].exhausted = false
    ' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"
    echo "Reset $tool quota to 100%"
  else
    jq --arg d "$TODAY" '
      to_entries | map(
        if .value | type == "object" then
          .value.quota_pct = 100 | .value.dispatches = 0 | .value.reset_date = $d | .value.tool_stuck_count = 0 | .value.exhausted = false
        else . end
      ) | from_entries |
      .advisor_calls_today = 0 | .last_advisor_reset = $d
    ' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"
    echo "Reset all tool quotas to 100% and advisor counter to 0"
  fi
}

cmd_check() {
  ensure_init
  jq '.' "$QUOTA_FILE"
}

cmd_refresh() {
  # Auto-reset any tool whose reset_date is before today (crossed midnight).
  # Subscription tools (Codex Plus, Gemini Advanced) renew daily.
  ensure_init
  TODAY=$(date -u +"%Y-%m-%d")
  TMP=$(mktemp)
  local reset_count=0

  # Check each tool and reset if stale
  while IFS= read -r tool; do
    local reset_date
    reset_date=$(jq -r --arg t "$tool" '.[$t].reset_date // ""' "$QUOTA_FILE" 2>/dev/null || echo "")
    if [[ -n "$reset_date" && "$reset_date" < "$TODAY" ]]; then
      jq --arg t "$tool" --arg d "$TODAY" '
        .[$t].quota_pct = 100 | .[$t].dispatches = 0 | .[$t].reset_date = $d | .[$t].tool_stuck_count = 0 | .[$t].exhausted = false
      ' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE" && cp "$QUOTA_FILE" "$TMP"
      echo "Auto-reset $tool (was $reset_date → $TODAY)"
      (( reset_count++ )) || true
    fi
  done < <(jq -r 'to_entries[] | select(.value | type == "object") | .key' "$QUOTA_FILE")

  # Reset advisor counter if date changed
  local last_reset
  last_reset=$(jq -r '.last_advisor_reset // ""' "$QUOTA_FILE")
  if [[ -n "$last_reset" && "$last_reset" < "$TODAY" ]]; then
    jq --arg d "$TODAY" '.advisor_calls_today = 0 | .last_advisor_reset = $d' "$QUOTA_FILE" > "$TMP" && mv "$TMP" "$QUOTA_FILE"
    echo "Auto-reset advisor_calls_today counter (was $last_reset → $TODAY)"
    (( reset_count++ )) || true
  fi

  if (( reset_count == 0 )); then
    echo "No tools needed refresh (all current as of $TODAY)"
  fi
}

# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <init|decrement|set|reset|check|refresh|stuck|advisor-call> [args...] [--dry-run]" >&2
  echo "  --dry-run  Print what would happen without modifying quota.json" >&2
  exit 1
fi

case "$1" in
  init)         cmd_init ;;
  decrement)    shift; cmd_decrement "$@" ;;
  set)          shift; cmd_set "$@" ;;
  reset)        shift; cmd_reset "$@" ;;
  check)        cmd_check ;;
  refresh)      cmd_refresh ;;
  stuck)        shift; cmd_stuck "$@" ;;
  advisor-call) cmd_advisor_call ;;
  *)
    echo "Error: unknown command '$1'" >&2
    echo "Valid: init, decrement, set, reset, check, refresh, stuck, advisor-call" >&2
    exit 1
    ;;
esac
