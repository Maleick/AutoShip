#!/usr/bin/env bash
# lib/test-fixtures.sh — Shared test fixture utilities for AutoShip tests
# Source this file in test scripts to reduce boilerplate
set -euo pipefail

# Create a temporary test repository with standard structure
# Usage: create_test_repo <base_dir> <repo_name>
# Returns: path to created repo via echo
create_test_repo() {
  local base_dir="$1"
  local repo_name="$2"
  local repo_path="$base_dir/$repo_name"
  mkdir -p "$repo_path/.autoship/workspaces" "$repo_path/hooks/opencode" "$repo_path/hooks" "$repo_path/bin"
  git init -q "$repo_path"
  echo "$repo_path"
}

# Copy standard hooks into a test repo
# Usage: copy_hooks <repo_path> [hook1] [hook2] ...
copy_hooks() {
  local repo_path="$1"
  shift
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../opencode" && pwd)"

  for hook in "$@"; do
    local src="$script_dir/$hook"
    local dest="$repo_path/hooks/opencode/$hook"
    if [[ -f "$src" ]]; then
      cp "$src" "$dest"
      chmod +x "$dest"
    elif [[ -f "$script_dir/../$hook" ]]; then
      cp "$script_dir/../$hook" "$repo_path/hooks/$hook"
      chmod +x "$repo_path/hooks/$hook"
    fi
  done
}

# Create a mock opencode binary
# Usage: create_mock_opencode <repo_path> [exit_code] [output]
create_mock_opencode() {
  local repo_path="$1"
  local exit_code="${2:-0}"
  local output="${3:-ok}"
  cat >"$repo_path/bin/opencode" <<SH
#!/usr/bin/env bash
printf '%s\n' "$output"
exit $exit_code
SH
  chmod +x "$repo_path/bin/opencode"
}

# Create a state.json file in a test repo
# Usage: create_state <repo_path> '{"issues":...}'
create_state() {
  local repo_path="$1"
  local state_json="$2"
  mkdir -p "$repo_path/.autoship"
  echo "$state_json" >"$repo_path/.autoship/state.json"
}

# Create a workspace status file
# Usage: set_workspace_status <repo_path> <issue-key> <status>
set_workspace_status() {
  local repo_path="$1"
  local issue_key="$2"
  local status="$3"
  mkdir -p "$repo_path/.autoship/workspaces/$issue_key"
  printf '%s\n' "$status" >"$repo_path/.autoship/workspaces/$issue_key/status"
}

# Wait for a condition with polling
# Usage: wait_for <timeout_seconds> <command> [message]
wait_for() {
  local timeout="$1"
  local check_cmd="$2"
  local message="${3:-condition not met}"
  local i
  for ((i = 0; i < timeout; i++)); do
    if eval "$check_cmd" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "Timeout: $message" >&2
  return 1
}

# Create a model-routing.json file
# Usage: create_routing <repo_path> '{"models":...}'
create_routing() {
  local repo_path="$1"
  local routing_json="$2"
  mkdir -p "$repo_path/.autoship"
  echo "$routing_json" >"$repo_path/config/model-routing.json"
}

# Create a config.json file
# Usage: create_config <repo_path> '{"maxConcurrentAgents":...}'
create_config() {
  local repo_path="$1"
  local config_json="$2"
  mkdir -p "$repo_path/.autoship"
  echo "$config_json" >"$repo_path/.autoship/config.json"
}

# Export all functions for use in sourced scripts
export -f create_test_repo copy_hooks create_mock_opencode create_state
export -f set_workspace_status wait_for create_routing create_config
