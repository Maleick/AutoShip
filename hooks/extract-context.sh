#!/usr/bin/env bash
set -euo pipefail

# extract-context.sh — Extract project-specific conventions and config for agents.
# Output: .autoship/project-context.md (capped at 1500 chars to keep worker context focused)

AUTOSHIP_DIR=".autoship"
CONTEXT_FILE="$AUTOSHIP_DIR/project-context.md"
CONFIG_FILE="$AUTOSHIP_DIR/config.json"
TEMP_FILE=$(mktemp)
trap 'rm -f "$TEMP_FILE"' EXIT
REPO_ROOT=$(pwd -P)

is_safe_repo_file() {
  local file_path="$1"
  local resolved_path

  [[ -f "$file_path" ]] || return 1
  [[ ! -L "$file_path" ]] || return 1

  resolved_path=$(readlink -f -- "$file_path" 2>/dev/null) || return 1
  [[ -n "$resolved_path" ]] || return 1
  [[ "$resolved_path" == "$REPO_ROOT/"* ]] || return 1
  [[ -f "$resolved_path" ]] || return 1
  [[ ! -L "$resolved_path" ]] || return 1
}

{
  echo "## Project Context"
  echo ""

  # 1. Read from .autoship/config.json
  if is_safe_repo_file "$CONFIG_FILE"; then
    echo "### Project Configuration"
    jq -r 'to_entries | map("- **\(.key)**: \(.value)") | .[]' "$CONFIG_FILE" 2>/dev/null | head -10 || true
    echo ""
  fi

  # 2. Extract from CLAUDE.md - only critical sections, strict line limit
  if is_safe_repo_file "CLAUDE.md"; then
    echo "### Key Conventions"
    # Extract ONLY lines under Patterns, Conventions, or Gotchas headings
    # Stop at first code block to keep it concise
    awk '
      BEGIN { printing = 0; count = 0; }
      tolower($0) ~ /^#+[[:space:]]*(patterns|conventions|gotchas|critical|invariants)/ {
        printing = 1;
        count = 0;
        print $0;
        next;
      }
      /^#+/ {
        if (printing) printing = 0;
      }
      /^```/ {
        if (printing) printing = 0;
      }
      printing {
        if (count < 15) {
          print $0;
          count++;
        } else {
          printing = 0;
        }
      }
    ' "CLAUDE.md"
    echo ""
  fi

  # 3. Read AGENTS.md if present - keep SHORT for worker context
  if is_safe_repo_file "AGENTS.md"; then
    echo "### Agent Constraints"
    head -15 "AGENTS.md"
    echo ""
  fi
} > "$TEMP_FILE"

# Capping at 1500 chars (~250 tokens proxy) keeps OpenCode worker prompts focused
head -c 1500 "$TEMP_FILE" > "$CONTEXT_FILE"

echo "Project context extracted to $CONTEXT_FILE ($(wc -c < "$CONTEXT_FILE") chars)"
