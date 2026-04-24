#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  if [[ -n "${AUTOSHIP_FAILURE_ISSUE:-${AUTOSHIP_ISSUE_ID:-}}" && -x "$SCRIPT_DIR/../capture-failure.sh" ]]; then
    AUTOSHIP_FAILURE_HOOK="hooks/opencode/test-policy.sh" \
      bash "$SCRIPT_DIR/../capture-failure.sh" failed_verification "${AUTOSHIP_FAILURE_ISSUE:-${AUTOSHIP_ISSUE_ID:-}}" "error_summary=$1" 2>/dev/null || true
  fi
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
mkdir -p "$STATE_REPO/.autoship/workspaces/issue-752"
cat > "$STATE_REPO/.autoship/state.json" <<'JSON'
{"config":{"maxConcurrentAgents":15},"issues":{"issue-746":{"state":"running"},"issue-749":{"state":"running"},"issue-750":{"state":"running"},"issue-751":{"state":"queued"}},"stats":{}}
JSON
printf 'COMPLETE\n' > "$STATE_REPO/.autoship/workspaces/issue-746/status"
printf 'BLOCKED\n' > "$STATE_REPO/.autoship/workspaces/issue-749/status"
printf 'RUNNING\n' > "$STATE_REPO/.autoship/workspaces/issue-750/status"
printf 'QUEUED\n' > "$STATE_REPO/.autoship/workspaces/issue-751/status"
printf 'changed\n' > "$STATE_REPO/.autoship/workspaces/issue-746/AUTOSHIP_RESULT.md"
printf 'STUCK\n' > "$STATE_REPO/.autoship/workspaces/issue-752/status"
printf '2026-04-24T00:00:00Z\n' > "$STATE_REPO/.autoship/workspaces/issue-752/started_at"
printf 'stale result from issue-762\n' > "$STATE_REPO/.autoship/workspaces/issue-752/AUTOSHIP_RESULT.md"
touch -t 202604230000 "$STATE_REPO/.autoship/workspaces/issue-752/AUTOSHIP_RESULT.md"
touch -t 202604240000 "$STATE_REPO/.autoship/workspaces/issue-752/started_at"

bash "$SCRIPT_DIR/reconcile-state.sh" --repo "$STATE_REPO" >/dev/null
assert_eq "completed" "$(jq -r '.issues["issue-746"].state' "$STATE_REPO/.autoship/state.json")" "COMPLETE workspace reconciles to completed"
assert_eq "blocked" "$(jq -r '.issues["issue-749"].state' "$STATE_REPO/.autoship/state.json")" "BLOCKED workspace reconciles to blocked"
assert_eq "running" "$(jq -r '.issues["issue-750"].state' "$STATE_REPO/.autoship/state.json")" "RUNNING workspace remains running"
assert_eq "queued" "$(jq -r '.issues["issue-751"].state' "$STATE_REPO/.autoship/state.json")" "QUEUED workspace remains queued"
assert_eq "false" "$(jq -r '.issues["issue-752"].has_result' "$STATE_REPO/.autoship/state.json")" "stale result older than started_at is ignored"
assert_eq "1" "$(jq -r '.stats.session_completed' "$STATE_REPO/.autoship/state.json")" "reconcile increments completion stats"
assert_eq "1" "$(jq -r '.stats.blocked' "$STATE_REPO/.autoship/state.json")" "reconcile increments blocked stats"

STATUS_OUTPUT=$(bash "$SCRIPT_DIR/status.sh" --repo "$STATE_REPO")
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'AGENTS (1 active / 15 max)' >/dev/null || fail "status shows active/max concurrency"
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'Queued:    1' >/dev/null || fail "status shows queued count"
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'Completed: 1' >/dev/null || fail "status shows completed count"
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'Blocked:   1' >/dev/null || fail "status shows blocked count"

NO_RUNNING_REPO="$TMP_DIR/no-running-repo"
mkdir -p "$NO_RUNNING_REPO/.autoship/workspaces/issue-999"
cat > "$NO_RUNNING_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-999":{"state":"stuck"}},"stats":{"failed":1},"config":{"maxConcurrentAgents":15}}
JSON
printf 'STUCK\n' > "$NO_RUNNING_REPO/.autoship/workspaces/issue-999/status"
NO_RUNNING_STATUS=$(bash "$SCRIPT_DIR/status.sh" --repo "$NO_RUNNING_REPO")
printf '%s\n' "$NO_RUNNING_STATUS" | grep -F 'AGENTS (0 active / 15 max)' >/dev/null || fail "status handles zero running workspaces under pipefail"
printf '%s\n' "$NO_RUNNING_STATUS" | grep -F 'STUCK:     1' >/dev/null || fail "status shows stuck workspace when none are running"

