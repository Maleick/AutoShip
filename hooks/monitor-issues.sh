#!/usr/bin/env bash
# monitor-issues.sh — Poll GitHub for new and closed issues.
# Emits: [ISSUE_NEW] number=<N>, [ISSUE_CLOSED] number=<N>
# Poll interval: 60 seconds (external events; lowest urgency monitor).

set -euo pipefail

BEACON_DIR=".beacon"
STATE_FILE="$BEACON_DIR/state.json"

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh CLI not found" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: jq not found" >&2
  exit 1
fi

# Get repo slug from state
REPO_SLUG=$(jq -r '.repo // empty' "$STATE_FILE" 2>/dev/null) || REPO_SLUG=""
if [[ -z "$REPO_SLUG" ]]; then
  REPO_SLUG=$(git remote get-url origin 2>/dev/null | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
fi

last_check=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

while true; do
  sleep 60
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  # New issues opened since last check
  new_issues=$(gh api \
    "repos/$REPO_SLUG/issues?state=open&since=${last_check}&sort=created&direction=asc" \
    --jq '.[].number' 2>/dev/null) || new_issues=""

  for num in $new_issues; do
    # Skip pull requests (GitHub issues API returns PRs too)
    is_pr=$(gh api "repos/$REPO_SLUG/issues/$num" --jq '.pull_request != null' 2>/dev/null) || is_pr="false"
    if [[ "$is_pr" == "false" ]]; then
      # Only emit if not already tracked in state
      tracked=$(jq -r --arg n "$num" '.issues[$n] // empty' "$STATE_FILE" 2>/dev/null)
      if [[ -z "$tracked" ]]; then
        echo "[ISSUE_NEW] number=$num"
      fi
    fi
  done

  # Issues closed since last check
  closed_issues=$(gh api \
    "repos/$REPO_SLUG/issues?state=closed&since=${last_check}&sort=updated&direction=asc" \
    --jq '.[].number' 2>/dev/null) || closed_issues=""

  for num in $closed_issues; do
    is_pr=$(gh api "repos/$REPO_SLUG/issues/$num" --jq '.pull_request != null' 2>/dev/null) || is_pr="false"
    if [[ "$is_pr" == "false" ]]; then
      # Only emit if we were tracking this issue
      tracked=$(jq -r --arg n "$num" '.issues[$n] // empty' "$STATE_FILE" 2>/dev/null)
      if [[ -n "$tracked" ]]; then
        echo "[ISSUE_CLOSED] number=$num"
      fi
    fi
  done

  last_check=$now
done
