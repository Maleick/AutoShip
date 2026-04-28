#!/usr/bin/env bash

ISSUE_TITLE="${1:-}"
ISSUE_BODY="${2:-}"  
ISSUE_LABELS="${3:-}"
WORKTREE_PATH="${4:-$PWD}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

CARGO_TIMEOUT="$("$REPO_ROOT/hooks/opencode/policy.sh" value cargoTimeoutSeconds 2>/dev/null)" || CARGO_TIMEOUT="120"
CARGO_THRESHOLD="$("$REPO_ROOT/hooks/opencode/policy.sh" value cargoTargetIsolationThreshold 2>/dev/null)" || CARGO_THRESHOLD="8"
MAX_AGENTS="$(jq -r '.config.maxConcurrentAgents // .maxConcurrentAgents // .max_agents // 15' .autoship/state.json .autoship/config.json 2>/dev/null | head -1)"
TEXT="$(printf '%s\n%s\n%s\n' "$ISSUE_TITLE" "$ISSUE_BODY" "$ISSUE_LABELS")"

cat <<EOF

## AutoShip Burndown Policy
- You are running INSIDE git worktree: $WORKTREE_PATH. Do NOT cd anywhere else.
- NEVER write outside this worktree. Use relative paths from \$PWD.
- If local cargo verification is blocked by build locks for more than ${CARGO_TIMEOUT}s, stop cargo and record that local cargo was skipped in AUTOSHIP_RESULT.md.
EOF

if [[ "$MAX_AGENTS" =~ ^[0-9]+$ ]] && [[ "$CARGO_THRESHOLD" =~ ^[0-9]+$ ]] && [[ "$MAX_AGENTS" -gt "$CARGO_THRESHOLD" ]]; then
  printf '%s\n' "- Use CARGO_TARGET_DIR=$WORKTREE_PATH/target-isolated for cargo commands in this worktree."
fi

runner="$("$REPO_ROOT/hooks/opencode/policy.sh" value workflowRunnerDefault 2>/dev/null)" || runner=""
if [[ -n "$runner" ]] && [[ "$runner" != "null" ]]; then
  printf '%s\n' "- New GitHub Actions workflows must use runs-on: $runner. Do not use ubuntu-latest."
fi

pjson="$("$REPO_ROOT/hooks/opencode/policy.sh" json 2>/dev/null)" || pjson='{}'

for struct in $(echo "$pjson" | jq -r '.hotStructs | to_entries | .[].key'); do
  [[ -z "$struct" ]] && continue
  short="${struct##*::}"
  if echo "$TEXT" | grep -qiF "$short"; then
    echo "- Hot fixture registry for $struct:"
    jq -r --arg s "$struct" '.hotStructs[$s][]' <<< "$pjson" | sed 's/^/  - /'
  fi
done

for enum in $(echo "$pjson" | jq -r '.knownEnums[]'); do
  [[ -z "$enum" ]] && continue
  echo "$TEXT" | grep -qi "\b$enum\b" && echo "- Enum update warning: grep all match sites for $enum and update exhaustive arms."
done

for c in $(echo "$pjson" | jq -r '.overlapClusters[]'); do
  [[ -z "$c" ]] && continue
  nm="$(echo "$c" | jq -r '.name')" || continue
  kw="$(echo "$c" | jq -r '.keywords | join(",")')" || continue
  f="$(echo "$c" | jq -r '.files | join(", ")')" || continue
  for k in $(echo "$kw" | tr ',' '\n'); do
    [[ -z "$k" ]] && continue
    if echo "$TEXT" | grep -qi "\b${k}\b"; then
      echo "- File overlap cluster $nm: $f"
      break
    fi
  done
done