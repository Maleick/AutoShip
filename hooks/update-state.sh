#!/usr/bin/env bash
set -euo pipefail

# update-state.sh — Update .autoship/state.json for a given issue.
# Usage: update-state.sh <action> <issue-id> [key=value ...]
# Actions: set-claimed, set-running, set-verifying, set-completed, set-blocked, set-merged, set-failed, set-paused

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
LEDGER_FILE="$AUTOSHIP_DIR/token-ledger.json"

# Locate repo root
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
STATE_FILE="$REPO_ROOT/$STATE_FILE"
LEDGER_FILE="$REPO_ROOT/$LEDGER_FILE"

# Acquire exclusive lock for entire script duration.
# Prevents concurrent read-modify-write races between parallel update-state.sh invocations.
LOCK_FILE="${STATE_FILE%.json}.lock"
if [[ -z "${AUTOSHIP_STATE_LOCKED:-}" ]]; then
  export AUTOSHIP_STATE_LOCKED=1
  if command -v flock >/dev/null 2>&1; then
    # Linux: hold FD lock for script duration
    exec 9>"$LOCK_FILE"
    flock -x 9
  elif command -v lockf >/dev/null 2>&1; then
    # macOS (BSD): re-exec under lockf; AUTOSHIP_STATE_LOCKED prevents infinite loop
    exec lockf -k "$LOCK_FILE" "$0" "$@"
  fi
  # No lock mechanism available -- proceed without locking
fi


if [[ ! -f "$STATE_FILE" ]]; then
  echo "Error: $STATE_FILE not found. Run init.sh first." >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: jq is required but not found" >&2
  exit 1
fi

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <action> <issue-id> [key=value ...]" >&2
  echo "Actions: set-claimed, set-running, set-verifying, set-completed, set-blocked, set-merged, set-failed, set-paused" >&2
  exit 1
fi

# Helper function to manage GitHub labels (bash 3.2 compatible)
# Usage: manage_labels <issue-id> <add-label> [remove-label1] [remove-label2] ...
manage_labels() {
  local issue_id="$1"
  local add_label="$2"
  shift 2
  local remove_labels=("$@")

  # Check if gh is available and repo info exists
  if ! command -v gh >/dev/null 2>&1; then
    return 0
  fi

  # Get repo slug from state.json
  local repo_slug
  repo_slug=$(jq -r '.repo // empty' "$STATE_FILE") || return 0
  if [[ -z "$repo_slug" ]]; then
    return 0
  fi

  # Remove old labels first
  for old_label in "${remove_labels[@]}"; do
    # Verify the label exists before trying to remove it
    if gh label list --repo "$repo_slug" --json name --jq ".[].name" 2>/dev/null | grep -q "^${old_label}$"; then
      gh issue edit "$issue_id" --repo "$repo_slug" --remove-label "$old_label" 2>/dev/null || true
    fi
  done

  # Add new label (if not already present)
  if [[ -n "$add_label" ]]; then
    if gh label list --repo "$repo_slug" --json name --jq ".[].name" 2>/dev/null | grep -q "^${add_label}$"; then
      gh issue edit "$issue_id" --repo "$repo_slug" --add-label "$add_label" 2>/dev/null || true
    fi
  fi
}

