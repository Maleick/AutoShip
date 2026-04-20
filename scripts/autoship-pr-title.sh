#!/usr/bin/env bash
# Generate a conventional commit PR title prefix for an AutoShip issue.
# Usage: autoship-pr-title.sh <issue-number>
# Output: "fix(scope): " or "feat(scope): " etc.
set -euo pipefail

ISSUE_NUM="${1:?usage: autoship-pr-title.sh <issue-number>}"

LABELS=$(gh issue view "$ISSUE_NUM" --json labels -q '[.labels[].name] | join(",")' 2>/dev/null || echo "")
TITLE=$(gh issue view "$ISSUE_NUM" --json title -q '.title' 2>/dev/null || echo "")

# Determine type from labels
if echo "$LABELS" | grep -qE "bug|p0-critical|p1-high|security"; then
  TYPE="fix"
elif echo "$LABELS" | grep -qE "documentation|docs"; then
  TYPE="docs"
elif echo "$LABELS" | grep -qE "ci|testing"; then
  TYPE="ci"
elif echo "$LABELS" | grep -qE "refactor|cleanup|polish"; then
  TYPE="refactor"
elif echo "$LABELS" | grep -qE "infrastructure|chore"; then
  TYPE="chore"
else
  TYPE="feat"
fi

# Determine scope from labels
if echo "$LABELS" | grep -qE "ui|tui"; then
  SCOPE="tui"
elif echo "$LABELS" | grep -qE "web|api"; then
  SCOPE="web"
elif echo "$LABELS" | grep -qE "dll|unsafe|windows"; then
  SCOPE="dll"
elif echo "$LABELS" | grep -qE "combat|rotation"; then
  SCOPE="combat"
elif echo "$LABELS" | grep -qE "soul"; then
  SCOPE="soul"
else
  SCOPE=""
fi

if [[ -n "$SCOPE" ]]; then
  echo "${TYPE}(${SCOPE}):"
else
  echo "${TYPE}:"
fi
