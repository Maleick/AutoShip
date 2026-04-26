#!/usr/bin/env bash
set -euo pipefail

DIFF_SIZE_LIMIT="${DIFF_SIZE_LIMIT:-50000}"
DIFF_FILE_LIMIT="${DIFF_FILE_LIMIT:-20}"
DIFF_WARN_ONLY="${DIFF_WARN_ONLY:-false}"

readonly DIFF_SIZE_LIMIT
readonly DIFF_FILE_LIMIT

check_diff_size() {
  local worktree_dir="${1:-.}"
  local diff_output
  local diff_size=0
  local diff_files=0

  if [[ ! -d "$worktree_dir" ]]; then
    echo "ERROR: worktree_dir $worktree_dir does not exist" >&2
    return 1
  fi

  cd "$worktree_dir"

  local base_ref="${DIFF_BASE_REF:-}"
  if [[ -z "$base_ref" ]]; then
    for ref in origin/main origin/master HEAD~1 main master; do
      if git rev-parse --verify "$ref" >/dev/null 2>&1; then
        base_ref="$ref"
        break
      fi
    done
  fi

  if [[ -n "$base_ref" ]]; then
    diff_output=$(git diff --stat "$base_ref"...HEAD 2>/dev/null || git diff --stat "$base_ref" HEAD 2>/dev/null || true)
  else
    diff_output=$(git diff --stat 2>/dev/null || true)
  fi

  if [[ -z "$diff_output" ]]; then
    echo "clean: no changes detected"
    return 0
  fi

  if [[ -n "$base_ref" ]]; then
    diff_size=$(git diff "$base_ref"...HEAD 2>/dev/null | wc -c | tr -d ' ' || echo "0")
    diff_files=$(git diff --name-only "$base_ref"...HEAD 2>/dev/null | wc -l | tr -d ' ' || echo "0")
  else
    diff_size=$(git diff 2>/dev/null | wc -c | tr -d ' ' || echo "0")
    diff_files=$(git diff --name-only 2>/dev/null | wc -l | tr -d ' ' || echo "0")
  fi

  local status="ok"
  local exit_code=0

  if [[ $diff_size -gt $DIFF_SIZE_LIMIT ]]; then
    status="oversize: $diff_size bytes (limit: $DIFF_SIZE_LIMIT)"
    exit_code=1
  elif [[ $diff_files -gt $DIFF_FILE_LIMIT ]]; then
    status="toomany_files: $diff_files files (limit: $DIFF_FILE_LIMIT)"
    exit_code=1
  fi

  if [[ "$DIFF_WARN_ONLY" == "true" && $exit_code -ne 0 ]]; then
    echo "WARN: $status" >&2
    return 0
  fi

  if [[ $exit_code -ne 0 ]]; then
    echo "ERROR: $status" >&2
    return 1
  fi

  echo "ok: $diff_files files, $diff_size bytes"
  return 0
}

get_diff_summary() {
  local worktree_dir="${1:-.}"
  cd "$worktree_dir"
  
  local stats
  local has_changes=false
  
  if git diff --quiet 2>/dev/null; then
    stats="no changes"
  else
    stats=$(git diff --stat HEAD 2>/dev/null || git diff --stat 2>/dev/null || echo "error reading diff")
    has_changes=true
  fi
  
  local fileschanged=0
  if [[ "$has_changes" == "true" ]]; then
    fileschanged=$(git diff --name-only 2>/dev/null | wc -l | tr -d ' ' || echo "0")
  fi
  
  jq -n \
    --arg stats "$stats" \
    --argjson files "$fileschanged" \
    '{
      diff_stats: $stats,
      files_changed: $files
    }'
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  COMMAND="${1:-}"
  shift || true

  case "$COMMAND" in
    check)
      check_diff_size "${1:-.}"
      ;;
    summary)
      get_diff_summary "${1:-.}"
      ;;
    *)
      echo "Usage: $0 <command> [worktree-dir]" >&2
      echo "Commands:" >&2
      echo "  check <dir>     - Check diff size; exit non-zero if over limit" >&2
      echo "  summary <dir>  - Get diff summary as JSON" >&2
      exit 1
      ;;
  esac
fi
