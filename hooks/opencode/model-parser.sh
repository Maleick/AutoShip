#!/usr/bin/env bash
set -euo pipefail

normalize_model_ids() {
  printf '%s\n' "$1" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//' | grep -E '^[a-z0-9._-]+/.+' | sort -u || true
}

is_free_model() {
  local model
  model=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
  [[ "$model" == *":free" || "$model" == *"/free"* || "$model" == *"-free"* || "$model" == "opencode/big-pickle" || "$model" == "opencode/gpt-5-nano" ]]
}

is_go_model() {
  local model
  model=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
  [[ "$model" == opencode-go/* ]]
}

default_free_models() {
  local available_model_ids="$1"
  local ranked_models
  local result
  ranked_models=$(mktemp)
  while IFS= read -r model; do
    [[ -z "$model" ]] && continue
    if is_free_model "$model"; then
      printf '%s\t%s\n' "$(free_model_rank "$model")" "$model" >> "$ranked_models"
    fi
  done <<< "$available_model_ids"
  result=$(sort -t $'\t' -k1,1nr -k2,2 "$ranked_models" | cut -f2 | paste -sd ',' -)
  rm -f "$ranked_models"
  printf '%s\n' "$result"
}

default_worker_models() {
  local available_model_ids="$1"
  local free_models
  local go_models
  free_models=$(default_free_models "$available_model_ids")
  go_models=$(printf '%s\n' "$available_model_ids" | grep -E '^opencode-go/' | paste -sd ',' - || true)
  if [[ -n "$free_models" && -n "$go_models" ]]; then
    printf '%s,%s\n' "$free_models" "$go_models"
  elif [[ -n "$free_models" ]]; then
    printf '%s\n' "$free_models"
  else
    printf '%s\n' "$go_models"
  fi
}

default_role_model() {
  local available_model_ids="$1"
  local preferred
  preferred=$(printf '%s\n' "$available_model_ids" | grep -Ei '^opencode-go/(kimi|kimmy).*2\.6' | head -1 || true)
  if [[ -n "$preferred" ]]; then
    printf '%s\n' "$preferred"
    return 0
  fi

  preferred=$(printf '%s\n' "$available_model_ids" | while IFS= read -r model; do
    if is_free_model "$model"; then
      printf '%s\t%s\n' "$(free_model_rank "$model")" "$model"
    fi
  done | sort -t $'\t' -k1,1nr -k2,2 | cut -f2 | head -1)
  if [[ -n "$preferred" ]]; then
    printf '%s\n' "$preferred"
    return 0
  fi

  preferred=$(printf '%s\n' "$available_model_ids" | grep -E '^opencode-go/' | head -1 || true)
  if [[ -n "$preferred" ]]; then
    printf '%s\n' "$preferred"
    return 0
  fi

  preferred=$(printf '%s\n' "$available_model_ids" | grep -Ei 'gpt-5\.5|gpt-5\.3-codex-spark' | head -1 || true)
  if [[ -n "$preferred" ]]; then
    printf '%s\n' "$preferred"
    return 0
  fi

  printf '%s\n' "$available_model_ids" | head -1
}

free_model_rank() {
  local model
  model=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
  local score=40

  case "$model" in
    *nemotron-3-super*) score=95 ;;
    *kimi*k2.6*|*kimi-2.6*) score=94 ;;
    *gpt-oss-120b*) score=92 ;;
    *gpt-5-nano*) score=90 ;;
    *llama-3.3-70b*) score=88 ;;
    *big-pickle*) score=86 ;;
    *minimax-m2.5*) score=84 ;;
    *qwen*|*glm*|*kimi*|*mimo*) score=80 ;;
    *gemma-3-27b*|*gemma-4-31b*) score=72 ;;
    *mistral*|*devstral*) score=68 ;;
    *ling*) score=62 ;;
    *hy3*) score=56 ;;
  esac

  case "$model" in
    opencode/*) score=$((score + 6)) ;;
    openrouter/*) score=$((score + 3)) ;;
  esac

  printf '%s\n' "$score"
}

reject_forbidden_models() {
  local models="$1"
  if printf '%s\n' "$models" | tr ',' '\n' | grep -qx 'openai/gpt-5.5-fast'; then
    echo "Error: openai/gpt-5.5-fast is not allowed for AutoShip. Use openai/gpt-5.5 instead." >&2
    return 1
  fi
}

find_missing_models() {
  local available_model_ids="$1"
  shift
  local requested
  requested=$(printf '%s\n' "$@" | tr ',' '\n' | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//' | grep -v '^$' || true)
  while IFS= read -r model; do
    [[ -z "$model" ]] && continue
    if ! printf '%s\n' "$available_model_ids" | grep -qxF "$model"; then
      printf '%s\n' "$model"
    fi
  done <<< "$requested" | awk '!seen[$0]++'
}

classify_models() {
  local models="$1"
  while IFS= read -r model; do
    [[ -z "$model" ]] && continue
    if is_free_model "$model"; then
      printf '%s:free\n' "$model"
    elif is_go_model "$model"; then
      printf '%s:go\n' "$model"
    else
      printf '%s:selected\n' "$model"
    fi
  done <<< "$models"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <available_models_file> [selected_models]" >&2
    exit 1
  fi

  available_models=$(cat "$1")
  available_ids=$(normalize_model_ids "$available_models")

  echo "=== Available IDs ==="
  printf '%s\n' "$available_ids"
  echo ""

  echo "=== Default Free Models ==="
  default_free_models "$available_ids"
  echo ""

  echo "=== Classification ==="
  classify_models "$available_ids"
fi
