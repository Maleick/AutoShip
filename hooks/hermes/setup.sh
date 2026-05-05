#!/usr/bin/env bash
# Hermes agent setup — discover Hermes capabilities and write model-routing.json
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUTOSHIP_DIR="$REPO_ROOT/.autoship"

mkdir -p "$AUTOSHIP_DIR"

# Check if Hermes CLI is available
HERMES_AVAILABLE=false
if command -v hermes &>/dev/null; then
  HERMES_AVAILABLE=true
fi

# Check if we're running inside a Hermes session (messaging gateway, cron, etc.)
HERMES_ACTIVE=false
if [[ -n "${HERMES_SESSION_ID:-}" || -n "${HERMES_CWD:-}" || -n "${HERMES_PROVIDER:-}" ]]; then
  HERMES_ACTIVE=true
fi

# Build model routing for Hermes
# Hermes uses providers + models from config.yaml
ROUTING='{
  "runtime": "hermes",
  "available": '$HERMES_AVAILABLE',
  "active_session": '$HERMES_ACTIVE',
  "models": [
    {
      "id": "hermes/default",
      "name": "Default Hermes Model",
      "provider": "inherited",
      "role": "implementer",
      "free": true,
      "capabilities": ["code", "review", "docs"]
    }
  ],
  "providers": [
    "nous",
    "openrouter",
    "openai-codex",
    "kimi-coding",
    "local"
  ],
  "dispatch_method": "cronjob",
  "max_concurrent": 20,
  "notes": "Hermes uses the provider/model configured in ~/.hermes/config.yaml"
}'

echo "$ROUTING" | jq . >"$AUTOSHIP_DIR/hermes-model-routing.json"

echo "Hermes runtime configured:"
echo "  CLI available: $HERMES_AVAILABLE"
echo "  Active session: $HERMES_ACTIVE"
echo "  Max concurrent: 20"
echo "  Routing file: $AUTOSHIP_DIR/hermes-model-routing.json"

# Update main model-routing.json to include Hermes if it exists
if [[ -f "$AUTOSHIP_DIR/model-routing.json" ]]; then
  jq --slurpfile hermes "$AUTOSHIP_DIR/hermes-model-routing.json" '
    .runtimes.hermes = $hermes[0]
  ' "$AUTOSHIP_DIR/model-routing.json" >"$AUTOSHIP_DIR/model-routing.json.tmp" \
    && mv "$AUTOSHIP_DIR/model-routing.json.tmp" "$AUTOSHIP_DIR/model-routing.json"
  echo "  Updated main model-routing.json with Hermes runtime"
fi
