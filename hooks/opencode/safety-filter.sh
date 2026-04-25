#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 --text <title> <labels> <body> | <issue-number>" >&2
}

normalize_labels() {
  printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]'
}

check_text() {
  local title="$1"
  local labels="$2"
  local body="$3"
  local haystack
  haystack="$(printf '%s\n%s\n%s' "$title" "$(normalize_labels "$labels")" "$body" | tr '[:upper:]' '[:lower:]')"

  if printf '%s' "$haystack" | grep -Eq '(^|[^a-z0-9])unsafe([^a-z0-9]|$)|anti[- ]?cheat|stealth|fingerprint(ing)?|vm[ -]?detect|virtual machine|shellcode|polymorphic|detour hiding|hide[ -]?(detour|hook)|hook signature|signature evasion|evasion|bypass detection|undetectable|anti[- ]?debug'; then
    echo "BLOCKED: abuse-prone evasion or stealth content requires human review"
    return 1
  fi

  echo "SAFE"
}

if [[ "${1:-}" == "--text" ]]; then
  [[ $# -eq 4 ]] || { usage; exit 2; }
  check_text "$2" "$3" "$4"
  exit $?
fi

ISSUE_NUM="${1:-}"
[[ -n "$ISSUE_NUM" ]] || { usage; exit 2; }

TITLE=$(gh issue view "$ISSUE_NUM" --json title --jq '.title' 2>/dev/null || echo "")
BODY=$(gh issue view "$ISSUE_NUM" --json body --jq '.body' 2>/dev/null || echo "")
LABELS=$(gh issue view "$ISSUE_NUM" --json labels --jq '[.labels[].name] | join(",")' 2>/dev/null || echo "")

check_text "$TITLE" "$LABELS" "$BODY"
