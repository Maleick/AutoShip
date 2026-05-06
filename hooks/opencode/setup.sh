#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
AUTOSHIP_DIR=".autoship"
ROUTING_FILE="$AUTOSHIP_DIR/model-routing.json"
CONFIG_FILE="$AUTOSHIP_DIR/config.json"
MAX_AGENTS="${AUTOSHIP_MAX_AGENTS:-20}"
SELECTED_MODELS="${AUTOSHIP_MODELS:-}"
REFRESH_MODELS="${AUTOSHIP_REFRESH_MODELS:-0}"
ROLE_OVERRIDES_PROVIDED=0
PLANNER_MODEL="${AUTOSHIP_PLANNER_MODEL:-}"
COORDINATOR_MODEL="${AUTOSHIP_COORDINATOR_MODEL:-}"
ORCHESTRATOR_MODEL="${AUTOSHIP_ORCHESTRATOR_MODEL:-}"
REVIEWER_MODEL="${AUTOSHIP_REVIEWER_MODEL:-}"
LEAD_MODEL="${AUTOSHIP_LEAD_MODEL:-}"
LABELS="${AUTOSHIP_LABELS:-agent:ready}"

NO_TUI=0
POSITIONAL=()

source "$SCRIPT_DIR/model-parser.sh"

usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

AutoShip OpenCode setup wizard.

OPTIONS:
  --no-tui              Run in non-interactive mode (skip prompts)
  --max-agents N        Set max concurrent agents (default: 20)
  --labels LABEL,...   Comma-separated labels to monitor (default: agent:ready)
  --refresh-models     Force refresh model inventory from OpenCode
  --planner-model MODEL Set planner/coordinator/orchestrator/reviewer/lead model (default: live free-first model)
  --orchestrator-model MODEL Set orchestrator model separately (default: same as planner)
  --reviewer-model MODEL Set reviewer model separately (default: same as planner)
  --lead-model MODEL   Set lead model separately (default: same as planner)
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
  AUTOSHIP_PLANNER_MODEL   Planner model (default: live free-first model)
  AUTOSHIP_ORCHESTRATOR_MODEL Orchestrator model (default: same as planner)
  AUTOSHIP_REVIEWER_MODEL  Reviewer model (default: same as planner)
  AUTOSHIP_LEAD_MODEL    Lead model (default: same as planner)
  AUTOSHIP_LABELS          Comma-separated labels (default: agent:ready)
  GH_TOKEN                 GitHub token (for gh auth)
EOF
  exit "${1:-0}"
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --no-tui)
        NO_TUI=1
        shift
        ;;
      --max-agents)
        [[ $# -ge 2 ]] || {
          echo "Error: --max-agents requires a value" >&2
          usage 2
        }
        MAX_AGENTS="$2"
        shift 2
        ;;
      --max-agents=*)
        MAX_AGENTS="${1#*=}"
        shift
        ;;
      --labels)
        [[ $# -ge 2 ]] || {
          echo "Error: --labels requires a value" >&2
          usage 2
        }
        LABELS="$2"
        shift 2
        ;;
      --labels=*)
        LABELS="${1#*=}"
        shift
        ;;
      --refresh-models)
        REFRESH_MODELS=1
        shift
        ;;
      --planner-model)
        [[ $# -ge 2 ]] || {
          echo "Error: --planner-model requires a value" >&2
          usage 2
        }
        ROLE_OVERRIDES_PROVIDED=1
        PLANNER_MODEL="$2"
        COORDINATOR_MODEL="$2"
        ORCHESTRATOR_MODEL="$2"
        REVIEWER_MODEL="$2"
        LEAD_MODEL="$2"
        shift 2
        ;;
      --planner-model=*)
        ROLE_OVERRIDES_PROVIDED=1
        PLANNER_MODEL="${1#*=}"
        COORDINATOR_MODEL="$PLANNER_MODEL"
        ORCHESTRATOR_MODEL="$PLANNER_MODEL"
        REVIEWER_MODEL="$PLANNER_MODEL"
        LEAD_MODEL="$PLANNER_MODEL"
        shift
        ;;
      --lead-model)
        [[ $# -ge 2 ]] || {
          echo "Error: --lead-model requires a value" >&2
          usage 2
        }
        ROLE_OVERRIDES_PROVIDED=1
        LEAD_MODEL="$2"
        shift 2
        ;;
      --lead-model=*)
        ROLE_OVERRIDES_PROVIDED=1
        LEAD_MODEL="${1#*=}"
        shift
        ;;
      --orchestrator-model)
        [[ $# -ge 2 ]] || {
          echo "Error: --orchestrator-model requires a value" >&2
          usage 2
        }
        ROLE_OVERRIDES_PROVIDED=1
        ORCHESTRATOR_MODEL="$2"
        shift 2
        ;;
      --orchestrator-model=*)
        ROLE_OVERRIDES_PROVIDED=1
        ORCHESTRATOR_MODEL="${1#*=}"
        shift
        ;;
      --reviewer-model)
        [[ $# -ge 2 ]] || {
          echo "Error: --reviewer-model requires a value" >&2
          usage 2
        }
        ROLE_OVERRIDES_PROVIDED=1
        REVIEWER_MODEL="$2"
        shift 2
        ;;
      --reviewer-model=*)
        ROLE_OVERRIDES_PROVIDED=1
        REVIEWER_MODEL="${1#*=}"
        shift
        ;;
      --worker-models)
        [[ $# -ge 2 ]] || {
          echo "Error: --worker-models requires a value" >&2
          usage 2
        }
        SELECTED_MODELS="$2"
        shift 2
        ;;
      --worker-models=*)
        SELECTED_MODELS="${1#*=}"
        shift
        ;;
      -h | --help)
        usage 0
        ;;
      --)
        shift
        break
        ;;
      -*)
        echo "Unknown option: $1" >&2
        usage 2
        ;;
      *)
        echo "Error: Unexpected argument: $1" >&2
        usage 2
        ;;
    esac
  done
}

