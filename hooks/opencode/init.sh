#!/bin/bash
# Initialize .autoship/ directory for OpenCode.

set -euo pipefail

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
LEDGER_FILE="$AUTOSHIP_DIR/token-ledger.json"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RELEASE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
if [[ -f "$RELEASE_ROOT/VERSION" ]]; then
  AUTOSHIP_VERSION="$(tr -d '[:space:]' <"$RELEASE_ROOT/VERSION")"
else
  AUTOSHIP_VERSION="dev"
fi

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

echo "AutoShip ${AUTOSHIP_VERSION} initializing for OpenCode..."

bash "$SCRIPT_DIR/sync-release.sh"

if ! command -v jq >/dev/null 2>&1; then
  echo "Warning: jq not found. Install with: brew install jq" >&2
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "Warning: gh not found. Install with: brew install gh" >&2
fi

REPO_SLUG=""
REMOTE_URL=$(git remote get-url origin 2>/dev/null) || true
if [[ -n "$REMOTE_URL" ]]; then
  REPO_SLUG=$(echo "$REMOTE_URL" | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//')
fi

mkdir -p "$WORKSPACES_DIR"
mkdir -p "$AUTOSHIP_DIR/results"

[[ ! -f "$AUTOSHIP_DIR/event-queue.json" ]] && echo '[]' >"$AUTOSHIP_DIR/event-queue.json"
[[ ! -f "$AUTOSHIP_DIR/.pr-monitor-seen.json" ]] && echo '{}' >"$AUTOSHIP_DIR/.pr-monitor-seen.json"
[[ ! -f "$AUTOSHIP_DIR/model-history.json" ]] && echo '{}' >"$AUTOSHIP_DIR/model-history.json"
[[ ! -f "$AUTOSHIP_DIR/quota.json" ]] && bash "$(dirname "$SCRIPT_DIR")/quota-update.sh" init 2>/dev/null || true
[[ ! -f "$AUTOSHIP_DIR/config.json" ]] && echo '{}' >"$AUTOSHIP_DIR/config.json"

# Initialize state.json
if [[ ! -f "$STATE_FILE" ]]; then
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  jq -n \
    --arg repo "$REPO_SLUG" \
    --arg now "$NOW" \
    --arg ver "$AUTOSHIP_VERSION" \
    '{
      version: 1,
      autoship_version: $ver,
      platform: "opencode",
      repo: $repo,
      started_at: $now,
      updated_at: $now,
      paused: false,
      plan: {
        phases: [],
        current_phase: 0,
        checkpoint_pending: false
      },
      issues: {},
      tools: {
        "opencode": {"status": "available", "quota_pct": -1}
      },
      stats: {
        session_dispatched: 0,
        session_completed: 0,
        total_dispatched_all_time: 0,
        total_completed_all_time: 0,
        failed: 0,
        blocked: 0
      },
      config: {
        maxConcurrentAgents: 20
      }
    }' >"$STATE_FILE"
  echo "Initialized $STATE_FILE"
else
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  jq --arg now "$NOW" \
    --arg ver "$AUTOSHIP_VERSION" \
    '.updated_at = $now |
     .stats.session_dispatched = 0 |
     .stats.session_completed = 0 |
     .platform = "opencode" |
     .autoship_version = $ver |
     .config.maxConcurrentAgents = (.config.maxConcurrentAgents // 20)' \
    "$STATE_FILE" >"$STATE_FILE.tmp" && mv "$STATE_FILE.tmp" "$STATE_FILE"
  echo "Refreshed $STATE_FILE"
fi

DETECTED_TOOLS="{}"
if command -v opencode >/dev/null 2>&1; then
  DETECTED_TOOLS=$(jq '.opencode = {"available": true}' <<<"$DETECTED_TOOLS")
fi

jq --argjson tools "$DETECTED_TOOLS" \
  '.tools = {"opencode": {"status": (if $tools.opencode.available == true then "available" else "unavailable" end), "quota_pct": -1}}' \
  "$STATE_FILE" >"$STATE_FILE.tmp" && mv "$STATE_FILE.tmp" "$STATE_FILE"

init_token_ledger() {
  if ! command -v jq >/dev/null 2>&1; then return 0; fi
  local now session_id
  now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  session_id="session-$(date -u +%s)-$$"
  local new_session
  new_session=$(jq -n \
    --arg sid "$session_id" \
    --arg now "$now" \
    --arg repo "$REPO_SLUG" \
    '{session_id: $sid, started_at: $now, repo: $repo, issues: []}')
  if [[ -f "$LEDGER_FILE" ]]; then
    jq --argjson s "$new_session" '.sessions += [$s]' "$LEDGER_FILE" >"$LEDGER_FILE.tmp" && mv "$LEDGER_FILE.tmp" "$LEDGER_FILE"
  else
    jq -n --argjson s "$new_session" '{schema_version: 1, sessions: [$s]}' >"$LEDGER_FILE"
  fi
  echo "Token ledger updated: session $session_id"
}
init_token_ledger

if command -v gh >/dev/null 2>&1 && [[ -n "$REPO_SLUG" ]]; then
  for pair in "autoship:in-progress=FFEB3B" "autoship:blocked=F44336" "autoship:paused=FF9800" "autoship:done=4CAF50"; do
    label="${pair%%=*}"
    color="${pair#*=}"
    gh label create "$label" --repo "$REPO_SLUG" --color "$color" --description "AutoShip" 2>/dev/null || true
  done
fi

load_routing_config() {
  local routing_file="$AUTOSHIP_DIR/routing.json"
  local model_routing_file="$AUTOSHIP_DIR/model-routing.json"
  local autoship_md="AUTOSHIP.md"
  local DEFAULT_ROUTING='{"routing":{"research":["opencode"],"docs":["opencode"],"simple_code":["opencode"],"medium_code":["opencode"],"complex":["opencode"],"mechanical":["opencode"],"ci_fix":["opencode"],"rust_unsafe":["opencode"]},"max_concurrent_agents":20}'
  write_model_routing() {
    if [[ -f "$model_routing_file" ]] && jq -e '(.models // []) | length > 0' "$model_routing_file" >/dev/null 2>&1; then
      return 0
    fi
    rm -f "$model_routing_file"
    if command -v opencode >/dev/null 2>&1 && [[ -x "$SCRIPT_DIR/setup.sh" ]]; then
      AUTOSHIP_MAX_AGENTS="${AUTOSHIP_MAX_AGENTS:-20}" bash "$SCRIPT_DIR/setup.sh" >/dev/null 2>&1 || true
    fi
    if [[ ! -f "$model_routing_file" ]]; then
      jq -n '{defaultFallback: null, models: []}' >"$model_routing_file"
    fi
  }
  write_model_routing
  if [[ ! -f "$autoship_md" ]]; then
    printf '%s\n' "$DEFAULT_ROUTING" >"$routing_file"
    return 0
  fi
  local front_matter
  front_matter=$(awk '/^---$/{if(p){exit}else{p=1;next}} p{print}' "$autoship_md" 2>/dev/null) || front_matter=""
  if [[ -z "$front_matter" ]]; then
    printf '%s\n' "$DEFAULT_ROUTING" >"$routing_file"
    return 0
  fi
  printf '%s\n' "$DEFAULT_ROUTING" >"$routing_file"
}
load_routing_config

# Write the repo hooks directory so shared hooks can resolve sibling scripts.
echo "$REPO_ROOT/hooks" >"$AUTOSHIP_DIR/hooks_dir"

echo "Scanning for stale worktrees..."

echo "AutoShip OpenCode workspace ready at $AUTOSHIP_DIR"
