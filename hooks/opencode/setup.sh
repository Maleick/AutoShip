#!/usr/bin/env bash
set -euo pipefail

AUTOSHIP_DIR=".autoship"
ROUTING_FILE="$AUTOSHIP_DIR/model-routing.json"
CONFIG_FILE="$AUTOSHIP_DIR/config.json"
MAX_AGENTS="${AUTOSHIP_MAX_AGENTS:-15}"
SELECTED_MODELS="${AUTOSHIP_MODELS:-}"
REFRESH_MODELS="${AUTOSHIP_REFRESH_MODELS:-0}"
PLANNER_MODEL="${AUTOSHIP_PLANNER_MODEL:-openai/gpt-5.5}"
COORDINATOR_MODEL="${AUTOSHIP_COORDINATOR_MODEL:-$PLANNER_MODEL}"
ORCHESTRATOR_MODEL="${AUTOSHIP_ORCHESTRATOR_MODEL:-$PLANNER_MODEL}"
REVIEWER_MODEL="${AUTOSHIP_REVIEWER_MODEL:-$PLANNER_MODEL}"
LABELS="${AUTOSHIP_LABELS:-agent:ready}"

NO_TUI=0
POSITIONAL=()

usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

AutoShip OpenCode setup wizard.

OPTIONS:
  --no-tui              Run in non-interactive mode (skip prompts)
  --max-agents N        Set max concurrent agents (default: 15)
  --labels LABEL,...   Comma-separated labels to monitor (default: agent:ready)
  --refresh-models     Force refresh model inventory from OpenCode
  --planner-model MODEL Set planner/coordinator/orchestrator/reviewer model (default: openai/gpt-5.5)
  --worker-models MODELS Comma-separated worker models (default: auto-detect free)
  -h, --help           Show this help message

EXAMPLES:
  # Interactive setup
  $(basename "$0")

  # Non-interactive with defaults
  $(basename "$0") --no-tui

  # Custom configuration
  $(basename "$0") --no-tui --max-agents 10 --labels "agent:ready,needs-work" --refresh-models

ENVIRONMENT VARIABLES:
  AUTOSHIP_MAX_AGENTS       Max concurrent agents
  AUTOSHIP_MODELS          Comma-separated worker models
  AUTOSHIP_REFRESH_MODELS  Set to 1 to force refresh
  AUTOSHIP_PLANNER_MODEL   Planner model (default: openai/gpt-5.5)
  AUTOSHIP_LABELS          Comma-separated labels (default: agent:ready)
  GH_TOKEN                 GitHub token (for gh auth)
EOF
  exit "${1:-0}"
}

parse_args() {
  if [[ $# -eq 0 ]]; then
    return 0
  fi

  if ! command -v getopt >/dev/null 2>&1; then
    echo "Error: getopt not found. Install via 'brew install gnu-getopt' or use environment variables." >&2
    exit 1
  fi

  local opts
  opts=$(getopt -o h -l no-tui,max-agents:,labels:,refresh-models,planner-model:,worker-models:,help -- "$@" 2>&1) || {
    echo "Error: $opts" >&2
    usage 2
  }

  eval set -- "$opts"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --no-tui)
        NO_TUI=1
        shift
        ;;
      --max-agents)
        MAX_AGENTS="$2"
        shift 2
        ;;
      --labels)
        LABELS="$2"
        shift 2
        ;;
      --refresh-models)
        REFRESH_MODELS=1
        shift
        ;;
      --planner-model)
        PLANNER_MODEL="$2"
        COORDINATOR_MODEL="$2"
        ORCHESTRATOR_MODEL="$2"
        REVIEWER_MODEL="$2"
        shift 2
        ;;
      --worker-models)
        SELECTED_MODELS="$2"
        shift 2
        ;;
      -h|--help)
        usage 0
        ;;
      --)
        shift
        break
        ;;
      *)
        echo "Unknown option: $1" >&2
        usage 2
        ;;
    esac
  done

  POSITIONAL=("$@")
}

parse_args "$@"

