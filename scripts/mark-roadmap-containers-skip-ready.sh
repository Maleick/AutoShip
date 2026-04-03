#!/usr/bin/env bash
set -euo pipefail

REPO="${1:-${GITHUB_REPOSITORY:-Maleick/DMFT}}"
READY_LABEL="agent:ready"
SKIP_LABEL="agent:skip-ready"

ALL_LABELS="$(gh label list --repo "$REPO" --limit 500 --json name)"

if ! jq -e --arg skip "$SKIP_LABEL" 'map(.name) | index($skip)' >/dev/null <<< "$ALL_LABELS"; then
  gh label create --repo "$REPO" "$SKIP_LABEL" \
    --color "D93F0B" \
    --description "Roadmap container issues are intentionally excluded from agent-ready."
fi

ALL_OPEN_ISSUES="$(gh issue list --repo "$REPO" --state open --limit 500 --json number,title,labels)"

echo "Applying agent:skip-ready to roadmap containers in ${REPO}"

TOTAL=0

ISSUE_ROWS="$(
  jq -c '
    .[]
    | select(((.title // "") | test("^M[0-9]+\\b")))
    | {
        number: .number,
        has_ready: ((.labels | map(.name) | index("agent:ready")) != null)
      }
  ' <<< "$ALL_OPEN_ISSUES"
)"

while IFS= read -r issue_row; do
  [ -z "$issue_row" ] && continue
  issue_number="$(jq -r '.number' <<< "$issue_row")"
  has_ready="$(jq -r '.has_ready' <<< "$issue_row")"

  if [ "$has_ready" = "true" ]; then
    gh issue edit --repo "$REPO" "$issue_number" \
      --add-label "$SKIP_LABEL" \
      --remove-label "$READY_LABEL" \
      >/dev/null
  else
    gh issue edit --repo "$REPO" "$issue_number" \
      --add-label "$SKIP_LABEL" \
      >/dev/null
  fi

  echo "Labeled issue #${issue_number} with agent:skip-ready"
  TOTAL=$((TOTAL + 1))
done <<< "$ISSUE_ROWS"

echo "Updated ${TOTAL} issue(s)."

SKIP_COUNT=$TOTAL
TOTAL_READY=0
TO_LABEL=$(
  jq -r --arg ready "$READY_LABEL" --arg skip "$SKIP_LABEL" '
    .[]
    | select((.labels | map(.name) | index($skip) | not))
    | select((.labels | map(.name) | index($ready) | not))
    | .number
  '
  <<< "$ALL_OPEN_ISSUES"
)

while IFS= read -r issue_number; do
  [ -z "$issue_number" ] && continue
  gh issue edit --repo "$REPO" "$issue_number" --add-label "$READY_LABEL" >/dev/null
  echo "Labeled issue #${issue_number} with agent:ready"
  TOTAL_READY=$((TOTAL_READY + 1))
done <<< "$TO_LABEL"

echo "Queue repair added ${TOTAL_READY} issue(s) with agent:ready."
echo "Total roadmap containers skipped now: ${SKIP_COUNT}."
