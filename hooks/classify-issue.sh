#!/usr/bin/env bash
# classify-issue.sh — Classify a GitHub issue into a task type.
# Usage: classify-issue.sh <issue-number>
# Outputs: task_type string to stdout
# Also writes task_type to .beacon/state.json for that issue.
#
# Task types: research | docs | simple_code | medium_code | complex | mechanical | ci_fix

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <issue-number>" >&2
  echo "Outputs task_type to stdout and writes it to .beacon/state.json" >&2
  exit 1
fi

ISSUE_NUMBER="$1"
ISSUE_ID="issue-${ISSUE_NUMBER}"

# Locate repo root
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}

STATE_FILE="$REPO_ROOT/.beacon/state.json"

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: jq is required but not found" >&2
  exit 1
fi

# Fetch issue data from GitHub
if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh CLI is required but not found" >&2
  exit 1
fi

ISSUE_JSON=$(gh issue view "$ISSUE_NUMBER" --json title,body,labels 2>/dev/null) || {
  echo "Error: could not fetch issue #${ISSUE_NUMBER} from GitHub" >&2
  exit 1
}

TITLE=$(echo "$ISSUE_JSON" | jq -r '.title // ""')
BODY=$(echo "$ISSUE_JSON" | jq -r '.body // ""')
LABELS=$(echo "$ISSUE_JSON" | jq -r '[.labels[].name] | join(",")' 2>/dev/null || echo "")

# Read complexity from state.json if present
COMPLEXITY=""
if [[ -f "$STATE_FILE" ]]; then
  COMPLEXITY=$(jq -r --arg id "$ISSUE_ID" '.issues[$id].complexity // ""' "$STATE_FILE" 2>/dev/null || echo "")
fi

# Normalize inputs to lowercase for case-insensitive matching
TITLE_LOWER=$(echo "$TITLE" | tr '[:upper:]' '[:lower:]')
BODY_LOWER=$(echo "$BODY" | tr '[:upper:]' '[:lower:]')
LABELS_LOWER=$(echo "$LABELS" | tr '[:upper:]' '[:lower:]')

# Classification logic — priority order:
TASK_TYPE=""

# Rule 1: Label override (highest priority)
if echo "$LABELS_LOWER" | grep -qw "mode:research"; then
  TASK_TYPE="research"
elif echo "$LABELS_LOWER" | grep -qw "mode:docs"; then
  TASK_TYPE="docs"
elif echo "$LABELS_LOWER" | grep -qw "mode:complex"; then
  TASK_TYPE="complex"
fi

# Rule 2: Existing complexity field in state.json
if [[ -z "$TASK_TYPE" && -n "$COMPLEXITY" ]]; then
  case "$COMPLEXITY" in
    simple)  TASK_TYPE="simple_code" ;;
    medium)  TASK_TYPE="medium_code" ;;
    complex) TASK_TYPE="complex" ;;
  esac
fi

# Rule 3: Body heuristics — CI/lint/format/test failure keywords
if [[ -z "$TASK_TYPE" ]]; then
  if echo "$BODY_LOWER" | grep -qiE '\bci\b|lint|format|test failure'; then
    TASK_TYPE="ci_fix"
  fi
fi

# Rule 4: Title keywords
if [[ -z "$TASK_TYPE" ]]; then
  if echo "$TITLE_LOWER" | grep -qiE '\bresearch\b|\binvestigate\b|\baudit\b|\banalyze\b'; then
    TASK_TYPE="research"
  elif echo "$TITLE_LOWER" | grep -qiE '\bdoc\b|\bdocs\b|\breadme\b|\bchangelog\b'; then
    TASK_TYPE="docs"
  elif echo "$TITLE_LOWER" | grep -qiE '\brefactor\b|\bcleanup\b|\bclean.?up\b|\brename\b'; then
    TASK_TYPE="mechanical"
  fi
fi

# Rule 5: Default
if [[ -z "$TASK_TYPE" ]]; then
  TASK_TYPE="medium_code"
fi

# Write task_type to state.json if state file exists
if [[ -f "$STATE_FILE" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  UPDATE_STATE="$SCRIPT_DIR/update-state.sh"
  if [[ -f "$UPDATE_STATE" ]]; then
    bash "$UPDATE_STATE" set-running "$ISSUE_ID" "task_type=${TASK_TYPE}" 2>/dev/null || {
      # If set-running fails (issue may not exist yet), try a direct jq write
      LOCK_FILE="${STATE_FILE%.json}.lock"
      TMP=$(mktemp)
      if command -v flock >/dev/null 2>&1; then
        flock "$LOCK_FILE" jq --arg id "$ISSUE_ID" --arg tt "$TASK_TYPE" \
          'if .issues[$id] then .issues[$id].task_type = $tt else . end' \
          "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
      else
        lockf -k "$LOCK_FILE" jq --arg id "$ISSUE_ID" --arg tt "$TASK_TYPE" \
          'if .issues[$id] then .issues[$id].task_type = $tt else . end' \
          "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE" 2>/dev/null || true
      fi
      rm -f "$TMP"
    }
  fi
fi

# Output task_type to stdout
echo "$TASK_TYPE"
