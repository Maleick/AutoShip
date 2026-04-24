#!/usr/bin/env bash
set -euo pipefail

# init.sh — Initialize .autoship/ directory structure and state file.
# Idempotent: safe to re-run without losing existing state.

AUTOSHIP_VERSION="1.4.0"

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
LEDGER_FILE="$AUTOSHIP_DIR/token-ledger.json"

# Resolve the directory this script lives in so we can call sibling scripts.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Detect repo
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

echo "AutoShip v${AUTOSHIP_VERSION} initializing..."

# Check for jq dependency and minimum version (>= 1.6 required for --argjson)
HAS_JQ=1
if ! command -v jq >/dev/null 2>&1; then
  HAS_JQ=0
  echo "Warning: jq not found. Install with: brew install jq" >&2
else
  jq_version=$(jq --version 2>/dev/null | sed 's/jq-//' | cut -d. -f1-2) || jq_version="0.0"
  jq_major=$(echo "$jq_version" | cut -d. -f1)
  jq_minor=$(echo "$jq_version" | cut -d. -f2)
  if (( jq_major < 1 || (jq_major == 1 && jq_minor < 6) )); then
    echo "Warning: jq ${jq_version} found, but >= 1.6 is required. Some features may not work." >&2
  fi
fi

# Check for gh CLI and minimum version (>= 2.0 required for --json flag)
if ! command -v gh >/dev/null 2>&1; then
  echo "Warning: gh not found. Install with: brew install gh && gh auth login" >&2
else
  gh_version=$(gh --version 2>/dev/null | head -1 | awk '{print $3}') || gh_version="0.0.0"
  gh_major=$(echo "$gh_version" | cut -d. -f1)
  if (( gh_major < 2 )); then
    echo "Warning: gh ${gh_version} found, but >= 2.0 is required. Upgrade with: brew upgrade gh" >&2
  fi
fi