RUNNER_REPO="$TMP_DIR/runner-repo"
mkdir -p "$RUNNER_REPO/.autoship/workspaces/issue-996" "$RUNNER_REPO/hooks/opencode" "$RUNNER_REPO/hooks" "$RUNNER_REPO/bin"
git init -q "$RUNNER_REPO"
cp "$SCRIPT_DIR/runner.sh" "$RUNNER_REPO/hooks/opencode/runner.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$RUNNER_REPO/hooks/update-state.sh"
cp "$SCRIPT_DIR/../capture-failure.sh" "$RUNNER_REPO/hooks/capture-failure.sh"
chmod +x "$RUNNER_REPO/hooks/opencode/runner.sh" "$RUNNER_REPO/hooks/update-state.sh" "$RUNNER_REPO/hooks/capture-failure.sh"
cat > "$RUNNER_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-996":{"state":"queued","model":"opencode/test-free","role":"implementer","attempt":2}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf 'QUEUED\n' > "$RUNNER_REPO/.autoship/workspaces/issue-996/status"
printf 'test prompt\n' > "$RUNNER_REPO/.autoship/workspaces/issue-996/AUTOSHIP_PROMPT.md"
printf 'opencode/test-free\n' > "$RUNNER_REPO/.autoship/workspaces/issue-996/model"
cat > "$RUNNER_REPO/bin/opencode" <<'SH'
#!/usr/bin/env bash
if [[ -n "${OPENCODE_RUN_ID:-}" || -n "${OPENCODE_SERVER_PASSWORD:-}" ]]; then
  printf 'ENV_LEAK\n'
  exit 0
fi
printf 'ok\n'
exit 0
SH
chmod +x "$RUNNER_REPO/bin/opencode"
(
  cd "$RUNNER_REPO"
  OPENCODE_RUN_ID=leaked OPENCODE_SERVER_PASSWORD=leaked PATH="$RUNNER_REPO/bin:$PATH" bash hooks/opencode/runner.sh >/dev/null
)
for _ in 1 2 3 4 5; do
  [[ "$(tr -d '[:space:]' < "$RUNNER_REPO/.autoship/workspaces/issue-996/status")" != "RUNNING" ]] && break
  sleep 1
done
assert_eq "STUCK" "$(tr -d '[:space:]' < "$RUNNER_REPO/.autoship/workspaces/issue-996/status")" "runner marks successful worker exit without terminal artifact as stuck"
if grep -F 'ENV_LEAK' "$RUNNER_REPO/.autoship/workspaces/issue-996/AUTOSHIP_RUNNER.log" >/dev/null 2>&1; then
  fail "runner must unset parent OpenCode session environment before nested opencode run"
fi
artifact_count=$(find "$RUNNER_REPO/.autoship/failures" -name '*-issue-996.json' 2>/dev/null | wc -l | tr -d '[:space:]')
assert_eq "1" "$artifact_count" "runner captures a stuck worker failure artifact"
artifact_file=$(find "$RUNNER_REPO/.autoship/failures" -name '*-issue-996.json' | head -1)
jq -e '.issue == "issue-996" and .model == "opencode/test-free" and .role == "implementer" and .workspace != "" and .hook == "hooks/opencode/runner.sh" and .failure_category == "stuck" and (.logs | contains("ok")) and .attempt == 2' "$artifact_file" >/dev/null || fail "failure artifact includes issue, model, workspace, hook, logs, category, role, and attempt"
test -s "$RUNNER_REPO/.autoship/workspaces/issue-996/worker.pid" || fail "runner records worker pid for lifecycle monitoring"

FALLBACK_REPO="$TMP_DIR/fallback-runner-repo"
mkdir -p "$FALLBACK_REPO/.autoship/workspaces/issue-208" "$FALLBACK_REPO/hooks/opencode" "$FALLBACK_REPO/hooks" "$FALLBACK_REPO/bin"
git init -q "$FALLBACK_REPO"
cp "$SCRIPT_DIR/runner.sh" "$FALLBACK_REPO/hooks/opencode/runner.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$FALLBACK_REPO/hooks/update-state.sh"
cp "$SCRIPT_DIR/../capture-failure.sh" "$FALLBACK_REPO/hooks/capture-failure.sh"
chmod +x "$FALLBACK_REPO/hooks/opencode/runner.sh" "$FALLBACK_REPO/hooks/update-state.sh" "$FALLBACK_REPO/hooks/capture-failure.sh"
cat > "$FALLBACK_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-208":{"state":"queued","model":"opencode/paid-model","role":"implementer","attempt":1,"task_type":"medium_code"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
cat > "$FALLBACK_REPO/.autoship/model-routing.json" <<'JSON'
{"models":[{"id":"opencode/paid-model","cost":"selected","strength":100,"max_task_types":["medium_code"]},{"id":"opencode/free-fallback","cost":"free","strength":80,"max_task_types":["medium_code"]}]}
JSON
printf 'QUEUED\n' > "$FALLBACK_REPO/.autoship/workspaces/issue-208/status"
printf 'test prompt\n' > "$FALLBACK_REPO/.autoship/workspaces/issue-208/AUTOSHIP_PROMPT.md"
printf 'opencode/paid-model\n' > "$FALLBACK_REPO/.autoship/workspaces/issue-208/model"
cat > "$FALLBACK_REPO/bin/opencode" <<'SH'
#!/usr/bin/env bash
model=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --model) model="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [[ "$model" == "opencode/paid-model" ]]; then
  printf 'Error: Insufficient balance. Manage your billing here\n' >&2
  exit 1
fi
printf 'COMPLETE\n' > status
printf 'fallback succeeded\n' > AUTOSHIP_RESULT.md
exit 0
SH
chmod +x "$FALLBACK_REPO/bin/opencode"
(
  cd "$FALLBACK_REPO"
  PATH="$FALLBACK_REPO/bin:$PATH" bash hooks/opencode/runner.sh >/dev/null
)
for _ in 1 2 3 4 5; do
  [[ "$(tr -d '[:space:]' < "$FALLBACK_REPO/.autoship/workspaces/issue-208/status")" != "RUNNING" ]] && break
  sleep 1
