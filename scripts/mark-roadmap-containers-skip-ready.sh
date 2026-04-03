#!/usr/bin/env bash
set -euo pipefail

if [ -n "${1:-}" ]; then
  REPO="$1"
elif [ -n "${GITHUB_REPOSITORY:-}" ]; then
  REPO="$GITHUB_REPOSITORY"
else
  echo "Error: repository must be provided as the first argument or via GITHUB_REPOSITORY." >&2
  exit 1
fi

READY_LABEL="agent:ready"
SKIP_LABEL="agent:skip-ready"

ALL_LABELS="$(gh label list --repo "$REPO" --limit 500 --json name)"

if ! jq -e --arg ready "$READY_LABEL" 'map(.name) | index($ready)' >/dev/null <<< "$ALL_LABELS"; then
  gh label create --repo "$REPO" "$READY_LABEL" \
    --color "0E8A16" \
    --description "Issue is eligible for autonomous pickup."
fi

if ! jq -e --arg skip "$SKIP_LABEL" 'map(.name) | index($skip)' >/dev/null <<< "$ALL_LABELS"; then
  gh label create --repo "$REPO" "$SKIP_LABEL" \
    --color "D93F0B" \
    --description "Roadmap container issues are intentionally excluded from agent-ready."
fi

ALL_OPEN_ISSUES="$(gh issue list --repo "$REPO" --state open --limit 1000 --json number,title,labels)"

echo "Applying agent:skip-ready to roadmap containers in ${REPO}"

TOTAL=0
ISSUE_ROWS="$(
  jq -c '
    .[]
    | select(((.title // "") | test("^M([0-9]+|x)\\b"; "i")))
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
  edit_args=(--repo "$REPO" "$issue_number" --add-label "$SKIP_LABEL")

  if [ "$has_ready" = "true" ]; then
    edit_args+=(--remove-label "$READY_LABEL")
  fi

  gh issue edit "${edit_args[@]}" >/dev/null
  echo "Labeled issue #${issue_number} with agent:skip-ready"
  TOTAL=$((TOTAL + 1))
done <<< "$ISSUE_ROWS"

echo "Updated ${TOTAL} issue(s)."

SKIP_COUNT=$TOTAL
TOTAL_READY=0
TO_LABEL=$(
  jq -r --arg ready "$READY_LABEL" --arg skip "$SKIP_LABEL" '
    .[]
    | select(((.title // "") | test("^M([0-9]+|x)\\b"; "i") | not))
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

SKIP_COUNT="$(jq -r '
  [
    .[]
    | select(((.title // "") | test("^M([0-9]+|x)\\b"; "i")))
  ]
  | length
' <<< "$ALL_OPEN_ISSUES")"

echo "Queue repair added ${TOTAL_READY} issue(s) with agent:ready."
echo "Total roadmap containers skipped now: ${SKIP_COUNT}."
