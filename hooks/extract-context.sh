#!/usr/bin/env bash
set -euo pipefail

# extract-context.sh — Extract project-specific conventions and config for agents.
# Output: .autoship/project-context.md (capped at ~3000 chars)

AUTOSHIP_DIR=".autoship"
CONTEXT_FILE="$AUTOSHIP_DIR/project-context.md"
CONFIG_FILE="$AUTOSHIP_DIR/config.json"
TEMP_FILE=$(mktemp)
trap 'rm -f "$TEMP_FILE"' EXIT

{
  echo "## Project Context"
  echo ""

  # 1. Read from .autoship/config.json
  if [[ -f "$CONFIG_FILE" ]]; then
    echo "### Project Configuration"
    jq -r 'to_entries | map("- **\(.key)**: \(.value)") | .[]' "$CONFIG_FILE" 2>/dev/null || true
    echo ""
  fi

  # 2. Extract from CLAUDE.md
  if [[ -f "CLAUDE.md" ]]; then
    echo "### Project Conventions (from CLAUDE.md)"
    # Extract sections whose heading topic is Patterns, Conventions, or Gotchas.
    awk '
      BEGIN { printing = 0; count = 0; }
      tolower($0) ~ /^#+[[:space:]]*(patterns|conventions|gotchas)/ {
        printing = 1;
        count = 0;
        print $0;
        next;
      }
      /^#+/ {
        if (printing) printing = 0;
      }
      printing {
        if (count < 40) {
          print $0;
          count++;
        } else {
          printing = 0;
        }
      }
    ' "CLAUDE.md"
    echo ""
  fi

  # 3. Read AGENTS.md if present
  if [[ -f "AGENTS.md" ]]; then
    echo "### Agent Constraints (from AGENTS.md)"
    cat "AGENTS.md"
    echo ""
  fi
} > "$TEMP_FILE"

# Capping at 3000 chars (~500 tokens proxy)
head -c 3000 "$TEMP_FILE" > "$CONTEXT_FILE"

echo "Project context extracted to $CONTEXT_FILE ($(wc -c < "$CONTEXT_FILE") chars)"
