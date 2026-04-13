#!/usr/bin/env bash
# beacon-activate.sh — SessionStart hook
# Initializes .beacon/ state directory, then injects a brief system note
# so Claude knows Beacon is installed and ready.

set -euo pipefail

VERSION_FILE="${CLAUDE_PLUGIN_ROOT:-$(dirname "$(dirname "$0")")}/VERSION"
VERSION="unknown"
if [[ -f "$VERSION_FILE" ]]; then
  VERSION="$(cat "$VERSION_FILE")"
fi

# Run beacon-init.sh silently if it exists
INIT_SCRIPT="${CLAUDE_PLUGIN_ROOT:-$(dirname "$(dirname "$0")")}/hooks/beacon-init.sh"
if [[ -f "$INIT_SCRIPT" ]]; then
  bash "$INIT_SCRIPT" >/dev/null 2>&1 || true
fi

# Inject a brief system note — this stdout becomes invisible system context
cat <<EOF
AutoShip ${VERSION} is installed. Autonomous multi-agent orchestration is available.
Commands: /autoship:start (launch), /beacon:plan (dry-run), /beacon:stop (shutdown), /beacon:status (dashboard).
AutoShip routes GitHub issues to AI agents (Codex/Gemini/Claude), verifies results, and opens PRs automatically.
Run /autoship:start to begin.
EOF
