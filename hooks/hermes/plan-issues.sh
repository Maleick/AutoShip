#!/usr/bin/env bash
# Hermes agent plan-issues — fetch and filter issues for Hermes dispatch
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUTOSHIP_DIR="$REPO_ROOT/.autoship"

# Hermes labels — can be customized
LABELS="${HERMES_LABELS:-autoship:ready-simple}"
REPO="${HERMES_TARGET_REPO:-Maleick/TextQuest}"

echo "=== Hermes Issue Plan ==="
echo "Target repo: $REPO"
echo "Labels: $LABELS"
echo ""

# Fetch issues from GitHub
issues=$(curl -s "https://api.github.com/repos/$REPO/issues?labels=$LABELS&state=open&per_page=50&sort=created&direction=asc" -H "Authorization: token $(gh auth token)" 2>/dev/null || echo "[]")

count=$(echo "$issues" | jq 'length')
echo "Found $count issues"
echo ""

# List issues with size labels
echo "$issues" | jq -r '.[] | "\(.number): \(.title) [\((.labels // []) | map(.name) | map(select(test("size|agent"))) | if length == 0 then "no-size" else join(",") end)]"'

# Write plan to state
mkdir -p "$AUTOSHIP_DIR"
echo "$issues" | jq '{plan: [.[].number], count: length, timestamp: now}' > "$AUTOSHIP_DIR/hermes-plan.json"

echo ""
echo "Plan written to $AUTOSHIP_DIR/hermes-plan.json"