parse_args "$@"

if [[ -n "${AUTOSHIP_PLANNER_MODEL:-}${AUTOSHIP_COORDINATOR_MODEL:-}${AUTOSHIP_ORCHESTRATOR_MODEL:-}${AUTOSHIP_REVIEWER_MODEL:-}${AUTOSHIP_LEAD_MODEL:-}" ]]; then
  ROLE_OVERRIDES_PROVIDED=1
fi

mkdir -p "$AUTOSHIP_DIR"

if [[ "$REFRESH_MODELS" == "1" ]]; then
  rm -f "$ROUTING_FILE" "$CONFIG_FILE"
fi

if [[ -f "$ROUTING_FILE" && -z "$SELECTED_MODELS" && "$REFRESH_MODELS" != "1" && "$ROLE_OVERRIDES_PROVIDED" -eq 0 ]]; then
  if jq -e '(.models // []) | length > 0' "$ROUTING_FILE" >/dev/null 2>&1; then
    if [[ ! -f "$CONFIG_FILE" ]]; then
      jq -n --argjson max "$MAX_AGENTS" --arg labels "$LABELS" \
        '{runtime: "opencode", maxConcurrentAgents: $max, max_agents: $max, models: [], labels: ($labels | split(",")), refreshModels: false, policyProfile: "default", cargoConcurrencyCap: 8, cargoTargetIsolationThreshold: 8, cargoTimeoutSeconds: 120, mergeStrategy: "safe", quotaRouting: true, workerCwdLock: true, truncationSalvage: true, workflowRunnerDefault: ""}' >"$CONFIG_FILE"
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

if ! command -v opencode >/dev/null 2>&1; then
  # Windows detection: if we're in a POSIX shell on Windows but opencode is a PowerShell cmdlet
  if [[ "$OSTYPE" == msys* || "$OSTYPE" == cygwin* || -n "${WSL_DISTRO_NAME:-}" ]] || [[ -f /proc/version && $(cat /proc/version) == *Microsoft* ]]; then
    echo "Error: opencode CLI is not available in this shell environment on Windows." >&2
    echo "" >&2
    echo "AutoShip provides a PowerShell setup script for Windows:" >&2
    echo "  pwsh hooks/opencode/setup.ps1" >&2
    echo "  # or" >&2
    echo "  .\hooks\opencode\setup.ps1" >&2
    echo "" >&2
    echo "For details see: https://github.com/Maleick/AutoShip/blob/main/INSTALL.md#windows-setup" >&2
  else
    echo "Error: opencode is required for AutoShip workers" >&2
  fi
  exit 1
fi

available_models=$(opencode models 2>/dev/null || true)
if [[ -z "$available_models" ]]; then
  echo "Error: unable to list OpenCode models" >&2
  exit 1
fi

available_model_ids=$(normalize_model_ids "$available_models")
if [[ -z "$available_model_ids" ]]; then
  echo "Error: no OpenCode model IDs found in model list" >&2
  exit 1
fi

if [[ -z "$SELECTED_MODELS" ]]; then
  SELECTED_MODELS=$(default_free_models "$available_model_ids")
fi

DEFAULT_ROLE_MODEL=$(default_role_model "$available_model_ids")
PLANNER_MODEL="${PLANNER_MODEL:-$DEFAULT_ROLE_MODEL}"
COORDINATOR_MODEL="${COORDINATOR_MODEL:-$PLANNER_MODEL}"
ORCHESTRATOR_MODEL="${ORCHESTRATOR_MODEL:-$PLANNER_MODEL}"
REVIEWER_MODEL="${REVIEWER_MODEL:-$PLANNER_MODEL}"
LEAD_MODEL="${LEAD_MODEL:-$PLANNER_MODEL}"

