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
# Validate ISSUE_NUM is numeric
if [[ ! "$ISSUE_NUM" =~ ^[0-9]+$ ]]; then
  ISSUE_NUM=0
fi
ROUTING_FILE=".autoship/model-routing.json"
if [[ ! -f "$ROUTING_FILE" && -f "config/model-routing.json" ]]; then
  ROUTING_FILE="config/model-routing.json"
fi
HISTORY_FILE=".autoship/model-history.json"
CIRCUIT_FILE=".autoship/circuit-breaker.json"

[[ -f "$ROUTING_FILE" ]] || exit 0
if [[ ! -f "$HISTORY_FILE" ]]; then
  HISTORY_FILE="/dev/null"
fi
if [[ ! -f "$CIRCUIT_FILE" ]]; then
  CIRCUIT_FILE="/dev/null"
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
  # Shared jq filter definitions
  JQ_DEFS='
def hist($id):
  (($history[0] // {})[$id] // {success: 0, fail: 0});
def circuit_open($id):
  if ($circuit[0] // {}) | has($id) then
    ((($circuit[0] // {})[$id].disabled_until // 0) | tonumber) > ($now | tonumber)
  else
    false
  end;
def compatible:
  (.enabled // true) == true
  and (((.max_task_types // []) | length == 0) or ((.max_task_types // []) | index($task) != null))
  and (circuit_open(.id) | not);
def cost_score:
  if .cost == "free" then 100
  elif (.id | test("(^|/)gpt-5\\.3-spark$|spark"; "i")) then 85
  elif (.id | startswith("opencode-go/")) then 80
  elif (.cost // "") == "selected" then 70
  else 50 end;
def reason:
  if .cost == "free" then "free model selected by default"
  elif (.id | test("(^|/)gpt-5\\.3-spark$|spark"; "i")) then "Spark model selected for complex task suitability"
  elif (.cost // "") == "selected" then "operator-selected model for task"
  else "model selected as fallback" end;
def scored_model:
  . as $m |
  (hist($m.id)) as $h |
  .score = ((cost_score) + (.strength // 0) + (($h.success // 0) * 12) - (($h.fail // 0) * 20)) |
  .reason = reason;
def compatible_models:
  [(.models // [])[] | select(compatible) | scored_model];
def sorted_models:
  compatible_models | sort_by(-(.score // 0), .id);
def advisor_model:
  .roles.orchestrator // .roles.lead // .roles.planner // empty;
def advisor_available:
  (advisor_model != "")
  and (circuit_open(advisor_model) | not)
  and ([(.models // [])[] | select(.id == advisor_model and ((.enabled // true) == false))] | length == 0);
def rotated_models:
  sorted_models as $models |
  if $task == "complex" and (($models | length) == 0 or (($models[0].strength // 0) < 70)) and advisor_available then
    [{id: advisor_model, score: 0, reason: "orchestrator advisor selected for complex task"}]
  elif ($models | length) > 0 then
    ($issue % ($models | length)) as $offset |
    $models[$offset:] + $models[:$offset]
  else
    []
  end;
'

  if [[ "$LOG" == true ]]; then
    jq -r --arg task "$TASK_TYPE" --argjson issue "$ISSUE_NUM" --slurpfile history "$HISTORY_FILE" --slurpfile circuit "$CIRCUIT_FILE" --argjson now "$(date +%s)" "${JQ_DEFS}
rotated_models |
. as \$candidates |
if length > 0 then
  \"routing_log:\" +
  (map(\"\\nselection: \\(.id)\\nscore: \\(.score)\\nreason: \\(.reason)\") | join(\"\")) +
  \"\\nfinal_selection: \" + \$candidates[0].id +
  \"\\nfinal_reason: \" + \$candidates[0].reason
else
  \"routing_log:\\nfinal_selection:\"
end" "$ROUTING_FILE"
    exit 0
  fi

  jq -r --arg task "$TASK_TYPE" --argjson issue "$ISSUE_NUM" --slurpfile history "$HISTORY_FILE" --slurpfile circuit "$CIRCUIT_FILE" --argjson now "$(date +%s)" "${JQ_DEFS}
rotated_models |
if length > 0 then .[0].id else empty end" "$ROUTING_FILE"
  exit 0
fi

jq -r --arg task "$TASK_TYPE" --argjson issue "$ISSUE_NUM" --slurpfile history "$HISTORY_FILE" --slurpfile circuit "$CIRCUIT_FILE" --argjson now "$(date +%s)" '
  def hist($id):
    (($history[0] // {})[$id] // {success: 0, fail: 0});
  def circuit_open($id):
    if ($circuit[0] // {}) | has($id) then
      ((($circuit[0] // {})[$id].disabled_until // 0) | tonumber) > ($now | tonumber)
    else
      false
    end;
  def compatible:
    (.enabled // true) == true
    and (((.max_task_types // []) | length == 0) or ((.max_task_types // []) | index($task) != null))
    and (circuit_open(.id) | not);
  def cost_score:
    if .cost == "free" then 100
    elif (.id | test("(^|/)gpt-5\\.3-spark$|spark"; "i")) then 85
    elif (.id | startswith("opencode-go/")) then 80
    elif (.cost // "") == "selected" then 70
    else 50 end;
  def sorted_models:
    [(.models // [])[] | select(compatible) |
      . as $m |
      (hist($m.id)) as $h |
      .score = ((cost_score) + (.strength // 0) + (($h.success // 0) * 12) - (($h.fail // 0) * 20))]
    | sort_by(-(.score // 0), .id);
  def advisor_model:
    .roles.orchestrator // .roles.lead // .roles.planner // empty;
  def advisor_available:
    (advisor_model != "")
    and (circuit_open(advisor_model) | not)
    and ([(.models // [])[] | select(.id == advisor_model and ((.enabled // true) == false))] | length == 0);
  def rotated_models:
    sorted_models as $models |
    if $task == "complex" and (($models | length) == 0 or (($models[0].strength // 0) < 70)) and advisor_available then
      [{id: advisor_model}]
    elif ($models | length) > 0 then
      ($issue % ($models | length)) as $offset |
      $models[$offset:] + $models[:$offset]
    else
      []
    end;
  rotated_models
  | if length > 0 then .[0].id else empty end
' "$ROUTING_FILE"
