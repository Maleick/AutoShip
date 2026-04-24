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

ISSUES_FILE="$TMP_DIR/issues.json"
cat > "$ISSUES_FILE" <<'JSON'
[
  {"number": 2301, "title": "new high issue", "body": "safe", "labels": [{"name": "agent:ready"}]},
  {"number": 746, "title": "low safe docs", "body": "update docs", "labels": [{"name": "agent:ready"}, {"name": "documentation"}, {"name": "size-s"}]},
  {"number": 748, "title": "VM fingerprint evasion research", "body": "hide hooks from anti-cheat detection", "labels": [{"name": "agent:ready"}, {"name": "security"}]},
  {"number": 749, "title": "middle safe bug", "body": "fix the setting", "labels": [{"name": "agent:ready"}, {"name": "bug"}]},
  {"number": 750, "title": "already running", "body": "safe", "labels": [{"name": "agent:ready"}, {"name": "agent:running"}]}
]
JSON

PLAN_OUTPUT="$TMP_DIR/plan.json"
bash "$SCRIPT_DIR/plan-issues.sh" --issues-file "$ISSUES_FILE" --limit 10 > "$PLAN_OUTPUT"

eligible_numbers=$(jq -r '.eligible[].number' "$PLAN_OUTPUT" | paste -sd ' ' -)
blocked_numbers=$(jq -r '.blocked[].number' "$PLAN_OUTPUT" | paste -sd ' ' -)
assert_eq "746 749 2301" "$eligible_numbers" "eligible issues are sorted ascending and exclude running/unsafe"
assert_eq "748" "$blocked_numbers" "unsafe issue is blocked"

limited_numbers=$(bash "$SCRIPT_DIR/plan-issues.sh" --issues-file "$ISSUES_FILE" --limit 2 | jq -r '.eligible[].number' | paste -sd ' ' -)
assert_eq "746 749" "$limited_numbers" "plan limit caps eligible queue"

safe_result=$(bash "$SCRIPT_DIR/safety-filter.sh" --text "safe title" "agent:ready,bug" "normal bug fix")
unsafe_result=$(bash "$SCRIPT_DIR/safety-filter.sh" --text "anti-cheat detection bypass" "agent:ready,security" "polymorphic shellcode loader" || true)
unsafe_label_result=$(bash "$SCRIPT_DIR/safety-filter.sh" --text "safe task" "agent:ready,unsafe" "normal maintenance" || true)
assert_eq "SAFE" "$safe_result" "safe issue passes safety filter"
case "$unsafe_result" in
  BLOCKED:*) ;;
  *) fail "unsafe issue should be blocked, got '$unsafe_result'" ;;
esac
case "$unsafe_label_result" in
  BLOCKED:*) ;;
  *) fail "unsafe label should be blocked, got '$unsafe_label_result'" ;;
esac

fix_title=$(bash "$SCRIPT_DIR/pr-title.sh" --issue 2298 --title "Validate Discord webhook URLs" --labels "bug,security,agent:ready")
docs_title=$(bash "$SCRIPT_DIR/pr-title.sh" --issue 2296 --title "mandate poison recovery pattern" --labels "documentation,agent:ready")
assert_eq "fix: Validate Discord webhook URLs (#2298)" "$fix_title" "bug/security title uses fix prefix"
assert_eq "docs: mandate poison recovery pattern (#2296)" "$docs_title" "documentation title uses docs prefix"

STATE_REPO="$TMP_DIR/repo"
mkdir -p "$STATE_REPO/.autoship/workspaces/issue-746" "$STATE_REPO/.autoship/workspaces/issue-749" "$STATE_REPO/.autoship/workspaces/issue-750"
mkdir -p "$STATE_REPO/.autoship/workspaces/issue-751"
cat > "$STATE_REPO/.autoship/state.json" <<'JSON'
{"config":{"maxConcurrentAgents":15},"issues":{"issue-746":{"state":"running"},"issue-749":{"state":"running"},"issue-750":{"state":"running"},"issue-751":{"state":"queued"}},"stats":{}}
JSON
printf 'COMPLETE\n' > "$STATE_REPO/.autoship/workspaces/issue-746/status"
printf 'BLOCKED\n' > "$STATE_REPO/.autoship/workspaces/issue-749/status"
printf 'RUNNING\n' > "$STATE_REPO/.autoship/workspaces/issue-750/status"
printf 'QUEUED\n' > "$STATE_REPO/.autoship/workspaces/issue-751/status"
printf 'changed\n' > "$STATE_REPO/.autoship/workspaces/issue-746/AUTOSHIP_RESULT.md"

