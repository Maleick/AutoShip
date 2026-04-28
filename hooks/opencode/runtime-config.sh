#!/usr/bin/env bash
set -euo pipefail

autoship_max_agents() {
  local state_file="${1:?state file required}"
  local autoship_dir="${2:?autoship dir required}"
  local max_agents

  max_agents=$(jq -r '.config.maxConcurrentAgents // .max_concurrent_agents // empty' "$state_file" 2>/dev/null || true)
  if [[ -z "$max_agents" && -f "$autoship_dir/config.json" ]]; then
    max_agents=$(jq -r '.maxConcurrentAgents // .max_agents // empty' "$autoship_dir/config.json" 2>/dev/null || true)
  fi
  printf '%s\n' "${max_agents:-15}"
}
