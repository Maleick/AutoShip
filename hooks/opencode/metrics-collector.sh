#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
METRICS_FILE="$REPO_ROOT/.autoship/metrics.json"

mkdir -p "$(dirname "$METRICS_FILE")"

[[ -f "$METRICS_FILE" ]] || printf '{"models":{},"sessions":[],"aggregated":{"total_dispatched":0,"total_completed":0,"total_failed":0,"total_blocked":0,"avg_completion_time_ms":0,"total_tokens_used":0}}\n' > "$METRICS_FILE"

action="${1:-}"
shift || true

case "$action" in
  record-start)
    issue_key="${1:-}"
    model="${2:-}"
    task_type="${3:-medium_code}"
    [[ -z "$issue_key" || -z "$model" ]] && exit 0
    tmp=$(mktemp)
    jq --arg issue "$issue_key" --arg model "$model" --arg task "$task_type" --arg now "$(date -u +%Y-%m-%dT%H:%M:%SZ)" '
      .sessions += [{issue: $issue, model: $model, task_type: $task, started_at: $now}]
    ' "$METRICS_FILE" > "$tmp" && mv "$tmp" "$METRICS_FILE"
    ;;

  record-complete)
    issue_key="${1:-}"
    model="${2:-}"
    tokens_used="${3:-0}"
    [[ -z "$issue_key" || -z "$model" ]] && exit 0
    [[ "$tokens_used" =~ ^[0-9]+$ ]] || tokens_used=0
    tmp=$(mktemp)
    now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    jq --arg issue "$issue_key" --arg model "$model" --argjson tokens "$tokens_used" --arg now "$now" '
      (.sessions | map(select(.issue == $issue and .model == $model and (.completed_at == null))) | first) as $session |
      if $session then
        ($now | fromdateiso8601 // empty) as $end |
        ($session.started_at | fromdateiso8601 // empty) as $start |
        (if $end and $start then ($end - $start) * 1000 else null end) as $duration |
        .sessions = [.sessions[] | if . == $session then . + {completed_at: $now, tokens_used: $tokens, duration_ms: ($duration // 0)} else . end]
      else
        .
      end |
      .models[$model] = ((.models[$model] // {}) + {
        success: (((.models[$model].success // 0) | tonumber) + 1),
        total_tokens: (((.models[$model].total_tokens // 0) | tonumber) + $tokens),
        total_duration_ms: (((.models[$model].total_duration_ms // 0) | tonumber) + (($session.duration_ms // 0) | tonumber))
      }) |
      .aggregated.total_completed = ((.aggregated.total_completed // 0) + 1)
    ' "$METRICS_FILE" > "$tmp" && mv "$tmp" "$METRICS_FILE"
    ;;

  record-failure)
    issue_key="${1:-}"
    model="${2:-}"
    [[ -z "$issue_key" || -z "$model" ]] && exit 0
    tmp=$(mktemp)
    now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    jq --arg issue "$issue_key" --arg model "$model" --arg now "$now" '
      (.sessions | map(select(.issue == $issue and .model == $model and (.completed_at == null))) | first) as $session |
      if $session then
        .sessions = [.sessions[] | if . == $session then . + {failed_at: $now, completed_at: $now, duration_ms: 0} else . end]
      else
        .
      end |
      .models[$model] = ((.models[$model] // {}) + {
        fail: (((.models[$model].fail // 0) | tonumber) + 1)
      }) |
      .aggregated.total_failed = ((.aggregated.total_failed // 0) + 1)
    ' "$METRICS_FILE" > "$tmp" && mv "$tmp" "$METRICS_FILE"
    ;;

  record-blocked)
    issue_key="${1:-}"
    model="${2:-}"
    [[ -z "$issue_key" ]] && exit 0
    tmp=$(mktemp)
    jq --arg issue "$issue_key" --arg model "${model:-unknown}" '
      .aggregated.total_blocked = ((.aggregated.total_blocked // 0) + 1)
    ' "$METRICS_FILE" > "$tmp" && mv "$tmp" "$METRICS_FILE"
    ;;

  record-dispatch)
    issue_key="${1:-}"
    model="${2:-}"
    [[ -z "$issue_key" || -z "$model" ]] && exit 0
    tmp=$(mktemp)
    jq --arg issue "$issue_key" --arg model "$model" '
      .aggregated.total_dispatched = ((.aggregated.total_dispatched // 0) + 1)
    ' "$METRICS_FILE" > "$tmp" && mv "$tmp" "$METRICS_FILE"
    ;;

  get)
    cat "$METRICS_FILE"
    ;;

  *)
    echo "Usage: $0 {record-start|record-complete|record-failure|record-blocked|record-dispatch|get} [args...]"
    exit 1
    ;;
esac