done
assert_eq "COMPLETE" "$(tr -d '[:space:]' < "$FALLBACK_REPO/.autoship/workspaces/issue-208/status")" "runner retries billing failures with a free fallback model"
assert_eq "opencode/free-fallback" "$(tr -d '[:space:]' < "$FALLBACK_REPO/.autoship/workspaces/issue-208/model")" "runner records fallback model in workspace"
jq -e '."opencode/paid-model".fail == 1 and (."opencode/paid-model".last_error | test("Insufficient balance"))' "$FALLBACK_REPO/.autoship/model-history.json" >/dev/null || fail "runner records paid model billing failure in model history"

MONITOR_REPO="$TMP_DIR/monitor-repo"
mkdir -p "$MONITOR_REPO/.autoship/workspaces/issue-997" "$MONITOR_REPO/hooks/opencode" "$MONITOR_REPO/hooks"
git init -q "$MONITOR_REPO"
cp "$SCRIPT_DIR/monitor-agents.sh" "$MONITOR_REPO/hooks/opencode/monitor-agents.sh"
cp "$SCRIPT_DIR/reconcile-state.sh" "$MONITOR_REPO/hooks/opencode/reconcile-state.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$MONITOR_REPO/hooks/update-state.sh"
chmod +x "$MONITOR_REPO/hooks/opencode/monitor-agents.sh" "$MONITOR_REPO/hooks/opencode/reconcile-state.sh" "$MONITOR_REPO/hooks/update-state.sh"
cat > "$MONITOR_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-997":{"state":"running"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf '[]\n' > "$MONITOR_REPO/.autoship/event-queue.json"
printf 'RUNNING\n' > "$MONITOR_REPO/.autoship/workspaces/issue-997/status"
printf '999999\n' > "$MONITOR_REPO/.autoship/workspaces/issue-997/worker.pid"
(
  cd "$MONITOR_REPO"
  bash hooks/opencode/monitor-agents.sh >/dev/null
)
assert_eq "STUCK" "$(tr -d '[:space:]' < "$MONITOR_REPO/.autoship/workspaces/issue-997/status")" "monitor marks RUNNING workspace stuck when worker pid is no longer live"
assert_eq "stuck" "$(jq -r '.issues["issue-997"].state' "$MONITOR_REPO/.autoship/state.json")" "monitor reconciles stale RUNNING workspace state"

MONITOR_COMPLETE_REPO="$TMP_DIR/monitor-complete-repo"
mkdir -p "$MONITOR_COMPLETE_REPO/.autoship/workspaces/issue-998" "$MONITOR_COMPLETE_REPO/hooks/opencode" "$MONITOR_COMPLETE_REPO/hooks"
git init -q "$MONITOR_COMPLETE_REPO"
git -C "$MONITOR_COMPLETE_REPO" config user.email autoship@example.invalid
git -C "$MONITOR_COMPLETE_REPO" config user.name AutoShip
printf 'base\n' > "$MONITOR_COMPLETE_REPO/README.md"
git -C "$MONITOR_COMPLETE_REPO" add README.md
git -C "$MONITOR_COMPLETE_REPO" commit -q -m initial
cp "$SCRIPT_DIR/monitor-agents.sh" "$MONITOR_COMPLETE_REPO/hooks/opencode/monitor-agents.sh"
cp "$SCRIPT_DIR/reconcile-state.sh" "$MONITOR_COMPLETE_REPO/hooks/opencode/reconcile-state.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$MONITOR_COMPLETE_REPO/hooks/update-state.sh"
chmod +x "$MONITOR_COMPLETE_REPO/hooks/opencode/monitor-agents.sh" "$MONITOR_COMPLETE_REPO/hooks/opencode/reconcile-state.sh" "$MONITOR_COMPLETE_REPO/hooks/update-state.sh"
cat > "$MONITOR_COMPLETE_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-998":{"state":"running"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf '[]\n' > "$MONITOR_COMPLETE_REPO/.autoship/event-queue.json"
printf 'RUNNING\n' > "$MONITOR_COMPLETE_REPO/.autoship/workspaces/issue-998/status"
printf '999999\n' > "$MONITOR_COMPLETE_REPO/.autoship/workspaces/issue-998/worker.pid"
printf '2026-04-24T00:00:00Z\n' > "$MONITOR_COMPLETE_REPO/.autoship/workspaces/issue-998/started_at"
printf 'result\n' > "$MONITOR_COMPLETE_REPO/.autoship/workspaces/issue-998/AUTOSHIP_RESULT.md"
touch -t 202604240001 "$MONITOR_COMPLETE_REPO/.autoship/workspaces/issue-998/AUTOSHIP_RESULT.md"
touch -t 202604240000 "$MONITOR_COMPLETE_REPO/.autoship/workspaces/issue-998/started_at"
(
  cd "$MONITOR_COMPLETE_REPO"
  bash hooks/opencode/monitor-agents.sh >/dev/null
)
assert_eq "COMPLETE" "$(tr -d '[:space:]' < "$MONITOR_COMPLETE_REPO/.autoship/workspaces/issue-998/status")" "monitor marks dead worker complete when fresh result artifact exists"

grep -F 'AUTOSHIP_VERSION="1.5.0-opencode"' "$SCRIPT_DIR/init.sh" >/dev/null 2>&1 && fail "init must not hardcode stale 1.5.0-opencode version"

INIT_REPO="$TMP_DIR/init-repo"
mkdir -p "$INIT_REPO"
git init -q "$INIT_REPO"
git -C "$INIT_REPO" remote add origin https://github.com/owner/repo.git
mkdir -p "$INIT_REPO/.autoship"
cat > "$INIT_REPO/.autoship/state.json" <<'JSON'
{"autoship_version":"old","platform":"opencode","repo":"owner/repo","issues":{},"stats":{"session_dispatched":2,"session_completed":1},"config":{"maxConcurrentAgents":15}}
JSON
(
  cd "$INIT_REPO"
  bash "$SCRIPT_DIR/init.sh" >/dev/null
)
expected_version=$(tr -d '[:space:]' < "$SCRIPT_DIR/../../VERSION")
assert_eq "$expected_version" "$(jq -r '.autoship_version' "$INIT_REPO/.autoship/state.json")" "init refreshes autoship_version from VERSION file"
test -f "$INIT_REPO/.autoship/quota.json" || fail "init creates quota.json using the shared quota-update hook"

WORKTREE_REPO="$TMP_DIR/worktree-repo"
mkdir -p "$WORKTREE_REPO"
git init -q "$WORKTREE_REPO"
git -C "$WORKTREE_REPO" config user.email autoship@example.invalid
git -C "$WORKTREE_REPO" config user.name AutoShip
printf 'base\n' > "$WORKTREE_REPO/README.md"
git -C "$WORKTREE_REPO" add README.md
git -C "$WORKTREE_REPO" commit -q -m initial
mkdir -p "$WORKTREE_REPO/.autoship/workspaces/issue-156"
printf 'stale\n' > "$WORKTREE_REPO/.autoship/workspaces/issue-156/AUTOSHIP_RESULT.md"
(
  cd "$WORKTREE_REPO"
  worktree_output=$(bash "$SCRIPT_DIR/create-worktree.sh" issue-156 autoship/issue-156)
  expected_worktree_path="$(git rev-parse --show-toplevel)/.autoship/workspaces/issue-156"
  assert_eq "$expected_worktree_path" "$worktree_output" "create-worktree prints only the workspace path on stdout"
)
test -d "$WORKTREE_REPO/.autoship/workspaces/issue-156/.git" || test -f "$WORKTREE_REPO/.autoship/workspaces/issue-156/.git" || fail "create-worktree replaces stale existing workspace directory"
test ! -e "$WORKTREE_REPO/.autoship/workspaces/issue-156/AUTOSHIP_RESULT.md" || fail "create-worktree clears stale AutoShip artifacts after recovery"

MERGE_REPO="$TMP_DIR/merge-repo"
mkdir -p "$MERGE_REPO/bin"
git init -q "$MERGE_REPO"
git -C "$MERGE_REPO" config user.email autoship@example.invalid
git -C "$MERGE_REPO" config user.name AutoShip
printf 'base\n' > "$MERGE_REPO/README.md"
git -C "$MERGE_REPO" add README.md
git -C "$MERGE_REPO" commit -q -m initial
mkdir -p "$MERGE_REPO/hooks/opencode" "$MERGE_REPO/.autoship/workspaces"
cp "$SCRIPT_DIR/cleanup-worktree.sh" "$MERGE_REPO/hooks/opencode/cleanup-worktree.sh"
cp "$SCRIPT_DIR/merge-pr.sh" "$MERGE_REPO/hooks/opencode/merge-pr.sh"
chmod +x "$MERGE_REPO/hooks/opencode/cleanup-worktree.sh" "$MERGE_REPO/hooks/opencode/merge-pr.sh"
cat > "$MERGE_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-210":{"state":"completed"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
git -C "$MERGE_REPO" branch autoship/issue-210
git -C "$MERGE_REPO" worktree add -q "$MERGE_REPO/.autoship/workspaces/issue-210" autoship/issue-210
printf 'result\n' > "$MERGE_REPO/.autoship/workspaces/issue-210/AUTOSHIP_RESULT.md"
cat > "$MERGE_REPO/bin/gh" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_ARGS_LOG"
exit 0
SH
chmod +x "$MERGE_REPO/bin/gh"
(
  cd "$MERGE_REPO"
  GH_ARGS_LOG="$MERGE_REPO/gh-args.log" PATH="$MERGE_REPO/bin:$PATH" bash hooks/opencode/merge-pr.sh 210 issue-210 >/dev/null
)
test ! -d "$MERGE_REPO/.autoship/workspaces/issue-210" || fail "merge cleanup removes issue worktree before branch deletion"
if git -C "$MERGE_REPO" show-ref --verify --quiet refs/heads/autoship/issue-210; then
  fail "merge cleanup deletes the local issue branch"
fi
if grep -F -- '--delete-branch' "$MERGE_REPO/gh-args.log" >/dev/null 2>&1; then
  fail "merge cleanup must not ask gh to delete a branch that is checked out by an issue worktree"
fi

REPORT_REPO="$TMP_DIR/report-repo"
mkdir -p "$REPORT_REPO/.autoship/failures" "$REPORT_REPO/.autoship/reports" "$REPORT_REPO/hooks/opencode"
git init -q "$REPORT_REPO"
cp "$SCRIPT_DIR/self-improvement-report.sh" "$REPORT_REPO/hooks/opencode/self-improvement-report.sh"
chmod +x "$REPORT_REPO/hooks/opencode/self-improvement-report.sh"
cat > "$REPORT_REPO/.autoship/failures/20260424T010000Z-issue-101.json" <<'JSON'
{
  "issue": "issue-101",
  "failure_category": "model_failure",
  "model": "opencode/paid-model",
  "workspace": "/tmp/workspaces/issue-101",
  "hook": "hooks/opencode/runner.sh",
  "logs": "Error: Insufficient balance. Manage your billing here",
  "error_summary": "Insufficient balance",
  "timestamp": "2026-04-24T01:00:00Z"
}
JSON
cat > "$REPORT_REPO/.autoship/failures/20260424T020000Z-issue-102.json" <<'JSON'
{
  "issue": "issue-102",
  "failure_category": "model_failure",
  "model": "opencode/paid-model",
  "workspace": "/tmp/workspaces/issue-102",
  "hook": "hooks/opencode/runner.sh",
  "logs": "Error: Insufficient balance. Manage your billing here",
  "error_summary": "Insufficient balance",
  "timestamp": "2026-04-24T02:00:00Z"
}
JSON
REPORT_OUTPUT="$REPORT_REPO/.autoship/reports/self-improvement.md"
(
  cd "$REPORT_REPO"
  bash hooks/opencode/self-improvement-report.sh > "$REPORT_OUTPUT"
)
grep -F '## Root Cause Evidence' "$REPORT_OUTPUT" >/dev/null || fail "self-improvement report includes root cause evidence section"
grep -F 'model_failure' "$REPORT_OUTPUT" >/dev/null || fail "self-improvement report includes recurring failure category"
grep -F 'Insufficient balance' "$REPORT_OUTPUT" >/dev/null || fail "self-improvement report includes log-backed root cause evidence"
grep -F 'hooks/opencode/runner.sh' "$REPORT_OUTPUT" >/dev/null || fail "self-improvement report includes affected files"
grep -F '## Candidate Acceptance Criteria' "$REPORT_OUTPUT" >/dev/null || fail "self-improvement report includes candidate acceptance criteria"
grep -F 'paid model balance failures fall back to a configured free model' "$REPORT_OUTPUT" >/dev/null || fail "self-improvement report proposes evidence-backed acceptance criteria"

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
  setup_output=$(PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh)
  test -f .autoship/.onboarded || fail "setup writes onboarding marker"
  printf '%s\n' "$setup_output" | grep -F 'opencode-autoship doctor' >/dev/null || fail "setup prints doctor next step"
  printf '%s\n' "$setup_output" | grep -F '/autoship-setup' >/dev/null || fail "setup prints setup next step"
  printf '%s\n' "$setup_output" | grep -F '/autoship' >/dev/null || fail "setup prints autoship next step"
  jq -e '.models | length == 5' .autoship/model-routing.json >/dev/null || fail "setup writes all live free models by default"
  jq -e '.maxConcurrentAgents == 15 and .max_agents == 15' .autoship/config.json >/dev/null || fail "setup writes default concurrency cap consumed by runtime"
  jq -e '.roles.planner == "openai/gpt-5.5" and .roles.coordinator == "openai/gpt-5.5" and .roles.orchestrator == "openai/gpt-5.5" and .roles.lead == "openai/gpt-5.5"' .autoship/model-routing.json >/dev/null || fail "setup configures GPT-5.5 as planner/coordinator/orchestrator/lead"
  jq -e '.pools != null and .pools.default != null and .pools.frontend != null and .pools.backend != null and .pools.docs != null' .autoship/model-routing.json >/dev/null || fail "setup writes worker pools"
  jq -e 'all(.models[]; .cost == "free")' .autoship/model-routing.json >/dev/null || fail "default setup excludes paid worker models"
  jq -e 'all(.models[]; .id != "openai/gpt-5.5")' .autoship/model-routing.json >/dev/null || fail "planner model is not used as a default worker"
  jq -e 'any(.models[]; .id == "openrouter/google/gemma-3-27b-it:free")' .autoship/model-routing.json >/dev/null || fail "setup includes OpenRouter free models from live OpenCode list"
  jq -e 'any(.models[]; .id == "zen/some-free-model:free")' .autoship/model-routing.json >/dev/null || fail "setup includes free models from any live OpenCode provider"
  jq '.models = [{"id":"manual/model","cost":"selected","strength":99,"max_task_types":["docs"]}] | .defaultFallback = "manual/model"' .autoship/model-routing.json > .autoship/model-routing.json.tmp && mv .autoship/model-routing.json.tmp .autoship/model-routing.json
  PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh >/dev/null
  jq -e '.models[0].id == "manual/model"' .autoship/model-routing.json >/dev/null || fail "setup preserves manual model-routing edits by default"
  PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh --no-tui --max-agents=9 >/dev/null
  jq -e '.models[0].id == "manual/model"' .autoship/model-routing.json >/dev/null || fail "noninteractive setup preserves manual model-routing edits by default"
  PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh --no-tui --refresh-models >/dev/null
  jq -e '.models | length == 5' .autoship/model-routing.json >/dev/null || fail "setup --refresh-models regenerates manual model routing when explicitly requested"
  jq '.models = [{"id":"manual/model","cost":"selected","strength":99,"max_task_types":["docs"]}] | .defaultFallback = "manual/model"' .autoship/model-routing.json > .autoship/model-routing.json.tmp && mv .autoship/model-routing.json.tmp .autoship/model-routing.json
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
  AUTOSHIP_LEAD_MODEL=openai/gpt-5.5 PATH="$SETUP_REPO/bin:$PATH" bash hooks/opencode/setup.sh >/dev/null
  jq -e '.roles.lead == "openai/gpt-5.5"' .autoship/model-routing.json >/dev/null || fail "setup accepts lead model override via AUTOSHIP_LEAD_MODEL"
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
    "reviewer": "openai/gpt-5.5",
    "lead": "openai/gpt-5.5"
  },
  "pools": {
    "default": {"description": "Default pool", "models": ["free/strong:free", "free/reliable:free"]},
    "frontend": {"description": "Frontend", "models": ["free/strong:free"]},
    "backend": {"description": "Backend", "models": ["free/reliable:free"]}
  },
  "models": [
    {"id":"free/strong:free","cost":"free","strength":90,"max_task_types":["simple_code"]},
    {"id":"free/reliable:free","cost":"free","strength":70,"max_task_types":["simple_code"]},
    {"id":"openai/gpt-5.3-codex-spark","cost":"selected","strength":95,"max_task_types":["complex"]},
    {"id":"opencode-go/qwen3.6-plus","cost":"selected","strength":110,"max_task_types":["medium_code"]}
  ]
}
JSON
assert_eq "free/strong:free" "$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh simple_code 101)" "selector treats missing model history as empty"
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
assert_eq "openai/gpt-5.5" "$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh --role lead)" "selector returns GPT-5.5 lead role"
POOL_MODELS=$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh --pool default)
assert_eq "true" "$(echo "$POOL_MODELS" | grep -q "free/strong:free" && echo "true" || echo "false")" "selector --pool default returns pool models"
POOL_MODELS=$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh --pool frontend)
assert_eq "true" "$(echo "$POOL_MODELS" | grep -q "free/strong:free" && echo "true" || echo "false")" "selector --pool frontend returns frontend pool models"

