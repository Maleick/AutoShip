#!/usr/bin/env bash
set -euo pipefail

ISSUES_FILE=""
CREATED_ISSUES_FILE=false
LIMIT=""
PLAN_ORDER="${AUTOSHIP_PLAN_ORDER:-ascending}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --issues-file)
      ISSUES_FILE="$2"
      shift 2
      ;;
    --limit)
      LIMIT="$2"
      shift 2
      ;;
    --order)
      PLAN_ORDER="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$ISSUES_FILE" ]]; then
  ISSUES_FILE=$(mktemp)
  CREATED_ISSUES_FILE=true
  gh issue list --state open --json number,title,body,labels,updatedAt --limit 200 >"$ISSUES_FILE"
fi

limit_filter='.'
if [[ -n "$LIMIT" ]]; then
  limit_filter=".[0:${LIMIT}]"
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
POLICY_JSON='{}'
if [[ -x "$SCRIPT_DIR/policy.sh" ]]; then
  POLICY_JSON=$(cd "$REPO_ROOT" && bash "$SCRIPT_DIR/policy.sh" json 2>/dev/null || printf '{}')
fi

eligible_tmp=$(mktemp)
blocked_tmp=$(mktemp)
cleanup() {
  rm -f "$eligible_tmp" "$blocked_tmp"
  if [[ "$CREATED_ISSUES_FILE" == true ]]; then
    rm -f "$ISSUES_FILE"
  fi
}
trap cleanup EXIT

jq -c '.[]' "$ISSUES_FILE" | while IFS= read -r issue; do
  if ! jq -e 'any(.labels[].name; . == "agent:ready")' <<<"$issue" >/dev/null; then
    continue
  fi
  if jq -e 'any(.labels[].name; . == "agent:running" or . == "agent:blocked" or . == "human:required")' <<<"$issue" >/dev/null; then
    continue
  fi

  issue_num=$(jq -r '.number' <<<"$issue")
  if command -v gh >/dev/null 2>&1 && gh pr list --state open --search "$issue_num in:title" --json number --jq 'length' 2>/dev/null | grep -qx '[1-9][0-9]*'; then
    jq -c --arg reason "open PR already exists" '. + {blocked_reason: $reason}' <<<"$issue" >>"$blocked_tmp"
    continue
  fi

  jq -c '.' <<<"$issue" >>"$eligible_tmp"
done

case "$PLAN_ORDER" in
  cadence | updated | recent)
    sort_filter='sort_by(.updatedAt // "") | reverse'
    ;;
  descending)
    sort_filter='sort_by(.number) | reverse'
    ;;
  *)
    sort_filter='sort_by(.number)'
    ;;
esac

eligible_json=$(jq -s "$sort_filter | $limit_filter" "$eligible_tmp")
blocked_json=$(jq -s 'sort_by(.number)' "$blocked_tmp")

jq -n --argjson eligible "$eligible_json" --argjson blocked "$blocked_json" '{eligible: $eligible, blocked: $blocked}'
