#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) REPO_ROOT="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

AUTOSHIP_DIR="$REPO_ROOT/.autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"

[[ -f "$STATE_FILE" ]] || { echo "No state file at $STATE_FILE" >&2; exit 1; }
[[ -d "$WORKSPACES_DIR" ]] || exit 0

tmp=$(mktemp)
cp "$STATE_FILE" "$tmp"

for dir in "$WORKSPACES_DIR"/*/; do
  [[ -d "$dir" ]] || continue
  key=$(basename "$dir")
  status_file="$dir/status"
  [[ -f "$status_file" ]] || continue
  status=$(tr -d '[:space:]' < "$status_file")
  case "$status" in
    COMPLETE) new_state="verifying"; action="set-verifying" ;;
    BLOCKED) new_state="blocked"; action="set-blocked" ;;
    STUCK) new_state="stuck"; action="set-stuck" ;;
    RUNNING) new_state="running"; action="set-running" ;;
    QUEUED) new_state="queued"; action="set-queued" ;;
    *) continue ;;
  esac

  has_result=false
  result_file="$dir/AUTOSHIP_RESULT.md"
  started_file="$dir/started_at"
  if [[ -s "$result_file" ]]; then
    if [[ ! -f "$started_file" || "$result_file" -nt "$started_file" ]]; then
      has_result=true
    fi
  fi
  has_uncommitted=false
  if [[ -d "$dir/.git" || -f "$dir/.git" ]]; then
    if git -C "$dir" status --porcelain 2>/dev/null | grep -q .; then
      has_uncommitted=true
    fi
  fi

  current_state=$(jq -r --arg key "$key" '.issues[$key].state // empty' "$tmp" 2>/dev/null || true)
  increment_stats=true
  if [[ "$current_state" != "$new_state" && -x "$REPO_ROOT/hooks/update-state.sh" && -d "$REPO_ROOT/.git" ]]; then
    if (cd "$REPO_ROOT" && bash hooks/update-state.sh "$action" "$key") >/dev/null 2>&1; then
      cp "$STATE_FILE" "$tmp"
      increment_stats=false
    fi
  fi

  jq --arg key "$key" \
     --arg state "$new_state" \
     --arg status "$status" \
     --argjson has_result "$has_result" \
     --argjson has_uncommitted "$has_uncommitted" \
     --arg current "$current_state" \
     --argjson increment_stats "$increment_stats" \
     '.issues[$key] = ((.issues[$key] // {}) + {
       state: $state,
       workspace_status: $status,
       has_result: $has_result,
       has_uncommitted_changes: $has_uncommitted
     })
     | if $increment_stats and $current != $state and $state == "completed" then
         .stats.session_completed = ((.stats.session_completed // 0) + 1) |
         .stats.total_completed_all_time = ((.stats.total_completed_all_time // 0) + 1)
       elif $increment_stats and $current != $state and $state == "blocked" then
         .stats.blocked = ((.stats.blocked // 0) + 1)
       elif $increment_stats and $current != $state and $state == "stuck" then
         .stats.failed = ((.stats.failed // 0) + 1)
       else . end' "$tmp" > "$tmp.next"
  mv "$tmp.next" "$tmp"
done

mv "$tmp" "$STATE_FILE"
echo "State reconciled"
