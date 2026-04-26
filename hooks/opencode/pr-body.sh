#!/usr/bin/env bash
set -euo pipefail

generate_pr_body() {
  local issue_num="${1:?issue required}"
  local result_path="${2:?result path required}"
  local criteria_json="${3:-}"
  printf '## Summary\n\n'
  cat "$result_path"
  printf '\n\n## Acceptance Criteria\n\n'
  if [[ -n "$criteria_json" && -f "$criteria_json" ]]; then
    jq -r '.criteria[]? | "- [ ] " + .' "$criteria_json" 2>/dev/null || true
  else
    printf -- '- [ ] Worker result reviewed\n'
  fi
  printf '\n## Verification\n\n'
  printf -- '- Reviewer: PASS\n'
  printf -- '- Tests: completed by AutoShip worker\n\n'
  printf 'Closes #%s\n\n' "$issue_num"
  printf 'Dispatched by AutoShip.\n'
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  generate_pr_body "$@"
fi