bash "$SCRIPT_DIR/reconcile-state.sh" --repo "$STATE_REPO" >/dev/null
assert_eq "completed" "$(jq -r '.issues["issue-746"].state' "$STATE_REPO/.autoship/state.json")" "COMPLETE workspace reconciles to completed"
assert_eq "blocked" "$(jq -r '.issues["issue-749"].state' "$STATE_REPO/.autoship/state.json")" "BLOCKED workspace reconciles to blocked"
assert_eq "running" "$(jq -r '.issues["issue-750"].state' "$STATE_REPO/.autoship/state.json")" "RUNNING workspace remains running"
assert_eq "queued" "$(jq -r '.issues["issue-751"].state' "$STATE_REPO/.autoship/state.json")" "QUEUED workspace remains queued"
assert_eq "1" "$(jq -r '.stats.session_completed' "$STATE_REPO/.autoship/state.json")" "reconcile increments completion stats"
assert_eq "1" "$(jq -r '.stats.blocked' "$STATE_REPO/.autoship/state.json")" "reconcile increments blocked stats"

STATUS_OUTPUT=$(bash "$SCRIPT_DIR/status.sh" --repo "$STATE_REPO")
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'AGENTS (1 active / 15 max)' >/dev/null || fail "status shows active/max concurrency"
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'Queued:    1' >/dev/null || fail "status shows queued count"
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'Completed: 1' >/dev/null || fail "status shows completed count"
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'Blocked:   1' >/dev/null || fail "status shows blocked count"

SETUP_REPO="$TMP_DIR/setup-repo"
mkdir -p "$SETUP_REPO/bin"
cp -R "$SCRIPT_DIR/../.." "$SETUP_REPO/autoship"
cat > "$SETUP_REPO/bin/opencode" <<'SH'
#!/usr/bin/env bash
if [[ "$1" == "models" ]]; then
  printf '%s\n' \
    'opencode/nemotron-3-super-free' \
    'opencode/minimax-m2.5-free' \
    'opencode/gpt-5' \
    'opencode-go/qwen3.6-plus' \
    'openrouter/google/gemma-3-27b-it:free' \
    'openrouter/minimax/minimax-m2.5:free' \
    'zen/some-free-model:free' \
    'openai/gpt-5.5' \
    'openai/gpt-5.5-fast' \
    'openai/gpt-5.3-codex-spark'
  exit 0
fi
echo '1.0.0'
SH
chmod +x "$SETUP_REPO/bin/opencode"
(
  cd "$SETUP_REPO/autoship"
  rm -f .autoship/model-routing.json .autoship/config.json
  PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh >/dev/null
  jq -e '.models | length == 5' .autoship/model-routing.json >/dev/null || fail "setup writes all live free models by default"
  jq -e '.maxConcurrentAgents == 15 and .max_agents == 15' .autoship/config.json >/dev/null || fail "setup writes default concurrency cap consumed by runtime"
  jq -e '.roles.planner == "openai/gpt-5.5" and .roles.coordinator == "openai/gpt-5.5" and .roles.orchestrator == "openai/gpt-5.5"' .autoship/model-routing.json >/dev/null || fail "setup configures GPT-5.5 as planner/coordinator/orchestrator"
  jq -e 'all(.models[]; .cost == "free")' .autoship/model-routing.json >/dev/null || fail "default setup excludes paid worker models"
  jq -e 'all(.models[]; .id != "openai/gpt-5.5")' .autoship/model-routing.json >/dev/null || fail "planner model is not used as a default worker"
  jq -e 'any(.models[]; .id == "openrouter/google/gemma-3-27b-it:free")' .autoship/model-routing.json >/dev/null || fail "setup includes OpenRouter free models from live OpenCode list"
  jq -e 'any(.models[]; .id == "zen/some-free-model:free")' .autoship/model-routing.json >/dev/null || fail "setup includes free models from any live OpenCode provider"
  jq '.models = [{"id":"manual/model","cost":"selected","strength":99,"max_task_types":["docs"]}] | .defaultFallback = "manual/model"' .autoship/model-routing.json > .autoship/model-routing.json.tmp && mv .autoship/model-routing.json.tmp .autoship/model-routing.json
  PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh >/dev/null
  jq -e '.models[0].id == "manual/model"' .autoship/model-routing.json >/dev/null || fail "setup preserves manual model-routing edits by default"
  AUTOSHIP_REFRESH_MODELS=1 PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh >/dev/null
  jq -e '.models | length == 5' .autoship/model-routing.json >/dev/null || fail "setup refreshes generated model routing when requested"
  AUTOSHIP_MODELS='opencode/gpt-5,opencode-go/qwen3.6-plus,openai/gpt-5.3-codex-spark' PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh >/dev/null
  jq -e '.models[0].id == "opencode/gpt-5" and .models[0].cost == "selected" and .models[1].id == "opencode-go/qwen3.6-plus" and .models[2].id == "openai/gpt-5.3-codex-spark"' .autoship/model-routing.json >/dev/null || fail "setup allows explicit selected non-free and Spark models from live list"
  if AUTOSHIP_MODELS='missing/model' PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh >/dev/null 2>&1; then
    fail "setup rejects selected models that are not in the live OpenCode list"
  fi
  if AUTOSHIP_MODELS='openai/gpt-5.5-fast' PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh >/dev/null 2>&1; then
    fail "setup rejects gpt-5.5-fast"
  fi
)

