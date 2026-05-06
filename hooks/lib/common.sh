#!/usr/bin/env bash
# lib/common.sh — Lightweight shared utilities for AutoShip hooks
# Sourced by hooks that need common functionality
# Minimal dependencies: bash 3.2+, jq (optional)
set -euo pipefail

# Only initialize if not already loaded
[[ -n "${_AUTOSHIP_COMMON_LOADED:-}" ]] && return 0
_AUTOSHIP_COMMON_LOADED=1

# =============================================================================
# Path Resolution
# =============================================================================

# Get repository root directory
autoship_repo_root() {
  git rev-parse --show-toplevel 2>/dev/null || {
    echo "Error: not inside a git repository" >&2
    return 1
  }
}

# Get script directory (works when sourced or executed)
autoship_script_dir() {
  local src="${BASH_SOURCE[1]:-$0}"
  cd "$(dirname "$src")" && pwd
}

# =============================================================================
# Configuration
# =============================================================================

# Read configuration value with fallback chain:
# 1. state.json config section
# 2. config.json
# 3. default value
autoship_config_value() {
  local key="$1" default="$2"
  local value="" repo_root
  repo_root="$(autoship_repo_root 2>/dev/null || true)"
  local state_file="$repo_root/.autoship/state.json"
  local config_file="$repo_root/.autoship/config.json"

  if [[ -f "$state_file" ]]; then
    value=$(jq -r --arg key "$key" '.config[$key] // empty' "$state_file" 2>/dev/null || true)
  fi
  if [[ -z "$value" && -f "$config_file" ]]; then
    value=$(jq -r --arg key "$key" '.[$key] // empty' "$config_file" 2>/dev/null || true)
  fi
  printf '%s\n' "${value:-$default}"
}

# Get max concurrent agents (common config value)
autoship_max_agents() {
  local max
  max=$(autoship_config_value "maxConcurrentAgents" "")
  if [[ -z "$max" ]]; then
    max=$(autoship_config_value "max_agents" "")
  fi
  if [[ -z "$max" || ! "$max" =~ ^[0-9]+$ ]]; then
    max=20
  fi
  printf '%s\n' "$max"
}

# =============================================================================
# State Operations
# =============================================================================

# Count running agents by scanning workspace status files
autoship_running_count() {
  local repo_root
  repo_root="$(autoship_repo_root 2>/dev/null || true)"
  local ws_dir="$repo_root/.autoship/workspaces"
  if [[ -d "$ws_dir" ]]; then
    grep -Rsl '^RUNNING$' "$ws_dir"/*/status 2>/dev/null | wc -l | tr -d ' '
  else
    printf '0\n'
  fi
}

# Update state via update-state.sh hook
autoship_state_set() {
  local action="$1" issue_key="$2"
  shift 2
  local repo_root
  repo_root="$(autoship_repo_root 2>/dev/null || true)"
  if [[ -x "$repo_root/hooks/update-state.sh" ]]; then
    bash "$repo_root/hooks/update-state.sh" "$action" "$issue_key" "$@" 2>/dev/null || true
  fi
}

# Capture failure via capture-failure.sh hook
autoship_capture_failure() {
  local category="$1" issue_id="$2"
  shift 2
  local repo_root
  repo_root="$(autoship_repo_root 2>/dev/null || true)"
  if [[ -x "$repo_root/hooks/capture-failure.sh" ]]; then
    bash "$repo_root/hooks/capture-failure.sh" "$category" "$issue_id" "$@" 2>/dev/null || true
  fi
}

# =============================================================================
# Model Selection
# =============================================================================

# Resolve model for a task type
# Usage: autoship_resolve_model <task_type> <issue_num> [override]
# Returns: model ID via stdout
autoship_resolve_model() {
  local task_type="$1" issue_num="$2" override="${3:-}"
  local script_dir repo_root routing_file

  if [[ -n "$override" ]]; then
    printf '%s\n' "$override"
    return 0
  fi

  repo_root="$(autoship_repo_root 2>/dev/null || true)"
  routing_file="$repo_root/config/model-routing.json"

  if [[ -f "$routing_file" ]]; then
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../opencode" && pwd 2>/dev/null || true)"
    if [[ -x "$script_dir/select-model.sh" ]]; then
      bash "$script_dir/select-model.sh" "$task_type" "$issue_num" 2>/dev/null || true
      return 0
    fi
  fi
  printf '%s\n' ""
}

# Resolve role for a task type
# Usage: autoship_resolve_role <task_type>
# Returns: role name via stdout
autoship_resolve_role() {
  case "$1" in
    docs | documentation) printf '%s\n' docs ;;
    review | code_review) printf '%s\n' reviewer ;;
    test | tests | ci_fix) printf '%s\n' tester ;;
    release) printf '%s\n' release ;;
    simplify | refactor) printf '%s\n' simplifier ;;
    plan | planning) printf '%s\n' planner ;;
    lead | orchestration | coordination) printf '%s\n' lead ;;
    *) printf '%s\n' implementer ;;
  esac
}

# =============================================================================
# Utilities
# =============================================================================

# Get current UTC timestamp
autoship_now() {
  date -u +"%Y-%m-%dT%H:%M:%SZ"
}

# Validate issue ID format
autoship_validate_issue_id() {
  [[ "$1" =~ ^(issue-)?[0-9]+$ ]]
}

# Normalize issue ID to issue-N format
autoship_normalize_issue_id() {
  local id="$1"
  local num="${id#issue-}"
  echo "issue-${num}"
}

# Export all functions
export -f autoship_repo_root autoship_script_dir
export -f autoship_config_value autoship_max_agents
export -f autoship_running_count autoship_state_set autoship_capture_failure
export -f autoship_resolve_model autoship_resolve_role
export -f autoship_now autoship_validate_issue_id autoship_normalize_issue_id
