#!/usr/bin/env bash
set -euo pipefail

COMMAND="${1:-}"
KEY="${2:-}"
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
AUTOSHIP_DIR="$REPO_ROOT/.autoship"
CONFIG_FILE="$AUTOSHIP_DIR/config.json"
POLICY_DIR="$REPO_ROOT/policies"

detect_profile() {
  if [[ -f "$CONFIG_FILE" ]]; then
    local configured
    configured=$(jq -r '.policyProfile // empty' "$CONFIG_FILE" 2>/dev/null || true)
    [[ -n "$configured" ]] && {
      printf '%s\n' "$configured"
      return 0
    }
  fi
  if [[ -f "$REPO_ROOT/Cargo.toml" ]] && [[ -d "$REPO_ROOT/textquest-web" || -d "$REPO_ROOT/textquest-dll" ]]; then
    printf '%s\n' textquest
  else
    printf '%s\n' default
  fi
}

policy_json() {
  local profile
  profile=$(detect_profile)
  local default_file="$POLICY_DIR/default.json"
  local profile_file="$POLICY_DIR/$profile.json"
  if [[ ! -f "$default_file" ]]; then
    jq -n '{profile:"default",cargoConcurrencyCap:8,cargoTargetIsolationThreshold:8,cargoTimeoutSeconds:120,mergeStrategy:"safe",quotaRouting:true,workerCwdLock:true,truncationSalvage:true,workflowRunnerDefault:"",hotStructs:{},knownEnums:[],overlapClusters:[]}'
    return 0
  fi
  if [[ "$profile" == default || ! -f "$profile_file" ]]; then
    jq '.' "$default_file"
    return 0
  fi
  jq -s '.[0] * .[1]' "$default_file" "$profile_file"
}

effective_policy_json() {
  local config_filter='with_entries(select(.key as $key | ["cargoConcurrencyCap", "cargoTargetIsolationThreshold", "cargoTimeoutSeconds", "mergeStrategy", "quotaRouting", "workerCwdLock", "truncationSalvage", "workflowRunnerDefault"] | index($key)))'
  if [[ -f "$CONFIG_FILE" ]]; then
    local config_override
    config_override=$(jq -c "$config_filter" "$CONFIG_FILE" 2>/dev/null || true)
    if [[ -n "$config_override" ]]; then
      policy_json | jq --argjson config "$config_override" '. * $config'
    else
      policy_json
    fi
  else
    policy_json
  fi
}

config_or_policy_value() {
  local key="$1"
  local from_config=""
  if [[ -f "$CONFIG_FILE" ]]; then
    from_config=$(jq -r --arg key "$key" '.[$key] // empty' "$CONFIG_FILE" 2>/dev/null || true)
  fi
  if [[ -n "$from_config" ]]; then
    printf '%s\n' "$from_config"
  else
    effective_policy_json | jq -r --arg key "$key" '.[$key] // empty'
  fi
}

case "$COMMAND" in
  profile) detect_profile ;;
  json) effective_policy_json ;;
  value)
    [[ -n "$KEY" ]] || {
      echo "value key required" >&2
      exit 2
    }
    config_or_policy_value "$KEY"
    ;;
  *)
    echo "Usage: policy.sh profile|json|value KEY" >&2
    exit 2
    ;;
esac