ROUTING_LOG=$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh --log simple_code 101)
assert_eq "true" "$(echo "$ROUTING_LOG" | grep -q "routing_log:" && echo "true" || echo "false")" "selector --log outputs routing log"
assert_eq "true" "$(echo "$ROUTING_LOG" | grep -q "selection: free/strong:free" && echo "true" || echo "false")" "routing log shows free model selection"
assert_eq "true" "$(echo "$ROUTING_LOG" | grep -q "score:" && echo "true" || echo "false")" "routing log shows score"
assert_eq "true" "$(echo "$ROUTING_LOG" | grep -q "reason:" && echo "true" || echo "false")" "routing log shows reason"
assert_eq "true" "$(echo "$ROUTING_LOG" | grep -q "final_selection: free/reliable:free" && echo "true" || echo "false")" "routing log shows final selection"

cat > "$SELECT_REPO/.autoship/model-history.json" <<'JSON'
{
  "free/strong:free": {"success": 0, "fail": 6},
  "free/reliable:free": {"success": 4, "fail": 0}
}
JSON

ROUTING_LOG_ESCALATE=$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh --log simple_code 101)
assert_eq "true" "$(echo "$ROUTING_LOG_ESCALATE" | grep -q "free model selected by default" && echo "true" || echo "false")" "routing log shows free selection reason"

