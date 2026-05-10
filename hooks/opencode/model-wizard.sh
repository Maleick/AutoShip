#!/usr/bin/env bash
# AutoShip Model Selection Wizard
# Interactive setup for model-routing.json and config.json
# Run this when .autoship/ is first initialized or when switching providers
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUTOSHIP_DIR="$REPO_ROOT/.autoship"

mkdir -p "$AUTOSHIP_DIR"

echo "=== AutoShip Model Selection Wizard ==="
echo ""

# Detect available models
KIMI_MODELS=""
FREE_MODELS=""
OPENCODE_GO_MODELS=""

if command -v opencode &>/dev/null; then
  echo "Detecting available models..."
  ALL_MODELS=$(opencode models 2>/dev/null || true)
  KIMI_MODELS=$(echo "$ALL_MODELS" | grep "^kimi-for-coding/" || true)
  FREE_MODELS=$(echo "$ALL_MODELS" | grep "\-free$" || true)
  OPENCODE_GO_MODELS=$(echo "$ALL_MODELS" | grep "^opencode-go/" || true)
else
  echo "Warning: opencode CLI not found. Using static model lists."
fi

KIMI_AVAILABLE=false
FREE_AVAILABLE=false
if [[ -n "$KIMI_MODELS" ]]; then
  KIMI_AVAILABLE=true
  echo "✓ kimi-for-coding provider detected"
  echo "$KIMI_MODELS" | sed 's/^/  /'
fi
if [[ -n "$FREE_MODELS" ]]; then
  FREE_AVAILABLE=true
  echo "✓ Free models detected"
  echo "$FREE_MODELS" | head -5 | sed 's/^/  /'
  [[ $(echo "$FREE_MODELS" | wc -l) -gt 5 ]] && echo "  ... ($(echo "$FREE_MODELS" | wc -l) total)"
fi

echo ""
echo "Select your preferred model strategy:"
echo ""
echo "  1) Kimi-for-coding subscription (recommended)"
echo "     Use kimi-for-coding/k2p6 for all roles and tasks."
echo "     Best for: Reliable PR creation, no cooldown issues."
echo ""
echo "  2) Free models only (OpenCode free tier)"
echo "     Use opencode/*-free models with quota-aware routing."
echo "     Best for: Cost-conscious runs, may hit cooldown."
echo ""
echo "  3) Mixed routing (free first, fallback to kimi)"
echo "     Prefer free models, fallback to kimi-for-coding on exhaustion."
echo "     Best for: Balancing cost and reliability."
echo ""
echo "  4) Custom selection"
echo "     Manually specify role models and worker pools."
echo ""

read -rp "Enter choice [1-4]: " CHOICE

