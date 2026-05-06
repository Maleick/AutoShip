#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
CIRCUIT_FILE="$REPO_ROOT/.autoship/circuit-breaker.json"

mkdir -p "$(dirname "$CIRCUIT_FILE")"

[[ -f "$CIRCUIT_FILE" ]] || printf '{"models":{},"global_disabled_until":null}\n' >"$CIRCUIT_FILE"

action="${1:-}"
shift || true

case "$action" in
  record-failure)
    model="${1:-}"
    [[ -z "$model" ]] && exit 0
    tmp=$(mktemp)
    now_epoch=$(date +%s)
    jq --arg model "$model" --argjson now "$now_epoch" '
      .models[$model] = ((.models[$model] // {}) + {
        consecutive_failures: (((.models[$model].consecutive_failures // 0) | tonumber) + 1),
        last_failed_at: $now
      }) |
      if (.models[$model].consecutive_failures // 0) > 3 then
        .models[$model].disabled_until = ($now + 600)
      else
        .
      end
    ' "$CIRCUIT_FILE" >"$tmp" && mv "$tmp" "$CIRCUIT_FILE"
    ;;

  record-success)
    model="${1:-}"
    [[ -z "$model" ]] && exit 0
    tmp=$(mktemp)
    jq --arg model "$model" '
      if .models[$model] then
        .models[$model].consecutive_failures = 0 |
        .models[$model].disabled_until = null
      else
        .
      end
    ' "$CIRCUIT_FILE" >"$tmp" && mv "$tmp" "$CIRCUIT_FILE"
    ;;

  is-open)
    model="${1:-}"
    [[ -z "$model" ]] && {
      echo "true"
      exit 0
    }
    now_epoch=$(date +%s)
    disabled_until=$(jq -r --arg model "$model" --argjson now "$now_epoch" '
      if .models[$model] then
        if ((.models[$model].disabled_until // 0) > $now) then
          .models[$model].disabled_until
        else
          null
        end
      else
        null
      end
    ' "$CIRCUIT_FILE" 2>/dev/null || echo "null")
    if [[ "$disabled_until" == "null" || -z "$disabled_until" ]]; then
      echo "true"
    else
      echo "false"
    fi
    ;;

  status)
    cat "$CIRCUIT_FILE"
    ;;

  *)
    echo "Usage: $0 {record-failure|record-success|is-open|status} [model]"
    exit 1
    ;;
esac