cat > "$SELECT_REPO/.autoship/model-routing.json" <<'JSON'
{
  "roles": {
    "planner": "openai/gpt-5.5",
    "coordinator": "openai/gpt-5.5",
    "orchestrator": "openai/gpt-5.5",
    "reviewer": "openai/gpt-5.5",
    "lead": "openai/gpt-5.5"
  },
  "pools": {
    "default": {"description": "Default pool", "models": ["free/strong:free", "free/reliable:free"]},
    "frontend": {"description": "Frontend", "models": ["free/strong:free"]},
    "backend": {"description": "Backend", "models": ["free/reliable:free"]}
  },
  "models": [
    {"id":"free/strong:free","cost":"free","strength":90,"max_task_types":["simple_code"]},
    {"id":"free/reliable:free","cost":"free","strength":70,"max_task_types":["simple_code"]},
    {"id":"openai/gpt-5.3-codex-spark","cost":"selected","strength":95,"max_task_types":["complex"]},
    {"id":"opencode-go/qwen3.6-plus","cost":"selected","strength":110,"max_task_types":["medium_code"]}
  ]
}
JSON

ROUTING_LOG_COMPLEX=$(cd "$SELECT_REPO" && bash hooks/opencode/select-model.sh --log complex 102)
assert_eq "true" "$(echo "$ROUTING_LOG_COMPLEX" | grep -q "final_selection: openai/gpt-5.3-codex-spark" && echo "true" || echo "false")" "routing log shows Spark for complex task"
assert_eq "true" "$(echo "$ROUTING_LOG_COMPLEX" | grep -q "Spark model selected for complex task" && echo "true" || echo "false")" "routing log shows escalation reason for Spark"

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

