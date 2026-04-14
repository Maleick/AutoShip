#!/usr/bin/env bash
# setup.sh — First-run configuration: model selection, tool detection (Codex CLI), concurrency tuning
# Called by activate.sh if .autoship/.onboarded is missing

set -euo pipefail

REPO_ROOT="${1:-.}"
cd "$REPO_ROOT" || exit 1

# Ensure .autoship exists
mkdir -p .autoship

echo "=== AutoShip First-Run Setup ==="
echo ""
echo "This wizard will configure:"
echo "  1. Model selection (Lean/Balanced/Maxed)"
echo "  2. Tool detection (Codex CLI, Gemini CLI)"
echo "  3. Agent concurrency limits"
echo ""

# Detect jq
if ! command -v jq >/dev/null 2>&1; then
  echo "ERROR: jq required. Install: brew install jq"
  exit 1
fi

# Detect gh
if ! command -v gh >/dev/null 2>&1; then
  echo "ERROR: gh required. Install: brew install gh"
  exit 1
fi

# Read current config or defaults
SETTINGS="${HOME}/.claude/settings.json"
MODEL_CONFIG=$(jq -r '.env.AUTOSHIP_MODEL_CONFIG // "lean"' "$SETTINGS" 2>/dev/null || echo "lean")
MAX_AGENTS=$(jq -r '.env.AUTOSHIP_MAX_AGENTS // "10"' "$SETTINGS" 2>/dev/null || echo "10")

echo "Current config:"
echo "  Model config: $MODEL_CONFIG"
echo "  Max agents: $MAX_AGENTS"
echo ""

# Detect available tools
echo "Detecting available tools..."
TOOLS=$(bash "$REPO_ROOT/hooks/detect-tools.sh" 2>/dev/null || echo '{}')

CLAUDE_AVAILABLE=$(echo "$TOOLS" | jq -r '.["claude-haiku"].available // false' 2>/dev/null || echo "false")
CODEX_AVAILABLE=$(echo "$TOOLS" | jq -r '.["codex-spark"].available // false' 2>/dev/null || echo "false")
GEMINI_AVAILABLE=$(echo "$TOOLS" | jq -r '.["gemini"].available // false' 2>/dev/null || echo "false")

echo "Tools detected:"
echo "  Claude (Haiku/Opus):  $CLAUDE_AVAILABLE"
echo "  Codex CLI:            $CODEX_AVAILABLE"
echo "  Gemini CLI:           $GEMINI_AVAILABLE"
echo ""

if [[ "$CODEX_AVAILABLE" == "true" ]]; then
  CODEX_VER=$(echo "$TOOLS" | jq -r '.["codex-spark"].version // "unknown"' 2>/dev/null || echo "unknown")
  echo "  Codex version: $CODEX_VER"
fi

echo ""

# Mark as onboarded
DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ)
echo "$DATE" > .autoship/.onboarded
echo "✓ Setup complete. Saved to .autoship/.onboarded"
echo ""
echo "Configuration:"
echo "  Model config: $MODEL_CONFIG"
echo "  Max agents: $MAX_AGENTS"
echo ""
echo "To reconfigure, run: rm .autoship/.onboarded && /autoship:setup"
