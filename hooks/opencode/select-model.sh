#!/usr/bin/env bash
set -euo pipefail

ROLE=""
if [[ "${1:-}" == "--role" ]]; then
  ROLE="${2:?Role required}"
  shift 2
fi

TASK_TYPE="${1:-simple_code}"
ISSUE_NUM="${2:-0}"
ROUTING_FILE=".autoship/model-routing.json"
HISTORY_FILE=".autoship/model-history.json"

[[ -f "$ROUTING_FILE" ]] || exit 0
if [[ ! -f "$HISTORY_FILE" ]]; then
  HISTORY_FILE="/dev/null"
fi

if [[ -n "$ROLE" ]]; then
  jq -r --arg role "$ROLE" '.roles[$role] // empty' "$ROUTING_FILE"
  exit 0
fi

jq -r --arg task "$TASK_TYPE" --argjson issue "$ISSUE_NUM" --slurpfile history "$HISTORY_FILE" '
  def hist($id):
    (($history[0] // {})[$id] // {success: 0, fail: 0});
  def compatible:
    (.enabled // true) == true
    and (((.max_task_types // []) | length == 0) or ((.max_task_types // []) | index($task) != null));
  def cost_score:
    if .cost == "free" then 100
    elif (.id | test("(^|/)gpt-5\\.3-codex-spark$|spark"; "i")) then 85
    elif (.id | startswith("opencode-go/")) then 80
    elif (.cost // "") == "selected" then 70
    else 50 end;
  [(.models // [])[] | select(compatible) |
    . as $m |
    (hist($m.id)) as $h |
    .score = ((cost_score) + (.strength // 0) + (($h.success // 0) * 12) - (($h.fail // 0) * 20))]
  | sort_by(-.score, .id)
  | if length > 0 then .[0].id else empty end
' "$ROUTING_FILE"
