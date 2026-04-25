#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local message="$3"
  if [[ "$expected" != "$actual" ]]; then
    fail "$message: expected '$expected', got '$actual'"
  fi
}

source "$SCRIPT_DIR/model-parser.sh"
source "$SCRIPT_DIR/test-fixtures/mock-opencode-models.sh"

AVAILABLE="opencode/nemotron-3-super-free
opencode/minimax-m2.5-free
openrouter/google/gemma-3-27b-it:free
openai/gpt-5.5
openai/gpt-5.3-codex-spark"
AVAILABLE_IDS=$(normalize_model_ids "$AVAILABLE")

echo "=== Test: free model IDs ==="
DEFAULT_FREE=$(default_free_models "$AVAILABLE_IDS")
DEFAULT_FREE_NL=$(printf '%s\n' "$DEFAULT_FREE" | tr ',' '\n')
assert_eq "3" "$(classify_models "$DEFAULT_FREE_NL" | grep -c ':free' || true)" "default model pool prefers free models"
assert_eq "opencode/nemotron-3-super-free,opencode/minimax-m2.5-free,openrouter/google/gemma-3-27b-it:free" "$DEFAULT_FREE" "default free models are ranked by capability before provider order"

echo "=== Test: explicit selected model IDs ==="
SELECTED_INPUT="openai/gpt-5.5,openai/gpt-5.3-codex-spark"
SELECTED_NL=$(printf '%s\n' "$SELECTED_INPUT" | tr ',' '\n')
assert_eq "2" "$(classify_models "$SELECTED_NL" | grep -c ':selected' || true)" "explicit selected models should be marked selected"

echo "=== Test: unavailable model IDs ==="
MISSING=$(find_missing_models "$AVAILABLE_IDS" "opencode/unavailable-model,openai/gpt-5.5")
assert_eq "opencode/unavailable-model" "$MISSING" "unavailable selected models are reported"

echo "=== Test: forbidden model IDs ==="
if reject_forbidden_models "openai/gpt-5.5-fast" 2>/dev/null; then
  fail "gpt-5.5-fast must be rejected"
fi

echo "=== Test: role selection is separate from worker selection ==="
ROLE_ROUTING="$TMP_DIR/role-routing.json"
cat > "$ROLE_ROUTING" <<'JSON'
{
  "roles": {
    "planner": "openai/gpt-5.5",
    "coordinator": "openai/gpt-5.5",
    "orchestrator": "openai/gpt-5.5",
    "reviewer": "openai/gpt-5.5",
    "lead": "openai/gpt-5.5"
  },
  "pools": {
    "default": {
      "models": ["opencode/nemotron-3-super-free", "opencode/minimax-m2.5-free"]
    }
  },
  "models": [
    {"id": "openai/gpt-5.5", "cost": "selected", "strength": 95},
    {"id": "opencode/nemotron-3-super-free", "cost": "free", "strength": 80},
    {"id": "opencode/minimax-m2.5-free", "cost": "free", "strength": 75}
  ]
}
JSON
ROLE_MODEL=$(jq -r '.roles.planner' "$ROLE_ROUTING")
assert_eq "openai/gpt-5.5" "$ROLE_MODEL" "planner role uses gpt-5.5"

echo "=== Test: select-model.sh --role returns role model ==="
ROLE_REPO="$TMP_DIR/role-test-repo"
mkdir -p "$ROLE_REPO/.autoship"
cp "$ROLE_ROUTING" "$ROLE_REPO/.autoship/model-routing.json"
ROLE_SELECTION="$(cd "$ROLE_REPO" && bash "$SCRIPT_DIR/select-model.sh" --role planner)"
assert_eq "openai/gpt-5.5" "$ROLE_SELECTION" "select-model.sh --role planner returns gpt-5.5"

echo "=== Test: select-model.sh --pool returns worker pool ==="
POOL_SELECTION="$(cd "$ROLE_REPO" && bash "$SCRIPT_DIR/select-model.sh" --pool default | head -1)"
assert_eq "opencode/nemotron-3-super-free" "$POOL_SELECTION" "select-model.sh --pool default returns worker pool"