# append_ledger_record — append an issue record to the current session in token-ledger.json.
# Usage: append_ledger_record <issue_id> <verdict>
# Reads complexity, agent, pr_number, attempt, task_type, started_at from state.json.
append_ledger_record() {
  local issue_id="$1"
  local verdict="$2"
  local ledger="$LEDGER_FILE"
  local lock="${ledger%.json}.lock"

  if [[ ! -f "$ledger" ]]; then
    # Ledger not created yet (init hasn't run with ledger support) — skip
    return 0
  fi

  # Read fields from state.json
  local issue_number complexity agent pr_number attempt task_type started_at duration_ms tokens_used
  issue_number=$(echo "$issue_id" | grep -o '[0-9]*' | head -1)
  complexity=$(jq -r --arg k "$issue_id" '.issues[$k].complexity // "medium"' "$STATE_FILE" 2>/dev/null) || complexity="medium"
  agent=$(jq -r --arg k "$issue_id" '.issues[$k].agent // ""' "$STATE_FILE" 2>/dev/null) || agent=""
  pr_number=$(jq -r --arg k "$issue_id" '.issues[$k].pr_number // 0' "$STATE_FILE" 2>/dev/null) || pr_number=0
  attempt=$(jq -r --arg k "$issue_id" '.issues[$k].attempt // 1' "$STATE_FILE" 2>/dev/null) || attempt=1
  task_type=$(jq -r --arg k "$issue_id" '.issues[$k].task_type // "medium_code"' "$STATE_FILE" 2>/dev/null) || task_type="medium_code"
  started_at=$(jq -r --arg k "$issue_id" '.issues[$k].started_at // ""' "$STATE_FILE" 2>/dev/null) || started_at=""
  tokens_used=$(jq -r --arg k "$issue_id" '.issues[$k].tokens_used // 0' "$STATE_FILE" 2>/dev/null) || tokens_used=0

  # Compute duration_ms from started_at to now
  duration_ms=0
  if [[ -n "$started_at" ]]; then
    local start_epoch now_epoch
    start_epoch=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$started_at" +%s 2>/dev/null \
      || date -d "$started_at" +%s 2>/dev/null \
      || echo 0)
    now_epoch=$(date -u +%s)
    if [[ "$start_epoch" -gt 0 && "$now_epoch" -ge "$start_epoch" ]]; then
      duration_ms=$(( (now_epoch - start_epoch) * 1000 ))
    fi
  fi

  # Coerce pr_number, attempt, and tokens_used to integers
  pr_number=$(( ${pr_number:-0} )) || pr_number=0
  attempt=$(( ${attempt:-1} )) || attempt=1
  tokens_used=$(( ${tokens_used:-0} )) || tokens_used=0

  # Build the record JSON
  local record
  record=$(jq -n \
    --argjson num "$issue_number" \
    --arg type "$task_type" \
    --arg complexity "$complexity" \
    --arg agent "$agent" \
    --argjson tokens "$tokens_used" \
    --argjson dur "$duration_ms" \
    --arg verdict "$verdict" \
    --argjson pr "$pr_number" \
    --argjson att "$attempt" \
    '{number: $num, type: $type, complexity: $complexity, agent: $agent,
      tokens_used: $tokens, duration_ms: $dur, verdict: $verdict,
      pr_number: $pr, attempt: $att}')

  # Write record into last session using lock
  _write_ledger_record() {
    local tmp
    tmp=$(mktemp)
    jq --argjson r "$record" \
      'if (.sessions | length) > 0
       then .sessions[-1].issues += [$r]
       else .
       end' \
      "$ledger" > "$tmp" && mv "$tmp" "$ledger"
  }

  if command -v flock >/dev/null 2>&1; then
    exec 9>"$lock"
    flock -x 9
    _write_ledger_record
    exec 9>&-
  elif command -v lockf >/dev/null 2>&1; then
    local record_tmp
    record_tmp=$(mktemp)
    chmod 600 "$record_tmp"   # close TOCTOU window before writing record data
    printf '%s' "$record" > "$record_tmp"
    # Pass paths as positional args ($1, $2) to avoid shell injection from special chars in paths
    lockf -k "$lock" bash -c '
      ledger="$1" record_tmp="$2"
      record=$(cat "$record_tmp")
      tmp=$(mktemp)
      jq --argjson r "$record" \
        '"'"'if (.sessions | length) > 0 then .sessions[-1].issues += [$r] else . end'"'"' \
        "$ledger" > "$tmp" && mv "$tmp" "$ledger"
    ' _ "$ledger" "$record_tmp"
    rm -f "$record_tmp"
  else
    _write_ledger_record
  fi
}

ACTION="$1"
ISSUE_ID="$2"
shift 2

# Validate ISSUE_ID format: reject malformed values before they reach jq keys or GitHub API calls
if [[ ! "$ISSUE_ID" =~ ^(issue-)?[0-9]+$ ]]; then
  echo "Error: invalid ISSUE_ID: $ISSUE_ID" >&2
  exit 1
fi

NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Cleanup temp files on exit
TMP_FILES=()
cleanup() { for f in "${TMP_FILES[@]+"${TMP_FILES[@]}"}"; do rm -f "$f"; done; }
trap cleanup EXIT

make_tmp() { local t; t=$(mktemp); TMP_FILES+=("$t"); echo "$t"; }

