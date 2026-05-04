#!/usr/bin/env bash
# AutoShip model routing integration
# Reads config/model-routing.json and dispatches to appropriate model tier
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
AUTOSHIP_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Load model routing config
ROUTING_CONFIG="${AUTOSHIP_ROOT}/config/model-routing.json"
if [[ ! -f "$ROUTING_CONFIG" ]]; then
  echo "Error: Model routing config not found: $ROUTING_CONFIG" >&2
  exit 1
fi

# Function to get next model from tier
get_model_from_tier() {
  local tier_name="$1"
  local tier_idx=0
  
  # Extract tier from JSON (requires jq)
  if ! command -v jq >/dev/null 2>&1; then
    echo "Error: jq required for model routing" >&2
    echo "kimi-k2.6"  # fallback
    return
  fi
  
  # Find tier index
  tier_idx=$(jq -r ".tiers | map(.name) | index(\"$tier_name\")" "$ROUTING_CONFIG")
  if [[ "$tier_idx" == "null" || -z "$tier_idx" ]]; then
    echo "Error: Tier '$tier_name' not found" >&2
    echo "kimi-k2.6"  # fallback
    return
  fi
  
  # Get models in tier
  local models
  models=$(jq -r ".tiers[$tier_idx].models[].id" "$ROUTING_CONFIG")
  
  # Round-robin: use usage log to determine next model
  local usage_log="${AUTOSHIP_ROOT}/.autoship/usage-log.json"
  local last_used=""
  if [[ -f "$usage_log" ]]; then
    last_used=$(jq -r ".last_model // empty" "$usage_log" 2>/dev/null || true)
  fi
  
  # Find next model in rotation
  local next_model=""
  local found_last=false
  for model in $models; do
    if [[ -z "$last_used" || "$found_last" == true ]]; then
      next_model="$model"
      break
    fi
    if [[ "$model" == "$last_used" ]]; then
      found_last=true
    fi
  done
  
  # If no next found (end of list), wrap to first
  if [[ -z "$next_model" ]]; then
    next_model=$(echo "$models" | head -1)
  fi
  
  echo "$next_model"
}

# Function to check if model is available (not rate limited)
check_model_available() {
  local model="$1"
  # This would check rate limits, quota, etc.
  # For now, assume available
  return 0
}

# Main dispatch function
dispatch_with_routing() {
  local task_type="${1:-code}"
  local complexity="${2:-simple}"
  
  # Determine tier based on task
  local tier="zen_free"
  if [[ "$complexity" == "complex" ]]; then
    tier="go_paid"
  fi
  
  # Try zen_free first
  local model
  model=$(get_model_from_tier "zen_free")
  
  if check_model_available "$model"; then
    echo "$model"
    return 0
  fi
  
  # Fallback to go_paid
  model=$(get_model_from_tier "go_paid")
  if check_model_available "$model"; then
    echo "$model"
    return 0
  fi
  
  # Final fallback to hermes
  echo "kimi-k2.6"
  return 0
}

# Update usage log
update_usage_log() {
  local model="$1"
  local usage_log="${AUTOSHIP_ROOT}/.autoship/usage-log.json"
  
  mkdir -p "$(dirname "$usage_log")"
  
  if [[ ! -f "$usage_log" ]]; then
    echo '{"last_model":"","usage":{}}' > "$usage_log"
  fi
  
  # Update last used model
  jq --arg model "$model" '.last_model = $model' "$usage_log" > "${usage_log}.tmp"
  mv "${usage_log}.tmp" "$usage_log"
}

# Export functions for use by other scripts
export -f get_model_from_tier
export -f check_model_available
export -f dispatch_with_routing
export -f update_usage_log

# If called directly, show current routing
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  echo "=== AutoShip Model Routing ==="
  echo "Config: $ROUTING_CONFIG"
  echo ""
  echo "Free tier models:"
  jq -r '.tiers[] | select(.name=="zen_free") | .models[].name' "$ROUTING_CONFIG"
  echo ""
  echo "Next model for simple task:"
  dispatch_with_routing "code" "simple"
fi