echo "=== Test: role and worker pool separation ==="
ROLE_IN_roles="$(jq -r '.roles.planner' "$ROLE_ROUTING")"
if [[ "$ROLE_IN_roles" != "openai/gpt-5.5" ]]; then
  fail "planner role should be gpt-5.5, got: $ROLE_IN_roles"
fi
POOL_MODELS="$(jq -r '.pools.default.models[]' "$ROLE_ROUTING")"
if printf '%s\n' "$POOL_MODELS" | grep -qx 'openai/gpt-5.5'; then
  fail "gpt-5.5 must NOT be in default worker pool"
fi

echo "=== Test: setup accepts portable --no-tui flags ==="
MODEL_REPO="$TMP_DIR/model-repo"
mkdir -p "$MODEL_REPO/bin" "$MODEL_REPO/autoship/hooks/opencode"
cp "$SCRIPT_DIR/setup.sh" "$SCRIPT_DIR/model-parser.sh" "$MODEL_REPO/autoship/hooks/opencode/"
chmod +x "$MODEL_REPO/autoship/hooks/opencode/setup.sh"
install_mock_opencode_models_fixture "$MODEL_REPO/bin"
(
  cd "$MODEL_REPO/autoship"
  PATH="$MODEL_REPO/bin:$PATH" bash hooks/opencode/setup.sh --no-tui --max-agents=7 --labels=agent:ready,needs-review --worker-models=openai/gpt-5.3-codex-spark >/dev/null
)
assert_eq "openai/gpt-5.3-codex-spark" "$(jq -r '.models[0].id' "$MODEL_REPO/autoship/.autoship/model-routing.json")" "setup writes explicit selected worker model"
assert_eq "selected" "$(jq -r '.models[0].cost' "$MODEL_REPO/autoship/.autoship/model-routing.json")" "setup classifies explicit worker model as selected"
assert_eq "7" "$(jq -r '.maxConcurrentAgents' "$MODEL_REPO/autoship/.autoship/config.json")" "setup honors portable --max-agents flag"
assert_eq "agent:ready,needs-review" "$(jq -r '.labels | join(",")' "$MODEL_REPO/autoship/.autoship/config.json")" "setup honors portable --labels flag"

echo "=== Test: setup fixture defaults to free models only ==="
FREE_REPO="$TMP_DIR/free-model-repo"
mkdir -p "$FREE_REPO/bin" "$FREE_REPO/autoship/hooks/opencode"
cp "$SCRIPT_DIR/setup.sh" "$SCRIPT_DIR/model-parser.sh" "$FREE_REPO/autoship/hooks/opencode/"
chmod +x "$FREE_REPO/autoship/hooks/opencode/setup.sh"
install_mock_opencode_models_fixture "$FREE_REPO/bin"
(
  cd "$FREE_REPO/autoship"
  PATH="$FREE_REPO/bin:$PATH" bash hooks/opencode/setup.sh --no-tui --planner-model=openai/gpt-5.5 >/dev/null
)
assert_eq "2" "$(jq '[.models[] | select(.cost == "free")] | length' "$FREE_REPO/autoship/.autoship/model-routing.json")" "setup fixture free defaults include only free workers"
assert_eq "opencode/nemotron-3-super-free" "$(jq -r '.defaultFallback' "$FREE_REPO/autoship/.autoship/model-routing.json")" "setup chooses strongest ranked free model as default fallback"
if jq -e '.pools.default.models[] | select(. == "openai/gpt-5.5" or . == "openai/gpt-5.3-codex-spark")' "$FREE_REPO/autoship/.autoship/model-routing.json" >/dev/null; then
  fail "setup default worker pool must not include paid or role models from fixture"
fi

echo "=== Test: setup rejects unavailable and forbidden models ==="
if (cd "$MODEL_REPO/autoship" && PATH="$MODEL_REPO/bin:$PATH" bash hooks/opencode/setup.sh --no-tui --refresh-models --worker-models=missing/model >/dev/null 2>&1); then
  fail "setup should reject unavailable worker models"
fi
if (cd "$MODEL_REPO/autoship" && PATH="$MODEL_REPO/bin:$PATH" bash hooks/opencode/setup.sh --no-tui --refresh-models --planner-model=openai/gpt-5.5-fast >/dev/null 2>&1); then
  fail "setup should reject forbidden planner models"
fi

echo "OpenCode model parsing tests passed"