case "$CHOICE" in
  1)
    echo ""
    echo "Configuring kimi-for-coding subscription..."
    cat > "$AUTOSHIP_DIR/model-routing.json" <<'JSON'
{
  "roles": {
    "planner": "kimi-for-coding/k2p6",
    "coordinator": "kimi-for-coding/k2p6",
    "orchestrator": "kimi-for-coding/k2p6",
    "reviewer": "kimi-for-coding/k2p6",
    "lead": "kimi-for-coding/k2p6"
  },
  "pools": {
    "default": {
      "description": "Default worker pool — kimi-for-coding models",
      "models": ["kimi-for-coding/k2p6", "kimi-for-coding/k2p5", "kimi-for-coding/kimi-k2-thinking"]
    },
    "frontend": {
      "description": "Frontend development tasks",
      "models": ["kimi-for-coding/k2p6", "kimi-for-coding/k2p5", "kimi-for-coding/kimi-k2-thinking"]
    },
    "backend": {
      "description": "Backend development tasks",
      "models": ["kimi-for-coding/k2p6", "kimi-for-coding/k2p5", "kimi-for-coding/kimi-k2-thinking"]
    },
    "docs": {
      "description": "Documentation tasks",
      "models": ["kimi-for-coding/k2p6", "kimi-for-coding/k2p5", "kimi-for-coding/kimi-k2-thinking"]
    },
    "mechanical": {
      "description": "Mechanical/boilerplate tasks",
      "models": ["kimi-for-coding/k2p6", "kimi-for-coding/k2p5", "kimi-for-coding/kimi-k2-thinking"]
    }
  },
  "defaultFallback": "kimi-for-coding/k2p5",
  "models": [
    {
      "id": "kimi-for-coding/k2p6",
      "cost": "selected",
      "strength": 95,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe", "research"]
    },
    {
      "id": "kimi-for-coding/k2p5",
      "cost": "selected",
      "strength": 85,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe", "research"]
    },
    {
      "id": "kimi-for-coding/kimi-k2-thinking",
      "cost": "selected",
      "strength": 90,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe", "research"]
    }
  ]
}
JSON
    ;;

  2)
    echo ""
    echo "Configuring free models only..."
    cat > "$AUTOSHIP_DIR/model-routing.json" <<'JSON'
{
  "roles": {
    "planner": "opencode/nemotron-3-super-free",
    "coordinator": "opencode/nemotron-3-super-free",
    "orchestrator": "opencode/nemotron-3-super-free",
    "reviewer": "opencode/nemotron-3-super-free",
    "lead": "opencode/nemotron-3-super-free"
  },
  "pools": {
    "default": {
      "description": "Default worker pool — free models",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "opencode/ring-2.6-1t-free"]
    },
    "frontend": {
      "description": "Frontend development tasks",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "opencode/ring-2.6-1t-free"]
    },
    "backend": {
      "description": "Backend development tasks",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free"]
    },
    "docs": {
      "description": "Documentation tasks",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "opencode/ring-2.6-1t-free"]
    },
    "mechanical": {
      "description": "Mechanical/boilerplate tasks",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "opencode/ring-2.6-1t-free"]
    }
  },
  "defaultFallback": "opencode/nemotron-3-super-free",
  "models": [
    {
      "id": "opencode/nemotron-3-super-free",
      "cost": "free",
      "strength": 80,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe"]
    },
    {
      "id": "opencode/minimax-m2.5-free",
      "cost": "free",
      "strength": 75,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "rust_unsafe"]
    },
    {
      "id": "opencode/ring-2.6-1t-free",
      "cost": "free",
      "strength": 45,
      "max_task_types": ["docs", "simple_code", "mechanical"]
    }
  ]
}
JSON
    ;;

  3)
    echo ""
    echo "Configuring mixed routing (free first, kimi fallback)..."
    cat > "$AUTOSHIP_DIR/model-routing.json" <<'JSON'
{
  "roles": {
    "planner": "kimi-for-coding/k2p6",
    "coordinator": "kimi-for-coding/k2p6",
    "orchestrator": "kimi-for-coding/k2p6",
    "reviewer": "kimi-for-coding/k2p6",
    "lead": "kimi-for-coding/k2p6"
  },
  "pools": {
    "default": {
      "description": "Default worker pool — free first, kimi fallback",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "kimi-for-coding/k2p6", "kimi-for-coding/k2p5"]
    },
    "frontend": {
      "description": "Frontend development tasks",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "kimi-for-coding/k2p6", "kimi-for-coding/k2p5"]
    },
    "backend": {
      "description": "Backend development tasks",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "kimi-for-coding/k2p6"]
    },
    "docs": {
      "description": "Documentation tasks",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "kimi-for-coding/k2p6", "kimi-for-coding/k2p5"]
    },
    "mechanical": {
      "description": "Mechanical/boilerplate tasks",
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "kimi-for-coding/k2p6", "kimi-for-coding/k2p5"]
    }
  },
  "defaultFallback": "opencode/nemotron-3-super-free",
  "models": [
    {
      "id": "kimi-for-coding/k2p6",
      "cost": "selected",
      "strength": 95,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe", "research"]
    },
    {
      "id": "kimi-for-coding/k2p5",
      "cost": "selected",
      "strength": 85,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe", "research"]
    },
    {
      "id": "opencode/nemotron-3-super-free",
      "cost": "free",
      "strength": 80,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe"]
    },
    {
      "id": "opencode/minimax-m2.5-free",
      "cost": "free",
      "strength": 75,
      "max_task_types": ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "rust_unsafe"]
    },
    {
      "id": "opencode/ring-2.6-1t-free",
      "cost": "free",
      "strength": 45,
      "max_task_types": ["docs", "simple_code", "mechanical"]
    }
  ]
}
JSON
    ;;

  4)
    echo ""
    echo "Custom selection not yet implemented. Defaulting to kimi-for-coding."
    exec "$0"
    ;;

  *)
    echo "Invalid choice. Defaulting to kimi-for-coding subscription."
    CHOICE=1
    ;;
esac

# Write config.json with matching role models
ROLE_MODEL="kimi-for-coding/k2p6"
MODELS_LIST='["kimi-for-coding/k2p6", "kimi-for-coding/k2p5", "kimi-for-coding/kimi-k2-thinking"]'

if [[ "$CHOICE" == "2" ]]; then
  ROLE_MODEL="opencode/nemotron-3-super-free"
  MODELS_LIST='["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "opencode/ring-2.6-1t-free"]'
elif [[ "$CHOICE" == "3" ]]; then
  MODELS_LIST='["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free", "kimi-for-coding/k2p6", "kimi-for-coding/k2p5", "opencode/ring-2.6-1t-free"]'
fi

cat > "$AUTOSHIP_DIR/config.json" <<JSON
{
  "runtime": "opencode",
  "maxConcurrentAgents": 20,
  "max_agents": 20,
  "plannerModel": "$ROLE_MODEL",
  "coordinatorModel": "$ROLE_MODEL",
  "orchestratorModel": "$ROLE_MODEL",
  "reviewerModel": "$ROLE_MODEL",
  "leadModel": "$ROLE_MODEL",
  "models": $MODELS_LIST,
  "labels": ["agent:ready"],
  "refreshModels": false,
  "policyProfile": "default",
  "cargoConcurrencyCap": 8,
  "cargoTargetIsolationThreshold": 8,
  "cargoTimeoutSeconds": 120,
  "mergeStrategy": "safe",
  "quotaRouting": true,
  "workerCwdLock": true,
  "truncationSalvage": true,
  "workflowRunnerDefault": ""
}
JSON

echo ""
echo "✓ Model configuration written to:"
echo "  $AUTOSHIP_DIR/model-routing.json"
echo "  $AUTOSHIP_DIR/config.json"
echo ""
echo "Run 'bash hooks/opencode/model-wizard.sh' anytime to reconfigure."
