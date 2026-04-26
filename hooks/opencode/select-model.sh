#!/usr/bin/env bash
set -euo pipefail

ROLE=""
POOL=""
LOG=false
if [[ "${1:-}" == "--role" ]]; then
  ROLE="${2:?Role required}"
  shift 2
fi
if [[ "${1:-}" == "--pool" ]]; then
  POOL="${2:?Pool required}"
  shift 2
fi
if [[ "${1:-}" == "--log" ]]; then
  LOG=true
  shift
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

if [[ -n "$POOL" ]]; then
  jq -r --arg pool "$POOL" '.pools[$pool].models[] // empty' "$ROUTING_FILE" 2>/dev/null || exit 0
  exit 0
fi

if [[ "$LOG" == true ]]; then
  jq -r --arg task "$TASK_TYPE" --argjson issue "$ISSUE_NUM" --slurpfile history "$HISTORY_FILE" '
    def hist($id):
      (($history[0] // {})[$id] // {success: 0, fail: 0});
    def compatible:
      (.enabled // true) == true
      and (((.max_task_types // []) | length == 0) or ((.max_task_types // []) | index($task) != null));
    def cost_score:
      if .cost == "free" then 100
      elif (.id | test("(^|/)gpt-5\\.3-codex-spark$|spark"; "i")) then 85
      elif .cost == "go" or (.id | startswith("opencode-go/")) then 80
      elif (.cost // "") == "selected" then 70
      else 50 end;
    def reason:
      if .cost == "free" then "free model selected by default"
      elif .cost == "go" then "OpenCode Go model selected as subscription fallback"
      elif (.id | test("(^|/)gpt-5\\.3-codex-spark$|spark"; "i")) then "Spark model selected for complex task suitability"
      elif (.cost // "") == "selected" then "operator-selected model for task"
      else "model selected as fallback" end;
    . as $routing |
    [($routing.models // [])[] | select(compatible) |
      . as $m |
      (hist($m.id)) as $h |
      select((($h.fail // 0) | tonumber) < 3) |
      .score = ((cost_score) + (.strength // 0) + (($h.success // 0) * 12) - (($h.fail // 0) * 20))
      | .reason = reason
    ] | sort_by(-.score, .id) |
    . as $candidates |
    (($issue % (length | if . == 0 then 1 else . end))) as $idx |
    if length > 0 and ($task != "complex" or (($candidates[0].strength // 0) >= 80)) then
      "routing_log:" +
      (map("\nselection: \(.id)\nscore: \(.score)\nreason: \(.reason)") | join("")) +
      "\nround_robin_index: " + ($idx | tostring) +
      "\nfinal_selection: " + $candidates[$idx].id +
      "\nfinal_reason: " + $candidates[$idx].reason
    elif $task == "complex" and (($routing.roles.orchestrator // "") != "") then
      "routing_log:" +
      (map("\nselection: \(.id)\nscore: \(.score)\nreason: below complex task strength threshold") | join("")) +
      "\nfinal_selection: " + $routing.roles.orchestrator +
      "\nfinal_reason: orchestrator advisor selected for complex task"
    else
      "routing_log:\nfinal_selection:"
    end
  ' "$ROUTING_FILE"
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
    elif .cost == "go" or (.id | startswith("opencode-go/")) then 80
    elif (.cost // "") == "selected" then 70
    else 50 end;
  . as $routing |
  [($routing.models // [])[] | select(compatible) |
    . as $m |
    (hist($m.id)) as $h |
    select((($h.fail // 0) | tonumber) < 3) |
    .score = ((cost_score) + (.strength // 0) + (($h.success // 0) * 12) - (($h.fail // 0) * 20))]
  | sort_by(-.score, .id)
  | . as $candidates
  | if length > 0 and ($task != "complex" or (($candidates[0].strength // 0) >= 80)) then .[$issue % length].id
    elif $task == "complex" and (($routing.roles.orchestrator // "") != "") then $routing.roles.orchestrator
    else empty end
' "$ROUTING_FILE"