if [[ "$NO_TUI" -eq 0 && -t 0 ]]; then
  echo "Running in interactive mode. Use --no-tui for non-interactive."
  printf 'Orchestrator model [%s]: ' "$ORCHESTRATOR_MODEL" >&2
  if IFS= read -r prompted_orchestrator; then
    [[ -n "$prompted_orchestrator" ]] && ORCHESTRATOR_MODEL="$prompted_orchestrator"
  fi
  printf 'Reviewer model [%s]: ' "$REVIEWER_MODEL" >&2
  if IFS= read -r prompted_reviewer; then
    [[ -n "$prompted_reviewer" ]] && REVIEWER_MODEL="$prompted_reviewer"
  fi
fi

reject_forbidden_models "$SELECTED_MODELS,$PLANNER_MODEL,$COORDINATOR_MODEL,$ORCHESTRATOR_MODEL,$REVIEWER_MODEL,$LEAD_MODEL"

if [[ -z "$SELECTED_MODELS" ]]; then
  echo "Error: no free OpenCode models found. Set AUTOSHIP_MODELS to choose models explicitly." >&2
  exit 1
fi

missing_models=$(find_missing_models "$available_model_ids" "$SELECTED_MODELS")
if [[ -n "$missing_models" ]]; then
  echo "Error: selected models are not currently available in this OpenCode instance:" >&2
  printf '%s\n' "$missing_models" >&2
  exit 1
fi

missing_role_models=$(find_missing_models "$available_model_ids" "$PLANNER_MODEL" "$COORDINATOR_MODEL" "$ORCHESTRATOR_MODEL" "$REVIEWER_MODEL" "$LEAD_MODEL")
if [[ -n "$missing_role_models" ]]; then
  echo "Error: planner/coordinator/orchestrator/reviewer/lead models are not currently available in this OpenCode instance:" >&2
  printf '%s\n' "$missing_role_models" >&2
  exit 1
fi

python3 - "$ROUTING_FILE" "$CONFIG_FILE" "$SELECTED_MODELS" "$MAX_AGENTS" "$PLANNER_MODEL" "$COORDINATOR_MODEL" "$ORCHESTRATOR_MODEL" "$REVIEWER_MODEL" "$LEAD_MODEL" "$LABELS" <<'PY'
import json
import sys
import os

routing_path, config_path, selected_models, max_agents, planner_model, coordinator_model, orchestrator_model, reviewer_model, lead_model, labels = sys.argv[1:]
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
        return ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "complex", "rust_unsafe"]
    if any(token in lower for token in ["minimax", "qwen", "glm", "kimi", "mimo"]):
        return ["docs", "simple_code", "medium_code", "mechanical", "ci_fix", "rust_unsafe"]
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
            "lead": lead_model,
        },
        "pools": {
            "default": {
                "description": "Default worker pool for general tasks",
                "models": [e["id"] for e in entries],
            },
            "frontend": {
                "description": "Frontend development tasks",
                "models": [e["id"] for e in entries if "frontend" in e.get("max_task_types", []) or "docs" in e.get("max_task_types", [])],
            },
            "backend": {
                "description": "Backend development tasks",
                "models": [e["id"] for e in entries if "medium_code" in e.get("max_task_types", []) or "complex" in e.get("max_task_types", [])],
            },
            "docs": {
                "description": "Documentation tasks",
                "models": [e["id"] for e in entries if "docs" in e.get("max_task_types", [])],
            },
            "mechanical": {
                "description": "Mechanical/boilerplate tasks",
                "models": [e["id"] for e in entries if "mechanical" in e.get("max_task_types", [])],
            },
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
        "leadModel": lead_model,
        "models": models,
        "labels": labels_list,
        "refreshModels": int(os.environ.get("AUTOSHIP_REFRESH_MODELS", "0")) == 1,
        "policyProfile": "default",
        "cargoConcurrencyCap": 8,
        "cargoTargetIsolationThreshold": 8,
        "cargoTimeoutSeconds": 120,
        "mergeStrategy": "safe",
        "quotaRouting": True,
        "workerCwdLock": True,
        "truncationSalvage": True,
        "workflowRunnerDefault": "",
    }, f, indent=2)
    f.write("\n")
PY

date -u +%Y-%m-%dT%H:%M:%SZ >"$AUTOSHIP_DIR/.onboarded"
echo "AutoShip OpenCode setup complete"
echo "Configured models: $SELECTED_MODELS"
echo "Max agents: $MAX_AGENTS"
echo "Labels: $LABELS"
echo ""
echo "Next steps:"
echo "  opencode-autoship doctor # Diagnose AutoShip installation"
echo "  /autoship-setup          # Re-run setup wizard"
echo "  /autoship                # Start orchestration"
