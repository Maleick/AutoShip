#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
AB_TEST_FILE="$REPO_ROOT/.autoship/ab-test.json"
ROUTING_FILE="$REPO_ROOT/config/model-routing.json"

mkdir -p "$(dirname "$AB_TEST_FILE")"

[[ -f "$AB_TEST_FILE" ]] || cat >"$AB_TEST_FILE" <<'JSON'
{
  "groups": {"A": [], "B": []},
  "performance": {},
  "model_scores": {},
  "enabled": true,
  "split_ratio": 0.5
}
JSON

action="${1:-}"
shift || true

case "$action" in
  assign)
    issue_key="${1:-}"
    task_type="${2:-medium_code}"
    [[ -z "$issue_key" ]] && exit 0

    # Deterministic assignment based on issue number hash
    issue_num=$(echo "$issue_key" | sed 's/issue-//')
    hash=$(printf '%s' "$issue_key" | cksum | awk '{print $1}')
    group=$((hash % 2 == 0 ? 0 : 1))
    group_name=$(if [[ "$group" -eq 0 ]]; then echo "A"; else echo "B"; fi)

    tmp=$(mktemp)
    jq --arg issue "$issue_key" --arg group "$group_name" --arg task "$task_type" '
      .groups[$group] = ((.groups[$group] // []) + [{issue: $issue, task_type: $task, assigned_at: now | todateiso8601}])
    ' "$AB_TEST_FILE" >"$tmp" && mv "$tmp" "$AB_TEST_FILE"

    echo "$group_name"
    ;;

  select-model)
    task_type="${1:-medium_code}"
    group="${2:-A}"
    [[ -f "$ROUTING_FILE" ]] || exit 0

    # Get models for this task type
    models=$(jq -r --arg task "$task_type" '
      [(.models // [])[] | select((.enabled // true) == true)] |
      select(((.max_task_types // []) | length == 0) or ((.max_task_types // []) | index($task) != null)) |
      map(.id)
    ' "$ROUTING_FILE" 2>/dev/null || echo "[]")

    model_count=$(echo "$models" | jq 'length')
    if [[ "$model_count" -lt 2 ]]; then
      echo "$models" | jq -r '.[0] // empty'
      exit 0
    fi

    # Apply A/B split: group A gets first half, group B gets second half
    if [[ "$group" == "A" ]]; then
      selected=$(echo "$models" | jq -r '.[:((length / 2) | ceil)][0] // empty')
    else
      selected=$(echo "$models" | jq -r '.[((length / 2) | ceil):][0] // empty')
    fi

    # If no model selected, fall back to first available
    if [[ -z "$selected" ]]; then
      selected=$(echo "$models" | jq -r '.[0] // empty')
    fi

    echo "$selected"
    ;;

  record-result)
    issue_key="${1:-}"
    model="${2:-}"
    task_type="${3:-medium_code}"
    outcome="${4:-pass}"
    [[ -z "$issue_key" || -z "$model" ]] && exit 0

    tmp=$(mktemp)
    jq --arg issue "$issue_key" --arg model "$model" --arg task "$task_type" --arg outcome "$outcome" '
      .performance[$model] = ((.performance[$model] // {}) + {
        total: (((.performance[$model].total // 0) | tonumber) + 1),
        pass: (((.performance[$model].pass // 0) | tonumber) + (if $outcome == "pass" then 1 else 0 end)),
        fail: (((.performance[$model].fail // 0) | tonumber) + (if $outcome == "fail" then 1 else 0 end))
      }) |
      .performance[$model][$task] = ((.performance[$model][$task] // {}) + {
        total: (((.performance[$model][$task].total // 0) | tonumber) + 1),
        pass: (((.performance[$model][$task].pass // 0) | tonumber) + (if $outcome == "pass" then 1 else 0 end)),
        fail: (((.performance[$model][$task].fail // 0) | tonumber) + (if $outcome == "fail" then 1 else 0 end))
      })
    ' "$AB_TEST_FILE" >"$tmp" && mv "$tmp" "$AB_TEST_FILE"

    # Auto-adjust model scores based on performance
    bash "$0" adjust-scores >/dev/null 2>&1 || true
    ;;

  adjust-scores)
    [[ -f "$ROUTING_FILE" ]] || exit 0

    tmp=$(mktemp)
    jq -s '
      .[0] as $routing |
      .[1] as $ab |
      $routing | .models = [.models[] | . as $m |
        if ($ab.performance[$m.id] // {}).total > 0 then
          ($ab.performance[$m.id].pass // 0) as $passes |
          ($ab.performance[$m.id].fail // 0) as $fails |
          ($ab.performance[$m.id].total // 1) as $total |
          ($passes / $total) as $rate |
          .ab_score = ($rate * 100 | floor) |
          .strength = ((.strength // 0) + (.ab_score // 0))
        else
          .
        end
      ]
    ' "$ROUTING_FILE" "$AB_TEST_FILE" >"$tmp" && mv "$tmp" "$ROUTING_FILE"
    ;;

  status)
    cat "$AB_TEST_FILE"
    ;;

  *)
    echo "Usage: $0 {assign|select-model|record-result|adjust-scores|status} [args...]"
    exit 1
    ;;
esac
