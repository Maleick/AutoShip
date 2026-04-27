#!/usr/bin/env bash
set -euo pipefail

ISSUES_FILE=""
CREATED_ISSUES_FILE=false
LIMIT=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --issues-file) ISSUES_FILE="$2"; shift 2 ;;
    --limit) LIMIT="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ -z "$ISSUES_FILE" ]]; then
  ISSUES_FILE=$(mktemp)
  CREATED_ISSUES_FILE=true
  gh issue list --state open --json number,title,body,labels --limit 200 > "$ISSUES_FILE"
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
  if ! jq -e 'any(.labels[].name; . == "agent:ready")' <<< "$issue" >/dev/null; then
    continue
  fi
  if jq -e 'any(.labels[].name; . == "agent:running" or . == "agent:blocked" or . == "human:required")' <<< "$issue" >/dev/null; then
    continue
  fi

  jq -c --argjson policy "$POLICY_JSON" '
    . as $issue
    | def literal_paths: [($issue | .. | strings | match("[A-Za-z0-9_.-]+(/[A-Za-z0-9_.-]+)+";"g").string)];
    def cluster_matches:
      [($policy.overlapClusters // [])[] | select(any(.keywords[]; . as $kw | $issue | .. | strings | test($kw; "i")))];
    $issue + {
      probable_files: ((literal_paths + (cluster_matches | map(.files[]) )) | unique),
      overlap_cluster: ((cluster_matches[0].name // null))
    }
  ' <<< "$issue" >> "$eligible_tmp"
done

eligible_json=$(jq -s "sort_by(.number) | $limit_filter" "$eligible_tmp")
blocked_json=$(jq -s 'sort_by(.number)' "$blocked_tmp")

jq -n --argjson eligible "$eligible_json" --argjson blocked "$blocked_json" '{eligible: $eligible, blocked: $blocked}'
