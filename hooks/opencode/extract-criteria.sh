#!/usr/bin/env bash
set -euo pipefail

extract_criteria() {
  local body="$1"
  local line
  local in_criteria=false
  local in_oos=false
  local criteria_lines=()
  local oos_lines=()
  local test_req="false"

  while IFS= read -r line; do
    local lower
    lower=$(echo "$line" | tr '[:upper:]' '[:lower:]')

    if echo "$lower" | grep -Eq '^##\s*(acceptance|criteria)'; then
      in_criteria=true
      in_oos=false
      continue
    fi

    if echo "$lower" | grep -Eq '^##\s*out\s*of\s*scope'; then
      in_criteria=false
      in_oos=true
      continue
    fi

    if echo "$lower" | grep -Eq '^##\s*'; then
      in_criteria=false
      in_oos=false
    fi

    if [[ "$in_criteria" == "true" ]]; then
      if echo "$line" | grep -Eq '^\s*-\s+'; then
        criteria_lines+=("$line")
      fi
    fi

    if [[ "$in_oos" == "true" ]]; then
      if echo "$line" | grep -Eq '^\s*-\s+'; then
        oos_lines+=("$line")
      fi
    fi
  done <<< "$body"

  if echo "$body" | grep -Ei 'test|verify|evidence|check' >/dev/null 2>&1; then
    test_req="true"
  fi

  local criteria_json
  if [[ ${#criteria_lines[@]} -gt 0 ]]; then
    criteria_json=$(printf '%s\n' "${criteria_lines[@]}" | jq -R . | jq -s .)
  else
    criteria_json='[]'
  fi

  local oos_json
  if [[ ${#oos_lines[@]} -gt 0 ]]; then
    oos_json=$(printf '%s\n' "${oos_lines[@]}" | jq -R . | jq -s .)
  else
    oos_json='[]'
  fi

  jq -n \
    --argjson criteria "$criteria_json" \
    --argjson oos "$oos_json" \
    --argjson test_req "$test_req" \
    '{
      criteria: $criteria,
      out_of_scope: $oos,
      files_likely_touched: [],
      test_evidence_required: $test_req
    }'
}

criteria_to_prompt() {
  local json="$1"
  local prompt="## Acceptance Criteria"

  local criteria_str
  criteria_str=$(echo "$json" | jq -r '.criteria | join("\n- ")' 2>/dev/null || echo "")
  if [[ "$criteria_str" != "null" && -n "$criteria_str" ]]; then
    prompt+="\n- $criteria_str"
  fi

  local oos_str
  oos_str=$(echo "$json" | jq -r '.out_of_scope | join("\n- ")' 2>/dev/null || echo "")
  if [[ "$oos_str" != "null" && -n "$oos_str" ]]; then
    prompt+="\n\n## Out of Scope\n- $oos_str"
  fi

  local test_req
  test_req=$(echo "$json" | jq -r '.test_evidence_required' 2>/dev/null || echo "false")
  if [[ "$test_req" == "true" ]]; then
    prompt+="\n\n## Test Evidence Required\nProvide evidence that tests passed."
  fi

  printf '%s' "$prompt"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  COMMAND="${1:-}"
  shift || true

  case "$COMMAND" in
    extract)
      BODY="${1:-}"
      [[ -z "$BODY" ]] && { echo "Usage: $0 extract <body>"; exit 1; }
      extract_criteria "$BODY"
      ;;
    prompt)
      JSON="${1:-}"
      [[ -z "$JSON" ]] && { echo "Usage: $0 prompt <json>"; exit 1; }
      criteria_to_prompt "$JSON"
      ;;
    *)
      BODY="${1:-}"
      [[ -z "$BODY" ]] && { echo "Usage: $0 <command> [args...]"; exit 1; }
      extract_criteria "$BODY"
      ;;
  esac
fi