#!/usr/bin/env bash
# activate.sh — SessionStart hook
# Initializes .autoship/ state directory, then injects a brief system note
# so OpenCode knows AutoShip is installed and ready.

set -euo pipefail

VERSION_FILE="${CLAUDE_PLUGIN_ROOT:-$(dirname "$(dirname "$0")")}/VERSION"
VERSION="unknown"
if [[ -f "$VERSION_FILE" ]]; then
  VERSION="$(cat "$VERSION_FILE")"
fi

# Run init.sh silently if it exists
INIT_SCRIPT="${CLAUDE_PLUGIN_ROOT:-$(dirname "$(dirname "$0")")}/hooks/init.sh"
if [[ -f "$INIT_SCRIPT" ]]; then
  bash "$INIT_SCRIPT" >/dev/null 2>&1 || true
fi

# Inject a brief system note — this stdout becomes invisible system context
cat <<EOF
AutoShip ${VERSION} is installed. Autonomous multi-agent orchestration is available.
Commands: /autoship:start (launch), /autoship:plan (dry-run), /autoship:stop (shutdown), /autoship:status (dashboard).
AutoShip routes GitHub issues to OpenCode workers, verifies results, and opens PRs automatically.
Run /autoship:start to begin.
EOF