# Map action to state, stat counter, and GitHub labels
case "$ACTION" in
  set-claimed)
    NEW_STATE="claimed"
    STAT_KEY=""
    ADD_LABEL=""
    REMOVE_LABELS=()
    ;;
  set-running)
    NEW_STATE="running"
    STAT_KEY="dispatched"  # signals: increment session_dispatched + total_dispatched_all_time
    ADD_LABEL="autoship:in-progress"
    REMOVE_LABELS=("autoship:blocked" "autoship:paused" "autoship:done")
    ;;
  set-verifying)
    NEW_STATE="verifying"
    STAT_KEY=""
    ADD_LABEL=""
    REMOVE_LABELS=()
    ;;
  set-completed)
    NEW_STATE="approved"
    STAT_KEY="completed"  # signals: increment session_completed + total_completed_all_time
    ADD_LABEL=""
    REMOVE_LABELS=()
    ;;
  set-blocked)
    NEW_STATE="blocked"
    STAT_KEY="blocked"
    ADD_LABEL="autoship:blocked"
    REMOVE_LABELS=("autoship:in-progress" "autoship:paused" "autoship:done")
    ;;
  set-merged)
    NEW_STATE="merged"
    STAT_KEY="completed"  # signals: increment session_completed + total_completed_all_time
    ADD_LABEL="autoship:done"
    REMOVE_LABELS=("autoship:in-progress" "autoship:blocked" "autoship:paused")
    ;;
  set-paused)
    NEW_STATE="paused"
    STAT_KEY=""
    ADD_LABEL="autoship:paused"
    REMOVE_LABELS=()
    ;;
  set-failed)
    NEW_STATE="blocked"
    STAT_KEY="failed"
    ADD_LABEL="autoship:blocked"
    REMOVE_LABELS=("autoship:in-progress" "autoship:paused" "autoship:done")
    ;;
  *)
    echo "Error: unknown action '$ACTION'" >&2
    echo "Valid actions: set-claimed, set-running, set-verifying, set-completed, set-blocked, set-merged, set-failed, set-paused" >&2
    exit 1
    ;;
esac

# Normalize state key: convert underscores to hyphens
NEW_STATE=$(echo "$NEW_STATE" | tr '_' '-')

# Ensure the issue entry exists (initialize if new)
CURRENT=$(jq -r --arg id "$ISSUE_ID" '.issues[$id] // empty' "$STATE_FILE")
if [[ -z "$CURRENT" ]]; then
  # Create a new issue entry
  TMP=$(make_tmp)
  jq --arg id "$ISSUE_ID" --arg now "$NOW" \
    '.issues[$id] = {"state": "unclaimed", "complexity": "medium", "agent": "", "attempt": 1, "worktree": "", "pane_id": "", "started_at": $now, "attempts_history": []}' \
    "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
fi

# Special handling for retries in set-running: preserve original started_at as first_started_at
if [[ "$NEW_STATE" == "running" ]]; then
  # Parse attempt from key=value args (default to 1 if not provided)
  ATTEMPT=1
  for pair in "$@"; do
    if [[ "$pair" == attempt=* ]]; then
      ATTEMPT="${pair#*=}"
      break
    fi
  done

  # On retry (attempt > 1), preserve original started_at as first_started_at
  if [[ "$ATTEMPT" -gt 1 ]]; then
    TMP=$(make_tmp)
    jq --arg key "$ISSUE_ID" \
      ".issues[\$key].first_started_at = (.issues[\$key].first_started_at // .issues[\$key].started_at) | .issues[\$key].started_at = (now | todate)" \
      "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
  fi
fi

# For set-merged: assert title and agent are present (enrich from GitHub if absent)
if [[ "$ACTION" == "set-merged" ]]; then
  ISSUE_NUMBER=$(echo "$ISSUE_ID" | grep -o '[0-9]*')
  
  TITLE=$(jq -r --arg k "$ISSUE_ID" '.issues[$k].title // ""' "$STATE_FILE")
  AGENT=$(jq -r --arg k "$ISSUE_ID" '.issues[$k].agent // ""' "$STATE_FILE")
  
  if [[ -z "$TITLE" ]]; then
    TITLE=$(gh issue view "$ISSUE_NUMBER" --json title --jq '.title' 2>/dev/null || echo '(unknown)')
  fi
  if [[ -z "$AGENT" ]]; then
    AGENT="direct"
  fi
  
  # Update state with title and agent
  TMP=$(make_tmp)
  jq --arg id "$ISSUE_ID" --arg state "$NEW_STATE" --arg now "$NOW" --arg title "$TITLE" --arg agent "$AGENT" \
    '.issues[$id].state = $state | .updated_at = $now | .issues[$id].title = $title | .issues[$id].agent = $agent' \
    "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