DISPATCH_REPO="$TMP_DIR/dispatch-repo"
mkdir -p "$DISPATCH_REPO/.autoship" "$DISPATCH_REPO/hooks/opencode" "$DISPATCH_REPO/hooks" "$DISPATCH_REPO/bin"
git init -q "$DISPATCH_REPO"
git -C "$DISPATCH_REPO" config user.email autoship@example.invalid
git -C "$DISPATCH_REPO" config user.name AutoShip
git -C "$DISPATCH_REPO" remote add origin git@github.com:owner/repo.git
printf 'base\n' > "$DISPATCH_REPO/README.md"
git -C "$DISPATCH_REPO" add README.md
git -C "$DISPATCH_REPO" commit -q -m initial
cp "$SCRIPT_DIR/dispatch.sh" "$SCRIPT_DIR/create-worktree.sh" "$SCRIPT_DIR/select-model.sh" "$SCRIPT_DIR/safety-filter.sh" "$SCRIPT_DIR/pr-title.sh" "$DISPATCH_REPO/hooks/opencode/"
cp "$SCRIPT_DIR/../update-state.sh" "$DISPATCH_REPO/hooks/update-state.sh"
cat > "$DISPATCH_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
cat > "$DISPATCH_REPO/.autoship/model-routing.json" <<'JSON'
{"models":[{"id":"free/strong:free","cost":"free","strength":90,"max_task_types":["docs","medium_code"]}]}
JSON
cat > "$DISPATCH_REPO/bin/gh" <<'SH'
#!/usr/bin/env bash
if [[ "$1 $2" == "issue view" ]]; then
  case "$4" in
    title) printf 'Issue title\n' ;;
    body) printf 'Issue body\n' ;;
    labels) printf 'agent:ready\n' ;;
  esac
  exit 0
