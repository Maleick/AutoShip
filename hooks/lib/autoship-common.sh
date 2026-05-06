#!/usr/bin/env bash
# lib/autoship-common.sh — Shared utilities for AutoShip shell scripts
# Source this file: source "$(dirname "$0")/../lib/autoship-common.sh"
#
# Provides:
# - Standard variable initialization (REPO_ROOT, SCRIPT_DIR, AUTOSHIP_DIR)
# - Atomic file operations
# - Date/time utilities
# - Issue ID validation and normalization
# - Repository slug parsing
# - JSON safe write operations
# - Temp file management
#
# Requires: bash 3.2+, jq (optional but recommended)
set -euo pipefail

# Fail early if not sourced correctly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  echo "Error: This script must be sourced, not executed directly" >&2
  exit 1
fi

# =============================================================================
# Standard Path Initialization
# =============================================================================

# Initialize SCRIPT_DIR if not already set
if [[ -z "${SCRIPT_DIR:-}" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi

# Initialize REPO_ROOT - prefers git root, falls back to SCRIPT_DIR/../..
init_repo_root() {
  if [[ -z "${REPO_ROOT:-}" ]]; then
    REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
      # Fallback: assume we're in hooks/ or hooks/opencode/
      local parent="$(cd "$SCRIPT_DIR/../.." && pwd 2>/dev/null)"
      if [[ -f "$parent/VERSION" || -d "$parent/.git" ]]; then
        REPO_ROOT="$parent"
      else
        echo "Error: not inside a git repository" >&2
        return 1
      fi
    }
  fi
  export REPO_ROOT
}

# Initialize AUTOSHIP_DIR
init_autoship_dir() {
  if [[ -z "${AUTOSHIP_DIR:-}" ]]; then
    AUTOSHIP_DIR="${REPO_ROOT:-.}/.autoship"
  fi
  export AUTOSHIP_DIR
}

# Initialize STATE_FILE
init_state_file() {
  if [[ -z "${STATE_FILE:-}" ]]; then
    STATE_FILE="${AUTOSHIP_DIR}/state.json"
  fi
  export STATE_FILE
}

# =============================================================================
# Date/Time Utilities
# =============================================================================

# Get current UTC timestamp in ISO 8601 format
utc_now() {
  date -u +"%Y-%m-%dT%H:%M:%SZ"
}

# Convert ISO timestamp to epoch seconds
iso_to_epoch() {
  local iso_date="$1"
  date -j -f "%Y-%m-%dT%H:%M:%SZ" "$iso_date" +%s 2>/dev/null \
    || date -d "$iso_date" +%s 2>/dev/null \
    || echo 0
}

# =============================================================================
# Issue ID Utilities
# =============================================================================

# Validate issue ID format (issue-N or just N)
# Returns 0 if valid, 1 otherwise
validate_issue_id() {
  local id="$1"
  [[ "$id" =~ ^(issue-)?[0-9]+$ ]]
}

# Normalize issue ID to issue-N format
# Usage: normalized=$(normalize_issue_id "123")  # returns "issue-123"
# Usage: normalized=$(normalize_issue_id "issue-456")  # returns "issue-456"
normalize_issue_id() {
  local id="$1"
  local num="${id#issue-}"
  echo "issue-${num}"
}

# Extract issue number from issue ID
# Usage: num=$(extract_issue_number "issue-123")  # returns "123"
extract_issue_number() {
  local id="$1"
  echo "${id#issue-}"
}

# =============================================================================
# Repository Utilities
# =============================================================================

# Parse repo slug from git remote URL
# Usage: slug=$(get_repo_slug)
get_repo_slug() {
  local remote_url
  remote_url=$(git remote get-url origin 2>/dev/null) || return 0
  echo "$remote_url" | sed -E 's#^.+[:/]([^/]+/[^/]+)(\.git)?$#\1#' | sed 's/\.git$//'
}

# =============================================================================
# Atomic File Operations
# =============================================================================

# Atomically write to a JSON file using jq
# Usage: json_atomic_write "$file" "$jq_filter" [jq_args...]
# Example: json_atomic_write "$STATE_FILE" '.foo = $val' --arg val "bar"
json_atomic_write() {
  local file="$1"
  local filter="$2"
  shift 2
  local tmp
  tmp="${file}.tmp.$$"
  if jq "$@" "$filter" "$file" >"$tmp" 2>/dev/null; then
    mv "$tmp" "$file"
    return 0
  else
    rm -f "$tmp"
    return 1
  fi
}

# Atomically write string content to file
# Usage: atomic_write "$file" "$content"
atomic_write() {
  local file="$1"
  local content="$2"
  local tmp
  tmp="${file}.tmp.$$"
  printf '%s' "$content" >"$tmp" && mv "$tmp" "$file"
}

# =============================================================================
# Temp File Management
# =============================================================================

# Track temp files for cleanup
_AUTOSHIP_TMP_FILES=()

# Create a temp file and register it for cleanup
# Usage: tmp=$(autoship_mktemp)
autoship_mktemp() {
  local tmp
  tmp=$(mktemp)
  _AUTOSHIP_TMP_FILES+=("$tmp")
  echo "$tmp"
}

# Create a temp directory and register it for cleanup
# Usage: tmpdir=$(autoship_mkdtemp)
autoship_mkdtemp() {
  local tmp
  tmp=$(mktemp -d)
  _AUTOSHIP_TMP_FILES+=("$tmp")
  echo "$tmp"
}

# Cleanup all registered temp files
cleanup_autoship_tmp() {
  for f in "${_AUTOSHIP_TMP_FILES[@]+"${_AUTOSHIP_TMP_FILES[@]}"}"; do
    rm -rf "$f"
  done
  _AUTOSHIP_TMP_FILES=()
}

# Set up automatic cleanup on exit
# Usage: setup_autoship_cleanup
setup_autoship_cleanup() {
  trap 'cleanup_autoship_tmp' EXIT
}

# =============================================================================
# Lock Utilities
# =============================================================================

# Acquire exclusive file lock
# Usage: with_lock "$lock_file" command [args...]
with_lock() {
  local lock_file="$1"
  shift

  if [[ -L "$lock_file" ]]; then
    echo "Error: refusing symlink lock file: $lock_file" >&2
    return 1
  fi

  if command -v flock >/dev/null 2>&1; then
    (
      exec 200>"$lock_file"
      flock -x 200
      "$@"
    )
  elif command -v lockf >/dev/null 2>&1; then
    lockf -k "$lock_file" "$@"
  else
    "$@"
  fi
}

# =============================================================================
# Validation Utilities
# =============================================================================

# Check if jq is available
require_jq() {
  if ! command -v jq >/dev/null 2>&1; then
    echo "Error: jq is required but not found" >&2
    return 1
  fi
}

# Check if gh is available
require_gh() {
  if ! command -v gh >/dev/null 2>&1; then
    echo "Error: gh CLI is required but not found" >&2
    return 1
  fi
}

# =============================================================================
# State File Operations
# =============================================================================

# Read a value from state.json for a given issue
# Usage: value=$(state_get_issue_field "$issue_id" "field_name" "default_value")
state_get_issue_field() {
  local issue_id="$1"
  local field="$2"
  local default="${3:-}"
  if [[ -f "${STATE_FILE}" ]]; then
    jq -r --arg id "$issue_id" --arg f "$field" --arg d "$default" \
      '.issues[$id][$f] // $d' "${STATE_FILE}" 2>/dev/null || echo "$default"
  else
    echo "$default"
  fi
}

# Check if state file exists, create empty one if not
ensure_state_file() {
  if [[ ! -f "${STATE_FILE}" ]]; then
    mkdir -p "${AUTOSHIP_DIR}"
    echo '{}' >"${STATE_FILE}"
  fi
}

# =============================================================================
# Auto-initialization (optional)
# =============================================================================

# Call this to auto-initialize standard paths
autoship_init() {
  init_repo_root || return 1
  init_autoship_dir
  init_state_file
}

# Export all functions
export -f utc_now iso_to_epoch
export -f validate_issue_id normalize_issue_id extract_issue_number
export -f get_repo_slug
export -f json_atomic_write atomic_write
export -f autoship_mktemp autoship_mkdtemp cleanup_autoship_tmp setup_autoship_cleanup
export -f with_lock
export -f require_jq require_gh
export -f state_get_issue_field ensure_state_file
export -f init_repo_root init_autoship_dir init_state_file autoship_init
