#!/usr/bin/env bash
set -euo pipefail

FIX=false
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --fix) FIX=true; shift ;;
    --repo) REPO_ROOT="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

STATE_FILE="$REPO_ROOT/.autoship/state.json"
WORKSPACES_DIR="$REPO_ROOT/.autoship/workspaces"
drift=0

report() {
  drift=$((drift + 1))
  printf 'DRIFT: %s\n' "$1"
}

[[ -f "$STATE_FILE" ]] || { echo "No .autoship/state.json"; exit 0; }

seen=$(mktemp)
trap 'rm -f "$seen"' EXIT

if [[ -d "$WORKSPACES_DIR" ]]; then
  for dir in "$WORKSPACES_DIR"/issue-*; do
    [[ -d "$dir" ]] || continue
    issue_key=$(basename "$dir")
    issue_num="${issue_key#issue-}"
    if grep -qx "$issue_num" "$seen" 2>/dev/null; then
      report "duplicate workspace for issue #$issue_num"
    fi
    printf '%s\n' "$issue_num" >> "$seen"
    if ! jq -e --arg key "$issue_key" '.issues[$key] != null' "$STATE_FILE" >/dev/null 2>&1; then
      report "$issue_key workspace exists but state.json has no issue entry"
    fi
    if command -v gh >/dev/null 2>&1; then
      state=$(gh issue view "$issue_num" --json state --jq '.state' 2>/dev/null || echo missing)
      if [[ "$state" == "CLOSED" || "$state" == "missing" ]]; then
        report "$issue_key is in-flight locally but GitHub state is $state"
        if [[ "$FIX" == true ]]; then
          printf 'BLOCKED\n' > "$dir/status"
          bash "$REPO_ROOT/hooks/update-state.sh" set-blocked "$issue_key" reason="audit drift: GitHub state $state" >/dev/null 2>&1 || true
        fi
      fi
    fi
  done
fi

if [[ $drift -eq 0 ]]; then
  echo "AutoShip audit: no drift found"
  exit 0
fi

echo "AutoShip audit: $drift drift item(s) found"
exit 1