# Derive owner/repo from git remote
REPO_SLUG=""
REMOTE_URL=$(git remote get-url origin 2>/dev/null) || true
if [[ -n "$REMOTE_URL" ]]; then
  # Handle both SSH and HTTPS remotes
  REPO_SLUG=$(echo "$REMOTE_URL" | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
fi

# Create directory structure
mkdir -p "$WORKSPACES_DIR"

# Initialize .pr-monitor-seen.json (used by monitor-prs.sh)
PR_SEEN_FILE="$AUTOSHIP_DIR/.pr-monitor-seen.json"
if [[ ! -f "$PR_SEEN_FILE" ]]; then
  echo '{}' > "$PR_SEEN_FILE"
  echo "Initialized $PR_SEEN_FILE"
fi

# Initialize event-queue.json (used by emit-event.sh)
EVENT_QUEUE_FILE="$AUTOSHIP_DIR/event-queue.json"
if [[ ! -f "$EVENT_QUEUE_FILE" ]]; then
  echo '[]' > "$EVENT_QUEUE_FILE"
  echo "Initialized $EVENT_QUEUE_FILE"
fi

# Detect OpenCode runtime and parse provider status.
TOOLS_RAW=$(bash "$SCRIPT_DIR/detect-tools.sh" 2>/dev/null) || TOOLS_RAW='{}'
TOOLS_JSON=$(printf '%s' "$TOOLS_RAW" | jq '
  to_entries
  | map({
      key: .key,
      value: {
        status: (if .value.available then "available" else "unavailable" end),
        quota_pct: .value.quota_pct
      }
    })
  | from_entries
  ' 2>/dev/null) || TOOLS_JSON='{"opencode":{"status":"available","quota_pct":-1}}'

# Initialize quota.json if it doesn't exist (creates decay-tracking file for third-party tools)
if [[ ! -f "$AUTOSHIP_DIR/quota.json" ]]; then
  bash "$SCRIPT_DIR/quota-update.sh" init 2>/dev/null || true
fi

# --- Error Recovery #6: Invalid/corrupted state file ---
# If state.json exists but is not valid JSON, back it up and reinitialize.
if [[ "$HAS_JQ" -eq 1 && -f "$STATE_FILE" ]]; then
  if ! jq '.' "$STATE_FILE" > /dev/null 2>&1; then
    BACKUP="${STATE_FILE}.corrupted.$(date +%s)"
    mv "$STATE_FILE" "$BACKUP"
    echo "State file corrupted — backed up to $BACKUP, initializing fresh" >&2
  fi
fi

# Initialize state.json only if it doesn't exist
if [[ ! -f "$STATE_FILE" ]]; then
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  if [[ "$HAS_JQ" -eq 1 ]]; then
    # Use jq to produce well-formed JSON, embedding the detected tools.
    jq -n \
      --arg repo "$REPO_SLUG" \
      --arg now "$NOW" \
      --arg ver "$AUTOSHIP_VERSION" \
      --argjson tools "$TOOLS_JSON" \
      '{
        version: 1,
        autoship_version: $ver,
        repo: $repo,
        started_at: $now,
        updated_at: $now,
        plan: {
          phases: [],
          current_phase: 0,
          checkpoint_pending: false
        },
        issues: {},
        tools: $tools,
        stats: {
          session_dispatched: 0,
          session_completed: 0,
          total_dispatched_all_time: 0,
          total_completed_all_time: 0,
          failed: 0,
          blocked: 0
        }
      }' > "$STATE_FILE"
  else
    REPO_SLUG="$REPO_SLUG" AUTOSHIP_VERSION="$AUTOSHIP_VERSION" NOW="$NOW" TOOLS_JSON="$TOOLS_JSON" \
      python3 - <<'PY' > "$STATE_FILE"
import json
import os
import sys

state = {
    "version": 1,
    "autoship_version": os.environ["AUTOSHIP_VERSION"],
    "repo": os.environ["REPO_SLUG"],
    "started_at": os.environ["NOW"],
    "updated_at": os.environ["NOW"],
    "plan": {"phases": [], "current_phase": 0, "checkpoint_pending": False},
    "issues": {},
    "tools": json.loads(os.environ["TOOLS_JSON"]),
    "stats": {
        "session_dispatched": 0,
        "session_completed": 0,
        "total_dispatched_all_time": 0,
        "total_completed_all_time": 0,
        "failed": 0,
        "blocked": 0,
    },
}

json.dump(state, sys.stdout, indent=2)
sys.stdout.write("\n")
PY
  fi
  echo "Initialized $STATE_FILE"
else
  if [[ "$HAS_JQ" -eq 1 ]]; then
    # Check for version mismatch and warn (migration notice, non-fatal)
    existing_ver=$(jq -r '.autoship_version // "unknown"' "$STATE_FILE" 2>/dev/null) || existing_ver="unknown"
    if [[ "$existing_ver" != "$AUTOSHIP_VERSION" ]]; then
      echo "Notice: state.json has autoship_version=${existing_ver}, current is ${AUTOSHIP_VERSION} (migration may be needed)" >&2
    fi

    # Refresh tools section with current quota data without touching other state.
    # Also reset session-scoped counters on each startup, and migrate old key names if present.
    NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    UPDATED=$(jq --argjson tools "$TOOLS_JSON" --arg now "$NOW" '
      .tools = $tools |
      .updated_at = $now |
      # Migrate old "dispatched"/"completed" keys to new schema (keep totals, reset session)
      if (.stats | has("dispatched")) then
        .stats.total_dispatched_all_time = (.stats.total_dispatched_all_time // .stats.dispatched) |
        .stats.total_completed_all_time  = (.stats.total_completed_all_time  // .stats.completed) |
        del(.stats.dispatched) | del(.stats.completed)
      else . end |
      # Ensure all four keys exist
      .stats.session_dispatched        = 0 |
      .stats.session_completed         = 0 |
      .stats.total_dispatched_all_time = (.stats.total_dispatched_all_time // 0) |
      .stats.total_completed_all_time  = (.stats.total_completed_all_time  // 0)
    ' "$STATE_FILE") && \
      printf '%s\n' "$UPDATED" > "$STATE_FILE"
    echo "Refreshed tools quota and reset session counters in $STATE_FILE"
  else
    echo "Warning: jq not found; skipping state refresh for existing $STATE_FILE" >&2
  fi
fi

# Create config.json for operator overrides (if not present)
CONFIG_FILE="$AUTOSHIP_DIR/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  echo '{}' > "$CONFIG_FILE"
  echo "Initialized $CONFIG_FILE (add test_command, etc. for overrides)"
fi

# Auto-populate test_command/verify_command if not present (idempotent)
if ! jq -e 'has("test_command")' "$CONFIG_FILE" >/dev/null 2>&1; then
  DETECTED_TEST=""
  # Scan CLAUDE.md and AGENTS.md for patterns
  for f in "CLAUDE.md" "AGENTS.md"; do
    if [[ -f "$f" ]]; then
      for p in "python3 scripts/dev-preflight.py" "cargo test" "pytest" "npm test" "make test" "./gradlew test"; do
        if grep -qF "$p" "$f"; then
          DETECTED_TEST="$p"
          break 2
        fi
      done
    fi
  done

  if [[ -n "$DETECTED_TEST" ]]; then
    UPDATED=$(jq --arg tc "$DETECTED_TEST" '.test_command = $tc | .verify_command = (.verify_command // $tc)' "$CONFIG_FILE")
    printf '%s\n' "$UPDATED" > "$CONFIG_FILE"
    echo "Auto-detected test_command: $DETECTED_TEST"
  else
    UPDATED=$(jq '.test_command = "" | .verify_command = (.verify_command // "")' "$CONFIG_FILE")
    printf '%s\n' "$UPDATED" > "$CONFIG_FILE"
    echo "Warning: no test_command detected — set it in .autoship/config.json"
  fi
elif ! jq -e 'has("verify_command")' "$CONFIG_FILE" >/dev/null 2>&1; then
  # Migration: test_command exists but verify_command does not.
  TC=$(jq -r '.test_command // ""' "$CONFIG_FILE")
  UPDATED=$(jq --arg tc "$TC" '.verify_command = $tc' "$CONFIG_FILE")
  printf '%s\n' "$UPDATED" > "$CONFIG_FILE"
fi

# Populate project-context.md from CLAUDE.md/AGENTS.md/config.json
bash "$SCRIPT_DIR/extract-context.sh" || true

# --- Token Ledger: create + append new session entry ---
init_token_ledger() {
  # The token ledger uses jq for both read and write paths, so skip it entirely
  # when jq is unavailable rather than logging a misleading success.
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi

  local ledger="$AUTOSHIP_DIR/token-ledger.json"
  local lock="${ledger%.json}.lock"
  local now
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  # Generate a session ID from epoch seconds + PID for uniqueness
  local session_id
  session_id="session-$(date -u +%s)-$$"

  # Build new session object
  local new_session
  new_session=$(jq -n \
    --arg sid "$session_id" \
    --arg now "$now" \
    --arg repo "$REPO_SLUG" \
    '{session_id: $sid, started_at: $now, repo: $repo, issues: []}')

  # Write new session into the ledger (caller must hold lock)
  _ledger_write() {
    local tmp
    tmp=$(mktemp)
    if [[ -f "$ledger" ]] && jq '.' "$ledger" >/dev/null 2>&1; then
      jq --argjson s "$new_session" '.sessions += [$s]' "$ledger" > "$tmp" \
        && mv "$tmp" "$ledger"
    else
      jq -n --argjson s "$new_session" \
        '{schema_version: 1, sessions: [$s]}' > "$tmp" \
        && mv "$tmp" "$ledger"
    fi
  }

  if command -v flock >/dev/null 2>&1; then
    exec 9>"$lock"
    flock -x 9
    _ledger_write
    exec 9>&-
  elif command -v lockf >/dev/null 2>&1; then
    # lockf -k holds the lock for the duration of the child process.
    # We write the session object to a temp file so the child can read it
    # without quoting/escaping issues.
    local session_tmp
    session_tmp=$(mktemp)
    chmod 600 "$session_tmp"   # close TOCTOU window before writing session data
    printf '%s' "$new_session" > "$session_tmp"
    # Pass paths as positional args ($1, $2) to avoid injection from special chars in paths
    lockf -k "$lock" bash -c '
      ledger="$1" session_tmp="$2"
      new_session=$(cat "$session_tmp")
      tmp=$(mktemp)
      if [[ -f "$ledger" ]] && jq '"'"'.'"'"' "$ledger" >/dev/null 2>&1; then
        jq --argjson s "$new_session" '"'"'.sessions += [$s]'"'"' "$ledger" > "$tmp" && mv "$tmp" "$ledger"
      else
        jq -n --argjson s "$new_session" '"'"'{schema_version: 1, sessions: [$s]}'"'"' > "$tmp" && mv "$tmp" "$ledger"
      fi
    ' _ "$ledger" "$session_tmp"
    rm -f "$session_tmp"
  else
    # No lock mechanism available — write directly
    _ledger_write
  fi

  echo "Token ledger updated: $ledger (session $session_id)"
}

init_token_ledger || true

# Create GitHub labels if the repo is on GitHub and gh CLI is available
create_github_labels() {
  # Check if gh CLI is available
  if ! command -v gh &> /dev/null; then
    return 0
  fi

  # Check if we're in a GitHub repo (REPO_SLUG will be set)
  if [[ -z "$REPO_SLUG" ]]; then
    return 0
  fi

  # Create each label if it doesn't exist (bash 3.2 compatible — no associative arrays)
  local pairs="autoship:in-progress=FFEB3B autoship:blocked=F44336 autoship:paused=FF9800 autoship:done=4CAF50"
  for pair in $pairs; do
    label="${pair%%=*}"
    color="${pair#*=}"
    if ! gh label list --repo "$REPO_SLUG" --json name --jq ".[].name" 2>/dev/null | grep -q "^${label}$"; then
      echo "Creating label: $label (color: $color)"
      gh label create "$label" --repo "$REPO_SLUG" --color "$color" --description "AutoShip orchestration label" 2>/dev/null || true
    fi
  done
}

# Attempt to create labels (non-fatal if it fails)
create_github_labels || true

# --- Error Recovery #1: Session Restart Reconciliation ---
# If state.json shows issues as running/claimed but the tmux pane is gone,
# reconcile against GitHub labels to decide whether to keep or reset each issue.
reconcile_stale_running() {
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi
  if [[ ! -f "$STATE_FILE" ]]; then
    return 0
  fi

  # Collect issue keys that are in running or claimed state
  local stale_keys
  stale_keys=$(jq -r '.issues | to_entries[] | select(.value.state == "running" or .value.state == "claimed") | .key' "$STATE_FILE" 2>/dev/null) || return 0

  if [[ -z "$stale_keys" ]]; then
    return 0
  fi

  echo "Reconciling stale running/claimed issues..."

  while IFS= read -r key; do
    [[ -z "$key" ]] && continue

    # Check if the tmux pane for this issue is still alive
    local pane_id
    pane_id=$(jq -r --arg k "$key" '.issues[$k].pane_id // empty' "$STATE_FILE" 2>/dev/null) || pane_id=""

    local pane_alive=0
    if [[ -n "$pane_id" ]] && command -v tmux >/dev/null 2>&1; then
      if tmux list-panes -a -F '#{pane_id}' 2>/dev/null | grep -q "^${pane_id}$"; then
        pane_alive=1
      fi
    fi

    if [[ $pane_alive -eq 1 ]]; then
      echo "  $key: pane $pane_id still alive — keeping as running"
      continue
    fi

    # Pane is gone. Check GitHub labels as source of truth (if gh available).
    local has_inprogress_label=0
    if [[ -n "$REPO_SLUG" ]] && command -v gh >/dev/null 2>&1; then
      # Extract numeric issue ID from key (e.g. "issue-42" → 42)
      local issue_num="${key#issue-}"
      if gh issue view "$issue_num" --repo "$REPO_SLUG" --json labels \
          --jq '.labels[].name' 2>/dev/null | grep -q "^autoship:in-progress$"; then
        has_inprogress_label=1
      fi
    fi

    # Check worktree directory
    local worktree_path="$WORKSPACES_DIR/$key"
    local worktree_exists=0
    [[ -d "$worktree_path" ]] && worktree_exists=1

    local state_tmp
    state_tmp=$(mktemp)
    trap 'rm -f "${state_tmp:-}"' RETURN
    if [[ $has_inprogress_label -eq 1 ]]; then
      # Agent may be running in a different session — preserve state but clear dead pane ref
      echo "  $key: autoship:in-progress label present — preserving running state, clearing pane_id"
      jq --arg k "$key" '.issues[$k].pane_id = null' "$STATE_FILE" > "$state_tmp" \
        && mv "$state_tmp" "$STATE_FILE"
    else
      # No label, no pane → reset to unclaimed
      echo "  $key: no active pane or in-progress label — resetting to unclaimed"
      if [[ $worktree_exists -eq 1 ]]; then
        # Keep worktree/assignment, just reset state
        jq --arg k "$key" \
          '.issues[$k].state = "unclaimed" | .issues[$k].pane_id = null' \
          "$STATE_FILE" > "$state_tmp" && mv "$state_tmp" "$STATE_FILE"
      else
        # Clear everything
        jq --arg k "$key" \
          '.issues[$k].state = "unclaimed" | .issues[$k].pane_id = null | .issues[$k].worktree = null | .issues[$k].agent = null' \
          "$STATE_FILE" > "$state_tmp" && mv "$state_tmp" "$STATE_FILE"
      fi
    fi
  done <<< "$stale_keys"
}

reconcile_stale_running || true

# --- Warning check: available-but-never-dispatched tools ---
# If total dispatches >= 10 and a tool is marked available but has never been dispatched, warn
check_available_never_dispatched() {
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi
  if [[ ! -f "$STATE_FILE" ]]; then
    return 0
  fi

  local total_dispatches
  total_dispatches=$(jq -r '.stats.total_dispatched_all_time // 0' "$STATE_FILE" 2>/dev/null) || total_dispatches=0

  # Only check if we've done 10+ dispatches
  if (( total_dispatches < 10 )); then
    return 0
  fi

  # Check each tool — warn if available but never dispatched
  jq -r '.tools | to_entries[] | select(.value.status == "available") | .key' "$STATE_FILE" 2>/dev/null | while read -r tool; do
    # Count how many issues were dispatched to this tool
    local tool_dispatches
    tool_dispatches=$(jq -r --arg t "$tool" '.issues | to_entries[] | select(.value.agent == $t) | length' "$STATE_FILE" 2>/dev/null | wc -l) || tool_dispatches=0

    if (( tool_dispatches == 0 )); then
      echo "WARN: $tool available but never dispatched (${total_dispatches} total dispatches) — check CLI" >> "$AUTOSHIP_DIR/poll.log"
    fi
  done
}

check_available_never_dispatched || true

# --- Routing Config: parse AUTOSHIP.md front matter and write .autoship/routing.json ---
# Default routing matrix used when AUTOSHIP.md is absent or YAML is malformed.
DEFAULT_ROUTING='{
  "routing": {
    "research":     ["opencode"],
    "docs":         ["opencode"],
    "simple_code":  ["opencode"],
    "medium_code":  ["opencode"],
    "complex":      ["opencode"],
    "mechanical":   ["opencode"],
    "ci_fix":       ["opencode"],
    "rust_unsafe":  ["opencode"]
  },
  "quota_thresholds": {"low": 10, "exhausted": 0},
  "stall_timeout_ms": 300000,
  "max_concurrent_agents": 10
}'

load_routing_config() {
  local routing_file="$AUTOSHIP_DIR/routing.json"
  local autoship_md="AUTOSHIP.md"

  # If AUTOSHIP.md is absent, write the default and return.
  if [[ ! -f "$autoship_md" ]]; then
    printf '%s\n' "$DEFAULT_ROUTING" > "$routing_file"
    echo "routing.json initialized with defaults (AUTOSHIP.md not found)"
    return 0
  fi

  # Extract YAML front matter (content between the first pair of --- markers).
  local front_matter
  front_matter=$(awk '/^---$/{if(p){exit}else{p=1;next}} p{print}' "$autoship_md" 2>/dev/null) || front_matter=""

  if [[ -z "$front_matter" ]]; then
    printf '%s\n' "$DEFAULT_ROUTING" > "$routing_file"
    echo "routing.json initialized with defaults (no front matter in AUTOSHIP.md)" >&2
    return 0
  fi

  # Parse each routing entry: "  key: [a, b, c]" → JSON array
  # Also parse scalar fields: quota_thresholds, stall_timeout_ms, max_concurrent_agents.
  local parsed
  parsed=$(printf '%s\n' "$front_matter" | python3 - << 'PYEOF'
import sys, json, re

text = sys.stdin.read()

def parse_inline_list(s):
    """Parse '[a, b, c]' into a Python list of strings."""
    s = s.strip()
    if s.startswith('[') and s.endswith(']'):
        items = [x.strip().strip('"').strip("'") for x in s[1:-1].split(',') if x.strip()]
        return items
    return None

result = {}
routing = {}
quota = {}
stall = None
max_agents = None

lines = text.splitlines()
current_section = None

for line in lines:
    # Top-level key (no leading spaces)
    m = re.match(r'^(\w+):\s*(.*)', line)
    if m:
        key, val = m.group(1), m.group(2).strip()
        if key == 'routing':
            current_section = 'routing'
        elif key == 'quota_thresholds':
            current_section = 'quota_thresholds'
        elif key == 'stall_timeout_ms':
            try:
                stall = int(val)
            except ValueError:
                pass
            current_section = None
        elif key == 'max_concurrent_agents':
            try:
                max_agents = int(val)
            except ValueError:
                pass
            current_section = None
        continue

    # Indented key under current section
    m2 = re.match(r'^\s+(\w+):\s*(.*)', line)
    if m2 and current_section:
        k, v = m2.group(1), m2.group(2).strip()
        if current_section == 'routing':
            lst = parse_inline_list(v)
            if lst is not None:
                routing[k] = lst
        elif current_section == 'quota_thresholds':
            try:
                quota[k] = int(v)
            except ValueError:
                pass

result['routing'] = routing if routing else None
result['quota_thresholds'] = quota if quota else None
if stall is not None:
    result['stall_timeout_ms'] = stall
if max_agents is not None:
    result['max_concurrent_agents'] = max_agents

print(json.dumps(result))
PYEOF
  ) 2>/dev/null || parsed=""

  # Validate parsed result — must have a non-empty routing object.
  if [[ -z "$parsed" ]] || ! printf '%s' "$parsed" | jq -e '.routing | length > 0' >/dev/null 2>&1; then
    printf '%s\n' "$DEFAULT_ROUTING" > "$routing_file"
    echo "routing.json initialized with defaults (AUTOSHIP.md front matter parse failed)" >&2
    return 0
  fi

  # Detect project profile; all profiles remain on OpenCode.
  local project_profile="standard"
  if [[ -f "Cargo.toml" ]] && grep -qr "\[cfg(windows)\]" src/ 2>/dev/null; then
    project_profile="rust_windows"
    echo "Project profile: rust_windows detected (routing overrides applied)"
  fi

  # Merge parsed values over defaults so missing fields fall back gracefully.
  local merged
  merged=$(jq -n \
    --argjson defaults "$DEFAULT_ROUTING" \
    --argjson parsed "$parsed" \
    --arg profile "$project_profile" \
    '
      $defaults
      | if ($parsed.routing | length) > 0 then .routing = $parsed.routing else . end
      | if ($profile == "rust_windows") then
          .routing |= with_entries(
            if .key == "rust_unsafe" then .
            else .value = ["opencode"]
            end
          )
        else . end
      | if ($parsed.quota_thresholds | length) > 0 then .quota_thresholds = $parsed.quota_thresholds else . end
      | if $parsed.stall_timeout_ms != null then .stall_timeout_ms = $parsed.stall_timeout_ms else . end
      | if $parsed.max_concurrent_agents != null then .max_concurrent_agents = $parsed.max_concurrent_agents else . end
    ' 2>/dev/null) || merged="$DEFAULT_ROUTING"

  printf '%s\n' "$merged" > "$routing_file"
  echo "routing.json loaded from AUTOSHIP.md front matter"
}

load_routing_config || true

# Write hooks_dir so skills can locate sibling hooks without relative paths.
echo "$SCRIPT_DIR" > "$AUTOSHIP_DIR/hooks_dir"

# Sweep stale worktrees on startup (non-fatal if it fails)
echo "Scanning for stale worktrees..."
bash "$SCRIPT_DIR/sweep-stale.sh" 2>/dev/null || true

echo "AutoShip workspace ready at $AUTOSHIP_DIR"
