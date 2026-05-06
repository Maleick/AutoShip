#!/usr/bin/env bash
# monitor-issues.sh — Poll GitHub for new and closed issues.
# Dependency graph: lib/common.sh (optional), opencode/init.sh
# Leaf callers: opencode/init.sh
# Emits: [ISSUE_NEW] number=<N>, [ISSUE_CLOSED] number=<N>
# Poll interval: 60 seconds (external events; lowest urgency monitor).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Load shared utilities if available; inline fallback for standalone/test use.
if [[ -f "$SCRIPT_DIR/opencode/lib/common.sh" ]]; then
  source "$SCRIPT_DIR/opencode/lib/common.sh"
else
  autoship_repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || {
      echo "Error: not inside a git repository" >&2
      return 1
    }
  }
fi

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"

REPO_ROOT=$(autoship_repo_root) || exit 1
cd "$REPO_ROOT"

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh CLI not found" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: jq not found" >&2
  exit 1
fi

REPO_SLUG=$(jq -r '.repo // empty' "$STATE_FILE" 2>/dev/null) || REPO_SLUG=""
if [[ -z "$REPO_SLUG" ]]; then
  REPO_SLUG=$(git remote get-url origin 2>/dev/null | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
fi
# Validate repo slug format: must be owner/repo
if [[ -n "$REPO_SLUG" && ! "$REPO_SLUG" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
  echo "Error: invalid repo slug format: $REPO_SLUG" >&2
  exit 1
fi

# Capture initial mtime of AUTOSHIP.md so we can detect changes during polling.
# stat -f %m (macOS) or stat -c %Y (Linux) — try both for portability.
_autoship_md_mtime() {
  local path="AUTOSHIP.md"
  if [[ ! -f "$path" ]]; then
    echo "0"
    return
  fi
  stat -f %m "$path" 2>/dev/null \
    || stat -c %Y "$path" 2>/dev/null \
    || echo "0"
}

AUTOSHIP_MD_LAST_MTIME=$(_autoship_md_mtime)

WATERMARK_FILE="$AUTOSHIP_DIR/.issue-monitor-watermark"

if [[ -L "$AUTOSHIP_DIR" ]]; then
  echo "Error: $AUTOSHIP_DIR must not be a symlink" >&2
  exit 1
fi

if [[ -e "$WATERMARK_FILE" ]]; then
  if [[ -L "$WATERMARK_FILE" ]]; then
    echo "Error: $WATERMARK_FILE must not be a symlink" >&2
    exit 1
  fi
  if [[ ! -f "$WATERMARK_FILE" ]]; then
    echo "Error: $WATERMARK_FILE must be a regular file" >&2
    exit 1
  fi
fi

# Initialize watermark on first run (capture current time, emit nothing on first loop)
if [[ ! -f "$WATERMARK_FILE" ]]; then
  last_check=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  echo "$last_check" >"$WATERMARK_FILE"
else
  last_check=$(cat "$WATERMARK_FILE" 2>/dev/null) || last_check=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
fi

while true; do
  sleep 60
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  watermark_updated=false

  AUTOSHIP_MD_CURRENT_MTIME=$(_autoship_md_mtime)
  if [[ "$AUTOSHIP_MD_CURRENT_MTIME" != "$AUTOSHIP_MD_LAST_MTIME" ]]; then
    bash hooks/opencode/init.sh
    echo "AUTOSHIP.md changed — routing config reloaded" >>"$AUTOSHIP_DIR/poll.log"
    AUTOSHIP_MD_LAST_MTIME="$AUTOSHIP_MD_CURRENT_MTIME"
  fi

  new_issues=$(gh api \
    "repos/$REPO_SLUG/issues?state=open&since=${last_check}&sort=created&direction=asc" \
    --jq '.[].number' 2>/dev/null) || new_issues=""

  for num in $new_issues; do
    # Skip pull requests (GitHub issues API returns PRs too)
    is_pr=$(gh api "repos/$REPO_SLUG/issues/$num" --jq '.pull_request != null' 2>/dev/null) || is_pr="false"
    if [[ "$is_pr" == "false" ]]; then
      # Only emit if not already tracked in state (state uses prefixed key "issue-<N>")
      tracked=$(jq -r --arg n "issue-$num" '.issues[$n] // empty' "$STATE_FILE" 2>/dev/null)
      if [[ -z "$tracked" ]]; then
        echo "[ISSUE_NEW] number=$num"
      fi
    fi
  done

  closed_issues=$(gh api \
    "repos/$REPO_SLUG/issues?state=closed&since=${last_check}&sort=updated&direction=asc" \
    --jq '.[].number' 2>/dev/null) || closed_issues=""

  for num in $closed_issues; do
    is_pr=$(gh api "repos/$REPO_SLUG/issues/$num" --jq '.pull_request != null' 2>/dev/null) || is_pr="false"
    if [[ "$is_pr" == "false" ]]; then
      # Only emit if we were tracking this issue (state uses prefixed key "issue-<N>")
      tracked=$(jq -r --arg n "issue-$num" '.issues[$n] // empty' "$STATE_FILE" 2>/dev/null)
      if [[ -n "$tracked" ]]; then
        echo "[ISSUE_CLOSED] number=$num"
      fi
    fi
  done

  # Only write watermark if changed (avoid unnecessary disk I/O every 60s)
  if [[ "$last_check" != "$now" ]]; then
    last_check=$now
    echo "$last_check" >"$WATERMARK_FILE"
  fi
done
