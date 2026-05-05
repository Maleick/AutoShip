#!/usr/bin/env bash
set -euo pipefail

ISSUE=""
TITLE=""
LABELS=""

REPO_SLUG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --issue) ISSUE="$2"; shift 2 ;;
    --title) TITLE="$2"; shift 2 ;;
    --labels) LABELS="$2"; shift 2 ;;
    --repo) REPO_SLUG="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

[[ -n "$ISSUE" ]] || { echo "--issue is required" >&2; exit 2; }

# Build repo flag if specified
REPO_FLAG=""
if [[ -n "$REPO_SLUG" ]]; then
  REPO_FLAG="--repo $REPO_SLUG"
fi

if [[ -z "$TITLE" ]]; then
  TITLE=$(gh issue view "$ISSUE" $REPO_FLAG --json title --jq '.title' 2>/dev/null || echo "issue $ISSUE")
fi
if [[ -z "$LABELS" ]]; then
  LABELS=$(gh issue view "$ISSUE" $REPO_FLAG --json labels --jq '[.labels[].name] | join(",")' 2>/dev/null || echo "")
fi

labels_lower=$(printf '%s' "$LABELS" | tr '[:upper:]' '[:lower:]')
scope=""
if printf '%s' "$labels_lower" | grep -Eq 'combat'; then scope="combat"
elif printf '%s' "$labels_lower" | grep -Eq 'tui'; then scope="tui"
elif printf '%s' "$labels_lower" | grep -Eq 'web'; then scope="web"
fi
type="feat"

if printf '%s' "$labels_lower" | grep -Eq 'bug|security|p0-critical|p1-high'; then
  type="fix"
elif printf '%s' "$labels_lower" | grep -Eq 'documentation|docs'; then
  type="docs"
elif printf '%s' "$labels_lower" | grep -Eq 'test|testing'; then
  type="test"
elif printf '%s' "$labels_lower" | grep -Eq 'ci|ci-cd'; then
  type="ci"
elif printf '%s' "$labels_lower" | grep -Eq 'style'; then
  type="style"
elif printf '%s' "$labels_lower" | grep -Eq 'refactor|cleanup|polish'; then
  type="refactor"
elif printf '%s' "$labels_lower" | grep -Eq 'chore|infrastructure'; then
  type="chore"
fi

clean_title=$(printf '%s' "$TITLE" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//; s/^(feat|fix|docs|style|refactor|test|chore|ci)(\([^)]*\))?:[[:space:]]*//I; s/[[:space:]]*\(#[0-9]+\)$//')
if [[ -n "$scope" && "$type" != "docs" && "$type" != "ci" && "$type" != "test" && "$type" != "chore" ]]; then
  printf '%s(%s): %s (#%s)\n' "$type" "$scope" "$clean_title" "$ISSUE"
else
  printf '%s: %s (#%s)\n' "$type" "$clean_title" "$ISSUE"
fi
