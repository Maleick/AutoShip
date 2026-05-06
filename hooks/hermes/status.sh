#!/usr/bin/env bash
# Hermes agent status — show Hermes-specific runtime state
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUTOSHIP_DIR="$REPO_ROOT/.autoship"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"

echo "=== Hermes Runtime Status ==="

# Check Hermes availability
HERMES_CLI=false
HERMES_SESSION=false
if command -v hermes &>/dev/null; then
  HERMES_CLI=true
fi
if [[ -n "${HERMES_SESSION_ID:-}" || -n "${HERMES_CWD:-}" ]]; then
  HERMES_SESSION=true
fi

echo "Hermes CLI: $HERMES_CLI"
echo "Hermes Session: $HERMES_SESSION"
echo ""

# List Hermes workspaces
if [[ -d "$WORKSPACES_DIR" ]]; then
  echo "=== Hermes Workspaces ==="
  for ws in "$WORKSPACES_DIR"/issue-*; do
    if [[ -d "$ws" ]]; then
      issue_key=$(basename "$ws")
      status=$(cat "$ws/status" 2>/dev/null || echo "unknown")
      model=$(cat "$ws/model" 2>/dev/null || echo "unknown")
      role=$(cat "$ws/role" 2>/dev/null || echo "unknown")
      started=$(cat "$ws/started_at" 2>/dev/null || echo "unknown")

      # Check if it has a Hermes prompt
      if [[ -f "$ws/HERMES_PROMPT.md" ]]; then
        echo "$issue_key: status=$status model=$model role=$role started=$started"
      fi
    fi
  done
else
  echo "No workspaces found"
fi

echo ""
echo "=== Hermes Config ==="
if [[ -f "$AUTOSHIP_DIR/hermes-model-routing.json" ]]; then
  cat "$AUTOSHIP_DIR/hermes-model-routing.json"
else
  echo "No Hermes routing config. Run: bash hooks/hermes/setup.sh"
fi