else
  # Standard state update
  TMP=$(make_tmp)
  jq --arg id "$ISSUE_ID" --arg state "$NEW_STATE" --arg now "$NOW" \
    '.issues[$id].state = $state | .updated_at = $now' \
    "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
fi
# Increment stat counters if applicable.
# "dispatched" → session_dispatched + total_dispatched_all_time
# "completed"  → session_completed  + total_completed_all_time
# other keys   → incremented directly (e.g. "blocked", "failed")
if [[ -n "$STAT_KEY" ]]; then
  TMP=$(make_tmp)
  case "$STAT_KEY" in
    dispatched)
      jq '
        .stats.session_dispatched        = ((.stats.session_dispatched        // 0) + 1) |
        .stats.total_dispatched_all_time = ((.stats.total_dispatched_all_time // 0) + 1)
      ' "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
      ;;
    completed)
      jq '
        .stats.session_completed        = ((.stats.session_completed        // 0) + 1) |
        .stats.total_completed_all_time = ((.stats.total_completed_all_time // 0) + 1)
      ' "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
      ;;
    *)
      jq --arg key "$STAT_KEY" \
        '.stats[$key] = ((.stats[$key] // 0) + 1)' \
        "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
      ;;
  esac
fi

# Append issue record to token ledger for completion events
if [[ "$ACTION" == "set-completed" || "$ACTION" == "set-merged" ]]; then
  append_ledger_record "$ISSUE_ID" "pass" || true
fi

# Manage GitHub labels for lifecycle transitions
if [[ -n "$ADD_LABEL" ]] || [[ ${#REMOVE_LABELS[@]} -gt 0 ]]; then
  manage_labels "$ISSUE_ID" "$ADD_LABEL" "${REMOVE_LABELS[@]}"
fi

# Apply optional key=value overrides
for pair in "$@"; do
  KEY="${pair%%=*}"
  VALUE="${pair#*=}"

  # Validate PR→issue assignment before writing pr_number
  if [[ "$KEY" == "pr_number" ]]; then
    ISSUE_NUMBER=$(echo "$ISSUE_ID" | grep -o '[0-9]*')
    if command -v gh >/dev/null 2>&1; then
      PR_ISSUE=$(gh pr view "$VALUE" --json body --jq '.body' 2>/dev/null | grep -o '#[0-9]*' | head -1 | tr -d '#')
      if [[ -n "$PR_ISSUE" && "$PR_ISSUE" != "$ISSUE_NUMBER" ]]; then
        echo "WARN: PR #$VALUE body references #$PR_ISSUE but expected #$ISSUE_NUMBER — possible transposition" >> "$REPO_ROOT/.autoship/poll.log"
      fi
    fi
  fi

  TMP=$(make_tmp)
  # Try to parse as JSON (for numbers/booleans), fall back to string
  if echo "$VALUE" | jq -e '.' >/dev/null 2>&1; then
    jq --arg id "$ISSUE_ID" --arg key "$KEY" --argjson val "$VALUE" \
      '.issues[$id][$key] = $val' \
      "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
  else
    jq --arg id "$ISSUE_ID" --arg key "$KEY" --arg val "$VALUE" \
      '.issues[$id][$key] = $val' \
      "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
  fi

  # When agent is set to a Claude model (non-worktree dispatch), mark worktree_free=true
  # so monitor-agents.sh skips the issue and defers to monitor-prs.sh for completion.
  if [[ "$KEY" == "agent" ]]; then
    case "$VALUE" in
      claude-haiku|claude-sonnet|claude-haiku-*|claude-sonnet-*)
        TMP=$(make_tmp)
        jq --arg id "$ISSUE_ID" \
          '.issues[$id].worktree_free = true' \
          "$STATE_FILE" > "$TMP" && mv "$TMP" "$STATE_FILE"
        ;;
    esac
  fi
done

echo "Updated issue $ISSUE_ID: state=$NEW_STATE"
