#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "Error: not inside a git repository" >&2
  exit 1
}

REPORT_FILE="${1:-$REPO_ROOT/.autoship/reports/self-improvement.md}"
SAFETY_FILTER="$REPO_ROOT/hooks/opencode/safety-filter.sh"

[[ -f "$REPORT_FILE" ]] || { echo "No report found at $REPORT_FILE" >&2; exit 1; }
[[ -x "$SAFETY_FILTER" ]] || { echo "No safety filter found at $SAFETY_FILTER" >&2; exit 1; }

extract_section() {
  local heading="$1"
  awk -v heading="$heading" '
    $0 == heading {found=1; next}
    found && /^## / {exit}
    found {print}
  ' "$REPORT_FILE"
}

evidence="$(extract_section '## Root Cause Evidence' | sed '/^[[:space:]]*$/d' | head -20)"
affected="$(extract_section '## Affected Files' | sed '/^[[:space:]]*$/d' | head -20)"

extract_section '## Candidate Acceptance Criteria' |
  sed -n 's/^- //p' |
  while IFS= read -r criterion; do
    [[ -n "$criterion" ]] || continue
    title="AutoShip self-improvement: ${criterion}"
    body="$(cat <<EOF
Generated from AutoShip self-improvement report.

## Root Cause Evidence
${evidence:-No evidence provided.}

## Affected Files
${affected:-No affected files listed.}

## Candidate Acceptance Criteria
- ${criterion}
EOF
)"

    if bash "$SAFETY_FILTER" --text "$title" "type:feature,area:self-improvement" "$body" >/dev/null 2>&1; then
      gh issue create \
        --title "$title" \
        --body "$body" \
        --label "type:feature" \
        --label "area:self-improvement" \
        --label "priority:medium" \
        --label "atomic" \
        --label "agent:ready"
    else
      gh issue create \
        --title "$title" \
        --body "$body" \
        --label "type:feature" \
        --label "area:self-improvement" \
        --label "priority:medium" \
        --label "atomic" \
        --label "agent:blocked"
    fi
  done
