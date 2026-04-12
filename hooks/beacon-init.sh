#!/usr/bin/env bash
set -euo pipefail

# beacon-init.sh — Initialize .beacon/ directory structure and state file.
# Idempotent: safe to re-run without losing existing state.

BEACON_DIR=".beacon"
STATE_FILE="$BEACON_DIR/state.json"
WORKSPACES_DIR="$BEACON_DIR/workspaces"

# Resolve the directory this script lives in so we can call sibling scripts.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Detect repo
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

# Check for jq dependency
if ! command -v jq >/dev/null 2>&1; then
  echo "Warning: jq not found. Install with: brew install jq" >&2
fi

# Derive owner/repo from git remote
REPO_SLUG=""
REMOTE_URL=$(git remote get-url origin 2>/dev/null) || true
if [[ -n "$REMOTE_URL" ]]; then
  # Handle both SSH and HTTPS remotes
  REPO_SLUG=$(echo "$REMOTE_URL" | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
fi

# Create directory structure
mkdir -p "$WORKSPACES_DIR"

# Detect tools and parse quotas.
# detect-tools.sh outputs: {"claude": {"available": bool, "quota_pct": N}, ...}
# Transform to state.json format:  {"claude": {"status": "available"|"unavailable", "quota_pct": N}, ...}
TOOLS_RAW=$(bash "$SCRIPT_DIR/detect-tools.sh" 2>/dev/null) || TOOLS_RAW='{}'
TOOLS_JSON=$(printf '%s' "$TOOLS_RAW" | jq '
  to_entries
  | map({
      key: .key,
      value: {
        status: (if .value.available then "available" else "unavailable" end),
        quota_pct: .value.quota_pct
      }
    })
  | from_entries
' 2>/dev/null) || TOOLS_JSON='{
    "claude":     {"status": "available",   "quota_pct": 100},
    "codex-spark":{"status": "unavailable", "quota_pct": -1},
    "codex-gpt":  {"status": "unavailable", "quota_pct": -1},
    "gemini":     {"status": "unavailable", "quota_pct": -1}
  }'

# Initialize state.json only if it doesn't exist
if [[ ! -f "$STATE_FILE" ]]; then
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  # Use jq to produce well-formed JSON, embedding the detected tools.
  jq -n \
    --arg repo "$REPO_SLUG" \
    --arg now "$NOW" \
    --argjson tools "$TOOLS_JSON" \
    '{
      version: 1,
      repo: $repo,
      started_at: $now,
      updated_at: $now,
      plan: {
        phases: [],
        current_phase: 0,
        checkpoint_pending: false
      },
      issues: {},
      tools: $tools,
      stats: {
        dispatched: 0,
        completed: 0,
        failed: 0,
        blocked: 0
      }
    }' > "$STATE_FILE"
  echo "Initialized $STATE_FILE"
else
  # Refresh tools section with current quota data without touching other state.
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  UPDATED=$(jq --argjson tools "$TOOLS_JSON" --arg now "$NOW" \
    '.tools = $tools | .updated_at = $now' "$STATE_FILE") && \
    printf '%s\n' "$UPDATED" > "$STATE_FILE"
  echo "Refreshed tools quota in $STATE_FILE"
fi

# Create config.json for operator overrides (if not present)
CONFIG_FILE="$BEACON_DIR/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  echo '{}' > "$CONFIG_FILE"
  echo "Initialized $CONFIG_FILE (add test_command, etc. for overrides)"
fi

# Create GitHub labels if the repo is on GitHub and gh CLI is available
create_github_labels() {
  # Check if gh CLI is available
  if ! command -v gh &> /dev/null; then
    return 0
  fi

  # Check if we're in a GitHub repo (REPO_SLUG will be set)
  if [[ -z "$REPO_SLUG" ]]; then
    return 0
  fi

  # Create each label if it doesn't exist (bash 3.2 compatible — no associative arrays)
  local pairs="beacon:in-progress=FFEB3B beacon:blocked=F44336 beacon:paused=FF9800 beacon:done=4CAF50"
  for pair in $pairs; do
    label="${pair%%=*}"
    color="${pair#*=}"
    if ! gh label list --repo "$REPO_SLUG" --json name --jq ".[].name" 2>/dev/null | grep -q "^${label}$"; then
      echo "Creating label: $label (color: $color)"
      gh label create "$label" --repo "$REPO_SLUG" --color "$color" --description "Beacon orchestration label" 2>/dev/null || true
    fi
  done
}

# Attempt to create labels (non-fatal if it fails)
create_github_labels || true

# Sweep stale worktrees on startup (non-fatal if it fails)
echo "Scanning for stale worktrees..."
bash "$SCRIPT_DIR/sweep-stale.sh" 2>/dev/null || true

echo "Beacon workspace ready at $BEACON_DIR"
