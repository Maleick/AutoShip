# classify-issue-opencode.sh — Classify GitHub issue complexity for OpenCode

set -euo pipefail

ISSUE_NUM="${1:-}"
[[ -z "$ISSUE_NUM" ]] && echo "Usage: $0 <issue-number>" >&2 && exit 1

ISSUE_BODY=$(gh issue view "$ISSUE_NUM" --json body --jq '.body' 2>/dev/null || echo "")
ISSUE_TITLE=$(gh issue view "$ISSUE_NUM" --json title --jq '.title' 2>/dev/null || echo "")
ISSUE_LABELS=$(gh issue view "$ISSUE_NUM" --json labels --jq '[.labels[].name]' 2>/dev/null || echo "[]")
LABEL_TEXT=$(echo "$ISSUE_LABELS" | jq -r 'join(",")' 2>/dev/null || echo "")

if bash "$(dirname "$0")/safety-filter.sh" --text "$ISSUE_TITLE" "$LABEL_TEXT" "$ISSUE_BODY" 2>/dev/null | grep -q '^BLOCKED:'; then
  echo "blocked"
  exit 0
fi

# Check for explicit label overrides
if echo "$ISSUE_LABELS" | jq -e '.[] | test("mode:(research|docs|complex|simple)"; "i")' >/dev/null 2>&1; then
  MODE=$(echo "$ISSUE_LABELS" | jq -r '.[] | select(test("mode:(research|docs|complex|simple)"; "i")) | capture("mode:(?<mode>.*)").mode' | head -1)
  case "$MODE" in
    research) echo "research" && exit 0 ;;
    docs) echo "docs" && exit 0 ;;
    complex) echo "complex" && exit 0 ;;
    simple) echo "simple_code" && exit 0 ;;
  esac
fi

if echo "$ISSUE_LABELS" | jq -e '.[] | test("documentation|docs"; "i")' >/dev/null 2>&1; then
  echo "docs"
  exit 0
fi

if echo "$ISSUE_LABELS" | jq -e '.[] | test("size-s|small|easy"; "i")' >/dev/null 2>&1; then
  echo "simple_code"
  exit 0
fi

if echo "$ISSUE_LABELS" | jq -e '.[] | test("size-m|medium"; "i")' >/dev/null 2>&1; then
  echo "medium_code"
  exit 0
fi

if echo "$ISSUE_LABELS" | jq -e '.[] | test("security|core|state-machine|architecture"; "i")' >/dev/null 2>&1; then
  echo "complex"
  exit 0
fi

# Check for rust_unsafe keyword
if echo "$ISSUE_BODY $ISSUE_TITLE" | grep -qiE "(unsafe|rust.*memory|DLL|cdylib)"; then
  echo "rust_unsafe"
  exit 0
fi

# Check for CI/lint/test failures
if echo "$ISSUE_BODY $ISSUE_TITLE" | grep -qiE "(ci.*fail|lint.*error|test.*fail|format.*error)"; then
  echo "ci_fix"
  exit 0
fi

# Check title keywords
TITLE_LOWER=$(echo "$ISSUE_TITLE" | tr '[:upper:]' '[:lower:]')

case "$TITLE_LOWER" in
  *research*) echo "research"; exit 0 ;;
  *docs*|*readme*|*documentation*) echo "docs"; exit 0 ;;
  *refactor*|*cleanup*|*cleanup*) echo "mechanical"; exit 0 ;;
  *fix*|*bug*|*patch*) echo "simple_code"; exit 0 ;;
  *feature*|*implement*|*add*) echo "medium_code"; exit 0 ;;
  *architecture*|*redesign*|*migration*) echo "complex"; exit 0 ;;
esac

# Default classification based on body analysis
BODY_LENGTH=${#ISSUE_BODY}
if (( BODY_LENGTH < 200 )); then
  echo "simple_code"
elif (( BODY_LENGTH < 800 )); then
  echo "medium_code"
else
  echo "complex"
fi
