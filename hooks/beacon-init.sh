#!/usr/bin/env bash
set -euo pipefail

# beacon-init.sh — Initialize .beacon/ directory structure and state file.
# Idempotent: safe to re-run without losing existing state.

BEACON_DIR=".beacon"
STATE_FILE="$BEACON_DIR/state.json"
WORKSPACES_DIR="$BEACON_DIR/workspaces"

# Check for jq dependency
if ! command -v jq &> /dev/null; then
  echo "Warning: jq not found. Install with: brew install jq (macOS) or apt install jq (Linux)"
  echo "Some beacon features will not work without jq."
fi

# Detect repo
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

# Derive owner/repo from git remote
REPO_SLUG=""
REMOTE_URL=$(git remote get-url origin 2>/dev/null) || true
if [[ -n "$REMOTE_URL" ]]; then
  # Handle both SSH and HTTPS remotes
  REPO_SLUG=$(echo "$REMOTE_URL" | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
fi

# Create directory structure
mkdir -p "$WORKSPACES_DIR"

# Initialize state.json only if it doesn't exist
if [[ ! -f "$STATE_FILE" ]]; then
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  cat > "$STATE_FILE" <<EOF
{
  "version": 1,
  "repo": "${REPO_SLUG}",
  "started_at": "${NOW}",
  "updated_at": "${NOW}",
  "plan": {
    "phases": [],
    "current_phase": 0,
    "checkpoint_pending": false
  },
  "issues": {},
  "tools": {
    "claude": { "status": "available", "quota_pct": 100 },
    "codex-spark": { "status": "available", "quota_pct": 100 },
    "codex-gpt": { "status": "available", "quota_pct": 100 },
    "gemini": { "status": "available", "quota_pct": 100 }
  },
  "stats": {
    "dispatched": 0,
    "completed": 0,
    "failed": 0,
    "blocked": 0
  }
}
EOF
  echo "Initialized $STATE_FILE"
else
  echo "$STATE_FILE already exists, skipping"
fi

# Create config.json for operator overrides (if not present)
CONFIG_FILE="$BEACON_DIR/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  echo '{}' > "$CONFIG_FILE"
  echo "Initialized $CONFIG_FILE (add test_command, etc. for overrides)"
fi

echo "Beacon workspace ready at $BEACON_DIR"
