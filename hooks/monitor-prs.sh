#!/usr/bin/env bash
# monitor-prs.sh — Watch AutoShip PRs for CI status, conflicts, and merges.
# Emits: [PR_CI_PASS], [PR_CI_FAIL], [PR_CONFLICT], [PR_MERGED]
# Poll interval: 30 seconds (CI takes minutes; faster polling wastes API quota).

set -euo pipefail

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
SEEN_FILE="$AUTOSHIP_DIR/.pr-monitor-seen.json"

# Temporary file for atomic seen-state updates — cleaned up on exit
_SEEN_TMP=$(mktemp)
trap 'rm -f "$_SEEN_TMP"' EXIT

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh CLI not found" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: jq not found" >&2
  exit 1
fi

REPO_SLUG=$(jq -r '.repo // empty' "$STATE_FILE" 2>/dev/null) || REPO_SLUG=""
if [[ -z "$REPO_SLUG" ]]; then
  REPO_SLUG=$(git remote get-url origin 2>/dev/null | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
fi

if [[ ! -f "$SEEN_FILE" ]]; then
  echo "Error: $SEEN_FILE not found. Run init.sh first." >&2
  exit 1
fi

emit_if_changed() {
  local pr_number="$1"
  local event="$2"
  local key="${pr_number}:${event}"

  # Only emit if we haven't emitted this event for this PR recently
  seen=$(jq -r --arg k "$key" '.[$k] // empty' "$SEEN_FILE")
  if [[ -z "$seen" ]]; then
    echo "$event number=$pr_number"
    jq --arg k "$key" --arg now "$(date -u +%s)" '.[$k] = $now' "$SEEN_FILE" > "$_SEEN_TMP" && \
      mv "$_SEEN_TMP" "$SEEN_FILE"
  fi
}

reset_pr_seen() {
  local pr_number="$1"
  jq --arg n "$pr_number" 'del(.[$n + ":PR_CI_PASS"]) | del(.[$n + ":PR_CI_FAIL"])' \
    "$SEEN_FILE" > "$_SEEN_TMP" && mv "$_SEEN_TMP" "$SEEN_FILE"
}

while true; do
  # Check open AutoShip PRs for CI status and conflicts
  gh pr list --label autoship --state open \
    --json number,mergeable,statusCheckRollup \
    --repo "$REPO_SLUG" 2>/dev/null | \
    jq -c '.[] | {number: .number, mergeable: .mergeable, checks: (.statusCheckRollup // [])}' | \
    while IFS= read -r pr_json; do
      num=$(jq -r '.number' <<< "$pr_json")
      mergeable=$(jq -r '.mergeable' <<< "$pr_json")
      checks_json=$(jq -c '.checks' <<< "$pr_json")

      # CI pass: ALL checks must have conclusion == "SUCCESS" (and there must be at least one)
      check_count=$(jq 'length' <<< "$checks_json")
      if [[ "$check_count" -gt 0 ]]; then
        all_pass=$(jq 'all(.conclusion == "SUCCESS")' <<< "$checks_json")
        any_fail=$(jq 'any(.conclusion == ("FAILURE", "ERROR"))' <<< "$checks_json")
        if [[ "$all_pass" == "true" ]]; then
          emit_if_changed "$num" "[PR_CI_PASS]"
        fi
        if [[ "$any_fail" == "true" ]]; then
          reset_pr_seen "$num"
          emit_if_changed "$num" "[PR_CI_FAIL]"
        fi
      fi

      # Merge conflicts
      if [[ "$mergeable" == "CONFLICTING" ]]; then
        emit_if_changed "$num" "[PR_CONFLICT]"
      fi
    done

  # Check recently merged AutoShip PRs (last 30s window)
  gh pr list --label autoship --state merged \
    --json number,mergedAt \
    --repo "$REPO_SLUG" 2>/dev/null | \
    jq -r '.[] | "\(.number) \(.mergedAt)"' | \
    while read -r num merged_at; do
      # Only emit for merges in the last 60 seconds.
      # macOS `date -j -f` treats input as local time unless -u is set; force UTC.
      merged_epoch=$(date -j -u -f "%Y-%m-%dT%H:%M:%SZ" "$merged_at" "+%s" 2>/dev/null || \
                     date -u -d "$merged_at" "+%s" 2>/dev/null || echo 0)
      now_epoch=$(date -u +%s)
      age=$((now_epoch - merged_epoch))
      if [[ $age -lt 60 ]]; then
        emit_if_changed "$num" "[PR_MERGED]"

        # Transition state to merged and write completed_at for the linked issue.
        # Look up which issue this PR belongs to by matching pr_number in state.json.
        # AutoShip state transition is gated by the same seen-set as [PR_MERGED] so it fires once per PR.
        if [[ -f "$STATE_FILE" ]] && command -v jq >/dev/null 2>&1; then
          ISSUE_ID=$(jq -r --argjson pr "$num" \
            '.issues | to_entries[] | select(.value.pr_number == $pr) | .key' \
            "$STATE_FILE" 2>/dev/null | head -1)
          if [[ -n "$ISSUE_ID" ]]; then
            autoship_key="${num}:AUTOSHIP_MERGED"
            autoship_seen=$(jq -r --arg k "$autoship_key" '.[$k] // empty' "$SEEN_FILE")
            if [[ -z "$autoship_seen" ]]; then
              COMPLETED_AT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
              bash "$REPO_ROOT/hooks/update-state.sh" set-merged "$ISSUE_ID" \
                completed_at="$COMPLETED_AT" 2>/dev/null || true
              echo "[AUTOSHIP] Transitioned issue $ISSUE_ID to merged (PR #$num, completed_at=$COMPLETED_AT)"
              jq --arg k "$autoship_key" --arg now "$(date -u +%s)" '.[$k] = $now' \
                "$SEEN_FILE" > "$_SEEN_TMP" && mv "$_SEEN_TMP" "$SEEN_FILE"
            fi
          fi
        fi
      fi
    done

  sleep 30
done
