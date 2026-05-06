#!/usr/bin/env bash
# hooks/opencode/lib/common.sh — Shared utilities for AutoShip shell scripts.
# Sourced by orchestration scripts; carries inline fallback for standalone use.

set -euo pipefail

# ── Repo root detection ──────────────────────────────────────────
autoship_repo_root() {
  git rev-parse --show-toplevel 2>/dev/null || {
    echo "Error: not inside a git repository" >&2
    return 1
  }
}

# ── Script directory detection ───────────────────────────────────
autoship_script_dir() {
  cd "$(dirname "${BASH_SOURCE[1]:-$0}")" && pwd
}

# ── Required command check ──────────────────────────────────────
autoship_require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Error: $cmd is required but not found" >&2
    return 1
  fi
}

# ── Standard paths ──────────────────────────────────────────────
autoship_autoship_dir() { printf '%s/.autoship\n' "$(autoship_repo_root)"; }
autoship_state_file() { printf '%s/.autoship/state.json\n' "$(autoship_repo_root)"; }
autoship_ledger_file() { printf '%s/.autoship/token-ledger.json\n' "$(autoship_repo_root)"; }
autoship_workspaces_dir() { printf '%s/.autoship/workspaces\n' "$(autoship_repo_root)"; }
autoship_event_queue() { printf '%s/.autoship/event-queue.json\n' "$(autoship_repo_root)"; }
autoship_routing_file() { printf '%s/config/model-routing.json\n' "$(autoship_repo_root)"; }
autoship_history_file() { printf '%s/.autoship/model-history.json\n' "$(autoship_repo_root)"; }

# ── Thin state update wrapper ───────────────────────────────────
# Callers should prefer this over hardcoding update-state.sh paths.
autoship_state_set() {
  local action="$1"
  local issue_key="$2"
  shift 2
  local hook_root
  hook_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  bash "$hook_root/update-state.sh" "$action" "$issue_key" "$@"
}

# ── Thin failure capture wrapper ────────────────────────────────
autoship_capture_failure() {
  local category="$1"
  local issue_id="$2"
  shift 2
  local hook_root
  hook_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  bash "$hook_root/capture-failure.sh" "$category" "$issue_id" "$@" 2>/dev/null || true
}

# ── Config value reader ─────────────────────────────────────────
autoship_config_value() {
  local key="$1"
  local default="${2:-}"
  local value=""
  local state_file
  state_file="$(autoship_state_file 2>/dev/null || true)"
  local config_file
  config_file="$(autoship_repo_root 2>/dev/null)/.autoship/config.json" || true
  if [[ -f "$state_file" ]]; then
    value=$(jq -r --arg key "$key" '.config[$key] // empty' "$state_file" 2>/dev/null || true)
  fi
  if [[ -z "$value" && -f "${config_file:-}" ]]; then
    value=$(jq -r --arg key "$key" '.[$key] // empty' "$config_file" 2>/dev/null || true)
  fi
  printf '%s\n' "${value:-$default}"
}

# ── Model resolver wrapper ──────────────────────────────────────
autoship_select_model() {
  bash "$(autoship_script_dir)/select-model.sh" "$@"
}