fi
if [[ "$1 $2" == "label list" ]]; then
  printf '%s\n' autoship:in-progress autoship:blocked autoship:paused autoship:done
  exit 0
fi
if [[ "$1 $2" == "issue edit" ]]; then
  exit 0
fi
exit 0
SH
chmod +x "$DISPATCH_REPO/bin/gh" "$DISPATCH_REPO/hooks/opencode/dispatch.sh" "$DISPATCH_REPO/hooks/opencode/create-worktree.sh" "$DISPATCH_REPO/hooks/opencode/select-model.sh" "$DISPATCH_REPO/hooks/opencode/safety-filter.sh" "$DISPATCH_REPO/hooks/opencode/pr-title.sh" "$DISPATCH_REPO/hooks/update-state.sh"
(
  cd "$DISPATCH_REPO"
  PATH="$DISPATCH_REPO/bin:$PATH" bash hooks/opencode/dispatch.sh 456 docs >/dev/null
)
assert_eq "docs" "$(cat "$DISPATCH_REPO/.autoship/workspaces/issue-456/role")" "dispatch records specialized role file"
assert_eq "docs" "$(jq -r '.issues["issue-456"].role' "$DISPATCH_REPO/.autoship/state.json")" "dispatch records specialized role in state"
assert_eq "free/strong:free" "$(jq -r '.issues["issue-456"].model' "$DISPATCH_REPO/.autoship/state.json")" "dispatch records selected model in state"
grep -F '## Specialized Role' "$DISPATCH_REPO/.autoship/workspaces/issue-456/AUTOSHIP_PROMPT.md" >/dev/null || fail "dispatch records specialized role in prompt"

