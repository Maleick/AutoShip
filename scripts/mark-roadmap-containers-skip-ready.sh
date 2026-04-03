#!/usr/bin/env bash
set -euo pipefail

REPO="${1:-${GITHUB_REPOSITORY:-Maleick/DMFT}}"
READY_LABEL="agent:ready"
SKIP_LABEL="agent:skip-ready"
MILESTONE_PREFIXES=("M5" "M6" "M7" "M8")

if ! gh label list --repo "$REPO" --json name | jq -e 'map(.name) | index("agent:skip-ready")' >/dev/null; then
  gh label create --repo "$REPO" "$SKIP_LABEL" \
    --color "D93F0B" \
    --description "Roadmap container issues are intentionally excluded from agent-ready."
fi

echo "Applying agent:skip-ready to roadmap containers in ${REPO}: ${MILESTONE_PREFIXES[*]}"

TOTAL=0

for prefix in "${MILESTONE_PREFIXES[@]}"; do
  ISSUE_NUMBERS="$(gh issue list --repo "$REPO" --state open --json number,title,labels | jq -r --arg p "$prefix" '
    .[]
    | select(((.title // "") | startswith($p)))
    | .number
  ')"

  if [ -z "$ISSUE_NUMBERS" ]; then
    continue
  fi

  while IFS= read -r issue_number; do
    [ -z "$issue_number" ] && continue
    gh issue edit --repo "$REPO" "$issue_number" \
      --add-label "$SKIP_LABEL" \
      --remove-label "$READY_LABEL" \
      >/dev/null
    echo "Labeled issue #${issue_number} with agent:skip-ready"
    TOTAL=$((TOTAL + 1))
  done <<< "$ISSUE_NUMBERS"
done

echo "Updated ${TOTAL} issue(s)."

SKIP_COUNT=$TOTAL
TOTAL_READY=0
TO_LABEL=$(
  gh issue list --repo "$REPO" --state open --json number,labels | jq -r --arg ready "$READY_LABEL" --arg skip "$SKIP_LABEL" '
    .[]
    | select((.labels | map(.name) | index($skip) | not))
    | select((.labels | map(.name) | index($ready) | not))
    | .number
  '
)

while IFS= read -r issue_number; do
  [ -z "$issue_number" ] && continue
  gh issue edit --repo "$REPO" "$issue_number" --add-label "$READY_LABEL" >/dev/null
  echo "Labeled issue #${issue_number} with agent:ready"
  TOTAL_READY=$((TOTAL_READY + 1))
done <<< "$TO_LABEL"

echo "Queue repair added ${TOTAL_READY} issue(s) with agent:ready."
echo "Total roadmap containers skipped now: ${SKIP_COUNT}."
