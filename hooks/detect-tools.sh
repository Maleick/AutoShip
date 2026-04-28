#!/usr/bin/env bash
set -euo pipefail

# detect-tools.sh — Detect OpenCode first-party model availability.

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
QUOTA_FILE="${REPO_ROOT}/.autoship/quota.json"

probe_command() {
  local name="$1" version_arg="$2"
  local available=false version="" last_error=""
  if command -v "$name" >/dev/null 2>&1; then
    available=true
    version=$("$name" $version_arg 2>/dev/null | head -1 || true)
    if [[ -z "$version" ]]; then
      last_error="probe returned no version output"
      version="unknown"
    fi
  fi
  jq -n --argjson available "$available" --arg version "$version" --arg last_error "$last_error" '{available:$available,version:$version,quota_pct:-1,quota_source:"probe",last_error:$last_error}'
}

opencode_json=$(probe_command opencode --version)
gemini_json=$(probe_command gemini --version)
codex_json=$(probe_command codex --version | jq '. + {requires_bypass_opt_in:true}')

json=$(jq -n --argjson opencode "$opencode_json" --argjson gemini "$gemini_json" --argjson codex "$codex_json" '{opencode:$opencode,gemini:$gemini,codex:$codex}')

echo "$json"

mkdir -p "$REPO_ROOT/.autoship"
echo "$json" | jq '.' > "${QUOTA_FILE}.tmp" 2>/dev/null && mv "${QUOTA_FILE}.tmp" "$QUOTA_FILE" || true