if [[ ${#POSITIONAL[@]} -gt 0 ]]; then
  echo "Error: Unexpected positional arguments: ${POSITIONAL[*]}" >&2
  usage 2
fi

if [[ "$NO_TUI" -eq 0 && -t 0 ]]; then
  echo "Running in interactive mode. Use --no-tui for non-interactive."
fi

mkdir -p "$AUTOSHIP_DIR"

if [[ "$REFRESH_MODELS" == "1" ]]; then
  rm -f "$ROUTING_FILE" "$CONFIG_FILE"
fi

if [[ -f "$ROUTING_FILE" && -z "$SELECTED_MODELS" && "$REFRESH_MODELS" != "1" ]]; then
  if jq -e '(.models // []) | length > 0' "$ROUTING_FILE" >/dev/null 2>&1; then
    if [[ ! -f "$CONFIG_FILE" ]]; then
      jq -n --argjson max "$MAX_AGENTS" --arg labels "$LABELS" \
        '{runtime: "opencode", maxConcurrentAgents: $max, max_agents: $max, models: [], labels: ($labels | split(",")), refreshModels: false}' > "$CONFIG_FILE"
    fi
    echo "AutoShip OpenCode setup already configured"
    echo "Model routing preserved: $ROUTING_FILE"
    echo "Set --refresh-models to regenerate from current opencode models."
    exit 0
  fi
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "Error: GitHub authentication required. Run 'gh auth login' or set GH_TOKEN." >&2
  exit 1
fi

if [[ -f "$ROUTING_FILE" && -z "$SELECTED_MODELS" && "$REFRESH_MODELS" != "1" ]]; then
  if jq -e '(.models // []) | length > 0' "$ROUTING_FILE" >/dev/null 2>&1; then
    if [[ ! -f "$CONFIG_FILE" ]]; then
      jq -n --argjson max "$MAX_AGENTS" '{runtime: "opencode", maxConcurrentAgents: $max, max_agents: $max, models: []}' > "$CONFIG_FILE"
    fi
    echo "AutoShip OpenCode setup already configured"
    echo "Model routing preserved: $ROUTING_FILE"
    echo "Set AUTOSHIP_REFRESH_MODELS=1 to regenerate from current opencode models."
    exit 0
  fi
fi

if ! command -v opencode >/dev/null 2>&1; then
  echo "Error: opencode is required for AutoShip workers" >&2
  exit 1
fi

available_models=$(opencode models 2>/dev/null || true)
if [[ -z "$available_models" ]]; then
  echo "Error: unable to list OpenCode models" >&2
  exit 1
fi

available_model_ids=$(printf '%s\n' "$available_models" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//' | grep -E '^[a-z0-9._-]+/.+' | sort -u || true)
if [[ -z "$available_model_ids" ]]; then
  echo "Error: no OpenCode model IDs found in model list" >&2
  exit 1
fi

if [[ -z "$SELECTED_MODELS" ]]; then
  SELECTED_MODELS=$(printf '%s\n' "$available_model_ids" | grep -E '(:free$|(^|[-/])free($|[-/]))' | paste -sd ',' -)
fi

if printf '%s\n%s\n%s\n%s\n%s\n' "$SELECTED_MODELS" "$PLANNER_MODEL" "$COORDINATOR_MODEL" "$ORCHESTRATOR_MODEL" "$REVIEWER_MODEL" | grep -Eq '(^|,)openai/gpt-5\.5-fast(,|$)'; then
  echo "Error: openai/gpt-5.5-fast is not allowed for AutoShip. Use openai/gpt-5.5 instead." >&2
  exit 1
fi

if [[ -z "$SELECTED_MODELS" ]]; then
  echo "Error: no free OpenCode models found. Set AUTOSHIP_MODELS to choose models explicitly." >&2
  exit 1
fi

missing_models=$(AVAILABLE_MODEL_IDS="$available_model_ids" python3 - "$SELECTED_MODELS" <<'PY'
import os
import sys
selected = [m.strip() for m in sys.argv[1].split(',') if m.strip()]
available = {line.strip() for line in os.environ.get('AVAILABLE_MODEL_IDS', '').splitlines() if line.strip()}
missing = [m for m in selected if m not in available]
print('\n'.join(missing))
PY
)
if [[ -n "$missing_models" ]]; then
  echo "Error: selected models are not currently available in this OpenCode instance:" >&2
  printf '%s\n' "$missing_models" >&2
  exit 1
fi

missing_role_models=$(AVAILABLE_MODEL_IDS="$available_model_ids" python3 - "$PLANNER_MODEL" "$COORDINATOR_MODEL" "$ORCHESTRATOR_MODEL" "$REVIEWER_MODEL" <<'PY'
import os
import sys
selected = [m.strip() for m in sys.argv[1:] if m.strip()]
available = {line.strip() for line in os.environ.get('AVAILABLE_MODEL_IDS', '').splitlines() if line.strip()}
missing = [m for m in selected if m not in available]
print('\n'.join(dict.fromkeys(missing)))
PY
)
if [[ -n "$missing_role_models" ]]; then
  echo "Error: planner/coordinator/orchestrator models are not currently available in this OpenCode instance:" >&2
  printf '%s\n' "$missing_role_models" >&2
  exit 1
fi

python3 - "$ROUTING_FILE" "$CONFIG_FILE" "$SELECTED_MODELS" "$MAX_AGENTS" "$PLANNER_MODEL" "$COORDINATOR_MODEL" "$ORCHESTRATOR_MODEL" "$REVIEWER_MODEL" "$LABELS" <<'PY'
import json
import sys
import os

routing_path, config_path, selected_models, max_agents, planner_model, coordinator_model, orchestrator_model, reviewer_model, labels = sys.argv[1:]
models = [m.strip() for m in selected_models.split(",") if m.strip()]
labels_list = [l.strip() for l in labels.split(",") if l.strip()]

def strength(model: str) -> int:
    lower = model.lower()
    if ":free" in lower:
        base = 45
    elif "free" in lower:
        base = 45
    else:
        base = 90
    if "nemotron-3-super" in lower:
        return 80
    if "minimax-m2.5" in lower:
        return 75
    if "gpt-oss-120b" in lower:
        return 78
    if "llama-3.3-70b" in lower:
        return 70
    if "gemma-3-27b" in lower or "gemma-4-31b" in lower:
        return 65
    if "ling-2.6" in lower:
        return 60
    if "hy3" in lower:
        return 55
    return base

def task_types(model: str) -> list[str]:
    lower = model.lower()
    if any(token in lower for token in ["nemotron-3-super", "gpt-oss-120b", "llama-3.3-70b"]):
        return ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex"]
    if any(token in lower for token in ["minimax", "qwen", "glm", "kimi", "mimo"]):
        return ["docs", "simple_code", "medium_code", "mechanical", "ci_fix"]
    if any(token in lower for token in ["ling", "gemma", "mistral", "devstral"]):
        return ["docs", "simple_code", "mechanical", "ci_fix"]
    return ["docs", "simple_code", "mechanical"]

entries = []
for model in models:
    entries.append({
        "id": model,
        "cost": "free" if (":free" in model.lower() or "free" in model.lower()) else "selected",
        "strength": strength(model),
        "max_task_types": task_types(model),
    })

default = next((e["id"] for e in entries if e["cost"] == "free"), entries[0]["id"])

with open(routing_path, "w", encoding="utf-8") as f:
    json.dump({
        "roles": {
            "planner": planner_model,
            "coordinator": coordinator_model,
            "orchestrator": orchestrator_model,
            "reviewer": reviewer_model,
        },
        "defaultFallback": default,
        "models": entries,
    }, f, indent=2)
    f.write("\n")

with open(config_path, "w", encoding="utf-8") as f:
    json.dump({
        "runtime": "opencode",
        "maxConcurrentAgents": int(max_agents),
        "max_agents": int(max_agents),
        "plannerModel": planner_model,
        "coordinatorModel": coordinator_model,
        "orchestratorModel": orchestrator_model,
        "reviewerModel": reviewer_model,
        "models": models,
        "labels": labels_list,
        "refreshModels": int(os.environ.get("AUTOSHIP_REFRESH_MODELS", "0")) == 1,
    }, f, indent=2)
    f.write("\n")
PY

date -u +%Y-%m-%dT%H:%M:%SZ > "$AUTOSHIP_DIR/.onboarded"
echo "AutoShip OpenCode setup complete"
echo "Configured models: $SELECTED_MODELS"
echo "Max agents: $MAX_AGENTS"
echo "Labels: $LABELS"
