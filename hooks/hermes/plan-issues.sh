#!/usr/bin/env bash
# Hermes agent plan-issues — fetch and filter issues for Hermes dispatch
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUTOSHIP_DIR="$REPO_ROOT/.autoship"

# Hermes labels — can be customized
# Default to atomic:ready (the actual label used in TextQuest)
LABELS="${HERMES_LABELS:-atomic:ready}"
# Resolve target repo: config.json → env → auto-detect → legacy fallback
REPO=""
if [[ -f "$AUTOSHIP_DIR/config.json" ]]; then
  REPO="$(jq -r '.repo // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)"
fi
if [[ -z "$REPO" && -n "${HERMES_TARGET_REPO:-}" ]]; then
  REPO="$HERMES_TARGET_REPO"
fi
if [[ -z "$REPO" ]]; then
  CURRENT_REMOTE="$(git remote get-url origin 2>/dev/null || true)"
  if [[ "$CURRENT_REMOTE" =~ github\.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
    REPO_NAME="${BASH_REMATCH[2]}"
    REPO_NAME="${REPO_NAME%.git}"
    REPO="${BASH_REMATCH[1]}/${REPO_NAME}"
  fi
fi
REPO="${REPO:-Maleick/TextQuest}"

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
echo "$issues" | jq '{plan: [.[].number], count: length, timestamp: now}' >"$AUTOSHIP_DIR/hermes-plan.json"

echo ""
echo "Plan written to $AUTOSHIP_DIR/hermes-plan.json"