PACKAGE_REPO="$TMP_DIR/package-repo"
cp -R "$SCRIPT_DIR/../.." "$PACKAGE_REPO"
(
  cd "$PACKAGE_REPO"
  rm -rf .autoship node_modules dist
  npm install --package-lock=false --no-audit --no-fund >/dev/null
  npm run build >/dev/null
  CONFIG_DIR="$TMP_DIR/package-config"
  mkdir -p "$CONFIG_DIR"
  printf '%s\n' '{"plugin":["file:///tmp/legacy/autoship.ts","other-plugin"],"customSetting":true}' > "$CONFIG_DIR/opencode.json"
  OPENCODE_CONFIG_DIR="$CONFIG_DIR" node dist/cli.js install >/dev/null
  jq -e '.plugin | index("opencode-autoship")' "$CONFIG_DIR/opencode.json" >/dev/null || fail "package installer registers opencode-autoship plugin"
  jq -e '.plugin | index("other-plugin")' "$CONFIG_DIR/opencode.json" >/dev/null || fail "package installer preserves unrelated plugins"
  jq -e '.customSetting == true' "$CONFIG_DIR/opencode.json" >/dev/null || fail "package installer preserves unrelated config"
  if jq -e '.plugin[] | select(type == "string" and contains("autoship.ts"))' "$CONFIG_DIR/opencode.json" >/dev/null; then
    fail "package installer removes legacy autoship.ts plugin entries"
  fi
  test -d "$CONFIG_DIR/.autoship/hooks" || fail "package installer copies hooks"
  test -d "$CONFIG_DIR/.autoship/commands" || fail "package installer copies commands"
  test -d "$CONFIG_DIR/.autoship/skills" || fail "package installer copies skills"
  test -f "$CONFIG_DIR/.autoship/AGENTS.md" || fail "package installer copies AGENTS.md"
  test -f "$CONFIG_DIR/.autoship/VERSION" || fail "package installer copies VERSION"
  DOCTOR_BIN="$TMP_DIR/doctor-bin"
  mkdir -p "$DOCTOR_BIN"
  cat > "$DOCTOR_BIN/opencode" <<'SH'
#!/usr/bin/env bash
if [[ "$1" == "models" ]]; then
  printf '%s\n' opencode/minimax-m2.5-free openai/gpt-5.5
  exit 0
fi
exit 0
SH
  cat > "$DOCTOR_BIN/gh" <<'SH'
#!/usr/bin/env bash
if [[ "$1 $2" == "auth status" ]]; then
  printf '%s\n' "Token scopes: 'repo', 'workflow'"
  exit 0
fi
exit 0
SH
  chmod +x "$DOCTOR_BIN/opencode" "$DOCTOR_BIN/gh"
  DOCTOR_CONFIG="$TMP_DIR/doctor-config"
  mkdir -p "$DOCTOR_CONFIG"
  if PATH="$DOCTOR_BIN:$PATH" OPENCODE_CONFIG_DIR="$DOCTOR_CONFIG" node dist/cli.js doctor >/"$TMP_DIR/doctor-fail-1.txt" 2>&1; then
    fail "doctor exits non-zero when required checks fail"
  fi
  PATH="$DOCTOR_BIN:$PATH" OPENCODE_CONFIG_DIR="$DOCTOR_CONFIG" node dist/cli.js doctor >/"$TMP_DIR/doctor-fail-2.txt" 2>&1 || true
  cmp "$TMP_DIR/doctor-fail-1.txt" "$TMP_DIR/doctor-fail-2.txt" >/dev/null || fail "doctor failure output is deterministic"
  grep -F '[FAIL]' "$TMP_DIR/doctor-fail-1.txt" >/dev/null || fail "doctor prints FAIL checks"
  grep -F '[WARN]' "$TMP_DIR/doctor-fail-1.txt" >/dev/null || fail "doctor prints WARN checks"
  grep -F 'opencode-autoship install' "$TMP_DIR/doctor-fail-1.txt" >/dev/null || fail "doctor failure output includes package install remediation"
  printf '%s\n' '{"plugin":["opencode-autoship"]}' > "$DOCTOR_CONFIG/opencode.json"
  mkdir -p "$DOCTOR_CONFIG/.autoship/hooks" "$DOCTOR_CONFIG/.autoship/commands" "$DOCTOR_CONFIG/.autoship/skills"
  printf '{}\n' > "$DOCTOR_CONFIG/.autoship/config.json"
  printf '%s\n' '{"models":[{"id":"opencode/minimax-m2.5-free"}]}' > "$DOCTOR_CONFIG/.autoship/model-routing.json"
  cp AGENTS.md "$DOCTOR_CONFIG/.autoship/AGENTS.md"
  cp VERSION "$DOCTOR_CONFIG/.autoship/VERSION"
  date -u +%Y-%m-%dT%H:%M:%SZ > "$DOCTOR_CONFIG/.autoship/.onboarded"
  PATH="$DOCTOR_BIN:$PATH" OPENCODE_CONFIG_DIR="$DOCTOR_CONFIG" node dist/cli.js doctor >/"$TMP_DIR/doctor-pass.txt"
  grep -F '[PASS]' "$TMP_DIR/doctor-pass.txt" >/dev/null || fail "doctor prints PASS checks"
  grep -F '0 failed' "$TMP_DIR/doctor-pass.txt" >/dev/null || fail "doctor summary reports zero failures"
  grep -F 'model-inventory' "$TMP_DIR/doctor-pass.txt" >/dev/null || fail "doctor validates OpenCode model inventory"
  grep -F 'gh-auth' "$TMP_DIR/doctor-pass.txt" >/dev/null || fail "doctor validates GitHub auth"
  cat > "$DOCTOR_BIN/opencode" <<'SH'
#!/usr/bin/env bash
exit 1
SH
  cat > "$DOCTOR_BIN/gh" <<'SH'
#!/usr/bin/env bash
exit 1
SH
  chmod +x "$DOCTOR_BIN/opencode" "$DOCTOR_BIN/gh"
  PATH="$DOCTOR_BIN:$PATH" OPENCODE_CONFIG_DIR="$DOCTOR_CONFIG" node dist/cli.js doctor >/"$TMP_DIR/doctor-optional-warn.txt"
  grep -F '[WARN] model-inventory' "$TMP_DIR/doctor-optional-warn.txt" >/dev/null || fail "doctor warns for unavailable model inventory"
  grep -F '[WARN] gh-auth' "$TMP_DIR/doctor-optional-warn.txt" >/dev/null || fail "doctor warns for missing GitHub auth"
  grep -F '0 failed' "$TMP_DIR/doctor-optional-warn.txt" >/dev/null || fail "doctor treats optional readiness checks as warnings"
  printf 'v0.0.0\n' > "$DOCTOR_CONFIG/.autoship/VERSION"
  if PATH="$DOCTOR_BIN:$PATH" OPENCODE_CONFIG_DIR="$DOCTOR_CONFIG" node dist/cli.js doctor >/"$TMP_DIR/doctor-version-fail.txt" 2>&1; then
    fail "doctor fails when installed asset version does not match package"
  fi
  grep -F 'asset version' "$TMP_DIR/doctor-version-fail.txt" >/dev/null || fail "doctor reports mismatched asset version"
)

bash "$SCRIPT_DIR/test-model-parsing.sh" >/dev/null

echo "OpenCode policy tests passed"