SELECT_REPO="$TMP_DIR/select-repo"
mkdir -p "$SELECT_REPO/.autoship" "$SELECT_REPO/hooks/opencode"
cp "$SCRIPT_DIR/select-model.sh" "$SELECT_REPO/hooks/opencode/select-model.sh"
cat > "$SELECT_REPO/.autoship/model-routing.json" <<'JSON'
{
  "roles": {
    "planner": "openai/gpt-5.5",
    "coordinator": "openai/gpt-5.5",
    "orchestrator": "openai/gpt-5.5",
    "reviewer": "openai/gpt-5.5"
  },
  "models": [
    {"id":"free/strong:free","cost":"free","strength":90,"max_task_types":["simple_code"]},
    {"id":"free/reliable:free","cost":"free","strength":70,"max_task_types":["simple_code"]},
    {"id":"openai/gpt-5.3-codex-spark","cost":"selected","strength":95,"max_task_types":["complex"]},
    {"id":"opencode-go/qwen3.6-plus","cost":"selected","strength":110,"max_task_types":["medium_code"]}
  ]
}
JSON
cat > "$SELECT_REPO/.autoship/model-history.json" <<'JSON'
{
  "free/strong:free": {"success": 0, "fail": 6},
  "free/reliable:free": {"success": 4, "fail": 0}
}
JSON
assert_eq "free/reliable:free" "$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh simple_code 101)" "selector learns from previous run outcomes"
assert_eq "openai/gpt-5.3-codex-spark" "$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh complex 102)" "selector can choose selected Spark model for complex work"
assert_eq "opencode-go/qwen3.6-plus" "$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh medium_code 103)" "selector can choose Go model when best for task"
assert_eq "openai/gpt-5.5" "$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh --role planner)" "selector returns GPT-5.5 planner role"
assert_eq "openai/gpt-5.5" "$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh --role reviewer)" "selector returns GPT-5.5 reviewer role"

UPDATE_REPO="$TMP_DIR/update-repo"
mkdir -p "$UPDATE_REPO/.autoship" "$UPDATE_REPO/bin" "$UPDATE_REPO/hooks"
git init -q "$UPDATE_REPO"
git -C "$UPDATE_REPO" remote add origin git@github.com:owner/repo.git
cp "$SCRIPT_DIR/../update-state.sh" "$UPDATE_REPO/hooks/update-state.sh"
cat > "$UPDATE_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
cat > "$UPDATE_REPO/bin/gh" <<'SH'
#!/usr/bin/env bash
if [[ "$1 $2" == "label list" ]]; then
  printf '%s\n' autoship:in-progress autoship:blocked autoship:paused autoship:done
  exit 0
fi
if [[ "$1 $2" == "issue edit" ]]; then
  printf '%s\n' "$3" >> "$AUTOSHIP_GH_ISSUES_LOG"
  exit 0
fi
exit 0
SH
chmod +x "$UPDATE_REPO/bin/gh" "$UPDATE_REPO/hooks/update-state.sh"
(cd "$UPDATE_REPO" && AUTOSHIP_GH_ISSUES_LOG="$UPDATE_REPO/gh-issues.log" PATH="$UPDATE_REPO/bin:$PATH" bash hooks/update-state.sh set-running issue-123 >/dev/null)
assert_eq "running" "$(jq -r '.issues["issue-123"].state' "$UPDATE_REPO/.autoship/state.json")" "update-state stores normalized issue key"
assert_eq "123" "$(head -1 "$UPDATE_REPO/gh-issues.log")" "update-state passes numeric issue to gh"

echo "OpenCode policy tests passed"
