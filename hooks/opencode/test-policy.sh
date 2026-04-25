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

assert_file_contains() {
  local file="$1"
  local text="$2"
  local message="$3"
  grep -F "$text" "$file" >/dev/null || fail "$message"
}

assert_canonical_inventory() {
  local repo_root
  repo_root="$(cd "$SCRIPT_DIR/../.." && pwd)"
  local canonical_commands="autoship.md autoship-plan.md autoship-status.md autoship-setup.md autoship-stop.md"
  local compatibility_commands="autoship-start.md start.md plan.md status.md setup.md stop.md"
  local canonical_skills="autoship-orchestrate autoship-dispatch autoship-verify autoship-status autoship-poll autoship-setup autoship-discord-webhook autoship-discord-commands"
  local compatibility_skills="orchestrate dispatch verify status poll setup discord-webhook discord-commands"

  local command
  for command in $canonical_commands; do
    [[ -f "$repo_root/commands/$command" ]] || fail "canonical command $command is missing"
    if grep -F 'compatibility-only' "$repo_root/commands/$command" >/dev/null; then
      fail "canonical command $command must not be marked compatibility-only"
    fi
  done

  for command in $compatibility_commands; do
    [[ -f "$repo_root/commands/$command" ]] || fail "compatibility command alias $command is missing"
    assert_file_contains "$repo_root/commands/$command" 'compatibility: true' "command alias $command must declare compatibility metadata"
    assert_file_contains "$repo_root/commands/$command" 'compatibility-only' "command alias $command must be clearly marked compatibility-only"
  done

  local skill
  for skill in $canonical_skills; do
    [[ -f "$repo_root/skills/$skill/SKILL.md" ]] || fail "canonical skill $skill is missing"
    if grep -F 'compatibility-only' "$repo_root/skills/$skill/SKILL.md" >/dev/null; then
      fail "canonical skill $skill must not be marked compatibility-only"
    fi
  done

  for skill in $compatibility_skills; do
    [[ -f "$repo_root/skills/$skill/SKILL.md" ]] || fail "compatibility skill alias $skill is missing"
    assert_file_contains "$repo_root/skills/$skill/SKILL.md" 'compatibility: true' "skill alias $skill must declare compatibility metadata"
    assert_file_contains "$repo_root/skills/$skill/SKILL.md" 'compatibility-only' "skill alias $skill must be clearly marked compatibility-only"
  done
}

assert_canonical_inventory

REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
test -f "$REPO_ROOT/commands/autoship-setup.md" || fail "canonical /autoship-setup command file is installed"
grep -F '| `/autoship-setup` |' "$REPO_ROOT/README.md" >/dev/null || fail "README public command table includes /autoship-setup"
grep -F '| `/autoship-setup` |' "$REPO_ROOT/commands/autoship.md" >/dev/null || fail "/autoship command table includes /autoship-setup"

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
assert_eq "746 748 749 2301" "$eligible_numbers" "eligible issues are sorted ascending and exclude only terminal/manual labels"
assert_eq "" "$blocked_numbers" "content-based safety filter does not block issues"

limited_numbers=$(bash "$SCRIPT_DIR/plan-issues.sh" --issues-file "$ISSUES_FILE" --limit 2 | jq -r '.eligible[].number' | paste -sd ' ' -)
assert_eq "746 748" "$limited_numbers" "plan limit caps eligible queue"

fix_title=$(bash "$SCRIPT_DIR/pr-title.sh" --issue 2298 --title "Validate Discord webhook URLs" --labels "bug,security,agent:ready")
docs_title=$(bash "$SCRIPT_DIR/pr-title.sh" --issue 2296 --title "mandate poison recovery pattern" --labels "documentation,agent:ready")
assert_eq "fix: Validate Discord webhook URLs (#2298)" "$fix_title" "bug/security title uses fix prefix"
assert_eq "docs: mandate poison recovery pattern (#2296)" "$docs_title" "documentation title uses docs prefix"

PACKAGE_VERIFY_REPO="$TMP_DIR/package-verify-repo"
mkdir -p "$PACKAGE_VERIFY_REPO/dist" "$PACKAGE_VERIFY_REPO/hooks/opencode" "$PACKAGE_VERIFY_REPO/commands" "$PACKAGE_VERIFY_REPO/skills/autoship-setup" "$PACKAGE_VERIFY_REPO/plugins" "$PACKAGE_VERIFY_REPO/.autoship"
cp "$SCRIPT_DIR/../../package.json" "$PACKAGE_VERIFY_REPO/package.json"
jq '.files += [".autoship", "unintended.tmp"]' "$PACKAGE_VERIFY_REPO/package.json" > "$PACKAGE_VERIFY_REPO/package.json.tmp" && mv "$PACKAGE_VERIFY_REPO/package.json.tmp" "$PACKAGE_VERIFY_REPO/package.json"
printf 'runtime state\n' > "$PACKAGE_VERIFY_REPO/.autoship/state.json"
printf 'unintended\n' > "$PACKAGE_VERIFY_REPO/unintended.tmp"
printf 'built\n' > "$PACKAGE_VERIFY_REPO/dist/index.js"
printf 'hook\n' > "$PACKAGE_VERIFY_REPO/hooks/init.sh"
printf 'hook\n' > "$PACKAGE_VERIFY_REPO/hooks/opencode/init.sh"
printf 'hook\n' > "$PACKAGE_VERIFY_REPO/hooks/opencode/sync-release.sh"
printf 'command\n' > "$PACKAGE_VERIFY_REPO/commands/autoship.md"
printf 'command\n' > "$PACKAGE_VERIFY_REPO/commands/autoship-setup.md"
printf 'skill\n' > "$PACKAGE_VERIFY_REPO/skills/autoship-orchestrate.md"
printf 'skill\n' > "$PACKAGE_VERIFY_REPO/skills/autoship-setup/SKILL.md"
printf 'plugin\n' > "$PACKAGE_VERIFY_REPO/plugins/autoship.ts"
printf 'agents\n' > "$PACKAGE_VERIFY_REPO/AGENTS.md"
printf '1.0.0\n' > "$PACKAGE_VERIFY_REPO/VERSION"
printf 'readme\n' > "$PACKAGE_VERIFY_REPO/README.md"
printf 'license\n' > "$PACKAGE_VERIFY_REPO/LICENSE"
(
  cd "$PACKAGE_VERIFY_REPO"
  if bash "$SCRIPT_DIR/verify-package.sh" >/dev/null 2>&1; then
    fail "package verification should reject runtime state and unintended files"
  fi
  rm -rf .autoship unintended.tmp
  cp "$SCRIPT_DIR/../../package.json" package.json
  bash "$SCRIPT_DIR/verify-package.sh" >/dev/null
)

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
assert_eq "verifying" "$(jq -r '.issues["issue-746"].state' "$STATE_REPO/.autoship/state.json")" "COMPLETE workspace reconciles to verifying"
assert_eq "blocked" "$(jq -r '.issues["issue-749"].state' "$STATE_REPO/.autoship/state.json")" "BLOCKED workspace reconciles to blocked"
assert_eq "running" "$(jq -r '.issues["issue-750"].state' "$STATE_REPO/.autoship/state.json")" "RUNNING workspace remains running"
assert_eq "queued" "$(jq -r '.issues["issue-751"].state' "$STATE_REPO/.autoship/state.json")" "QUEUED workspace remains queued"
assert_eq "stuck" "$(jq -r '.issues["issue-752"].state' "$STATE_REPO/.autoship/state.json")" "STUCK workspace reconciles to stuck"
assert_eq "false" "$(jq -r '.issues["issue-752"].has_result' "$STATE_REPO/.autoship/state.json")" "stale result older than started_at is ignored"
assert_eq "0" "$(jq -r '.stats.session_completed // 0' "$STATE_REPO/.autoship/state.json")" "reconcile does not count completion before review"
assert_eq "1" "$(jq -r '.stats.blocked' "$STATE_REPO/.autoship/state.json")" "reconcile increments blocked stats"
assert_eq "1" "$(jq -r '.stats.failed' "$STATE_REPO/.autoship/state.json")" "reconcile increments failed stats for stuck workspaces"

STATUS_OUTPUT=$(bash "$SCRIPT_DIR/status.sh" --repo "$STATE_REPO")
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'AGENTS (1 active / 15 max)' >/dev/null || fail "status shows active/max concurrency"
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'Queued:    1' >/dev/null || fail "status shows queued count"
printf '%s\n' "$STATUS_OUTPUT" | grep -F 'Completed: 0' >/dev/null || fail "status shows completed count"
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
for _ in 1 2 3 4 5; do
  artifact_count=$(find "$RUNNER_REPO/.autoship/failures" -name '*-issue-996.json' 2>/dev/null | wc -l | tr -d '[:space:]')
  [[ "$artifact_count" != "0" ]] && break
  sleep 1
done
artifact_count=$(find "$RUNNER_REPO/.autoship/failures" -name '*-issue-996.json' 2>/dev/null | wc -l | tr -d '[:space:]')
for _ in 1 2 3 4 5; do
  [[ "$artifact_count" != "0" ]] && break
  sleep 1
  artifact_count=$(find "$RUNNER_REPO/.autoship/failures" -name '*-issue-996.json' 2>/dev/null | wc -l | tr -d '[:space:]')
done
assert_eq "1" "$artifact_count" "runner captures a stuck worker failure artifact"
artifact_file=$(find "$RUNNER_REPO/.autoship/failures" -name '*-issue-996.json' | head -1)
jq -e '.issue == "issue-996" and .model == "opencode/test-free" and .role == "implementer" and .workspace != "" and .hook == "hooks/opencode/runner.sh" and .failure_category == "stuck" and (.logs | contains("ok")) and .attempt == 2' "$artifact_file" >/dev/null || fail "failure artifact includes issue, model, workspace, hook, logs, category, role, and attempt"
test -s "$RUNNER_REPO/.autoship/workspaces/issue-996/worker.pid" || fail "runner records worker pid for lifecycle monitoring"

RETRY_REPO="$TMP_DIR/retry-limit-repo"
mkdir -p "$RETRY_REPO/.autoship/workspaces/issue-181" "$RETRY_REPO/.autoship/failures" "$RETRY_REPO/hooks/opencode" "$RETRY_REPO/hooks"
git init -q "$RETRY_REPO"
cp "$SCRIPT_DIR/../update-state.sh" "$RETRY_REPO/hooks/update-state.sh"
cp "$SCRIPT_DIR/dispatch.sh" "$RETRY_REPO/hooks/opencode/dispatch.sh"
chmod +x "$RETRY_REPO/hooks/update-state.sh" "$RETRY_REPO/hooks/opencode/dispatch.sh"
cat > "$RETRY_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-181":{"state":"running","attempt":3,"model":"opencode/test","role":"implementer"}},"stats":{},"config":{"maxConcurrentAgents":15,"maxRetries":3}}
JSON
cat > "$RETRY_REPO/.autoship/failures/20260424T010000Z-issue-181.json" <<'JSON'
{"failure_id":"20260424T010000Z-issue-181","issue":"issue-181","failure_category":"failed_verification","error_summary":"tests failed","attempt":3,"timestamp":"2026-04-24T01:00:00Z"}
JSON
(
  cd "$RETRY_REPO"
  bash hooks/update-state.sh set-failed issue-181 escalation_reason="failed verification after retry limit" >/dev/null
)
jq -e '.issues["issue-181"].state == "blocked"
  and .issues["issue-181"].retry_count == 3
  and .issues["issue-181"].retry_limit == 3
  and .issues["issue-181"].retry_eligible == false
  and .issues["issue-181"].terminal_failure == true
  and .issues["issue-181"].escalation_reason == "failed verification after retry limit"
  and (.issues["issue-181"].failure_evidence.failure_file | endswith("20260424T010000Z-issue-181.json"))
  and .issues["issue-181"].failure_evidence.failure_category == "failed_verification"
  and .issues["issue-181"].failure_evidence.error_summary == "tests failed"' "$RETRY_REPO/.autoship/state.json" >/dev/null || fail "set-failed records terminal retry exhaustion evidence"
dispatch_output=$(cd "$RETRY_REPO" && bash hooks/opencode/dispatch.sh 181 medium_code)
printf '%s\n' "$dispatch_output" | grep -F 'failed verification after retry limit' >/dev/null || fail "dispatch blocks terminal retry-exhausted issue"
assert_eq "BLOCKED" "$(tr -d '[:space:]' < "$RETRY_REPO/.autoship/workspaces/issue-181/status")" "terminal retry-exhausted issue remains blocked instead of redispatched"

printf '{broken json' > "$RETRY_REPO/.autoship/failures/20260424T020000Z-issue-182.json"
jq '.issues["issue-182"] = {"state":"running","attempt":2}' "$RETRY_REPO/.autoship/state.json" > "$RETRY_REPO/.autoship/state.json.tmp" && mv "$RETRY_REPO/.autoship/state.json.tmp" "$RETRY_REPO/.autoship/state.json"
(
  cd "$RETRY_REPO"
  bash hooks/update-state.sh set-failed issue-182 error_summary="malformed artifact fallback" >/dev/null
)
jq -e '.issues["issue-182"].retry_count == 2
  and .issues["issue-182"].retry_limit == 3
  and .issues["issue-182"].retry_eligible == true
  and .issues["issue-182"].terminal_failure == false
  and .issues["issue-182"].failure_evidence.error_summary == "malformed artifact fallback"' "$RETRY_REPO/.autoship/state.json" >/dev/null || fail "set-failed falls back to synthesized evidence before retry limit"

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
for _ in 1 2 3 4 5 6 7 8 9 10; do
  [[ "$(tr -d '[:space:]' < "$FALLBACK_REPO/.autoship/workspaces/issue-208/status")" != "RUNNING" ]] && break
  sleep 1
done
assert_eq "COMPLETE" "$(tr -d '[:space:]' < "$FALLBACK_REPO/.autoship/workspaces/issue-208/status")" "runner retries billing failures with a free fallback model"
assert_eq "opencode/free-fallback" "$(tr -d '[:space:]' < "$FALLBACK_REPO/.autoship/workspaces/issue-208/model")" "runner records fallback model in workspace"
jq -e '."opencode/paid-model".fail == 1 and (."opencode/paid-model".last_error | test("Insufficient balance"))' "$FALLBACK_REPO/.autoship/model-history.json" >/dev/null || fail "runner records paid model billing failure in model history"

AUTOCOMMIT_REPO="$TMP_DIR/autocommit-runner-repo"
mkdir -p "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253" "$AUTOCOMMIT_REPO/hooks/opencode" "$AUTOCOMMIT_REPO/hooks" "$AUTOCOMMIT_REPO/bin"
git init -q "$AUTOCOMMIT_REPO"
cp "$SCRIPT_DIR/runner.sh" "$AUTOCOMMIT_REPO/hooks/opencode/runner.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$AUTOCOMMIT_REPO/hooks/update-state.sh"
cp "$SCRIPT_DIR/../capture-failure.sh" "$AUTOCOMMIT_REPO/hooks/capture-failure.sh"
chmod +x "$AUTOCOMMIT_REPO/hooks/opencode/runner.sh" "$AUTOCOMMIT_REPO/hooks/update-state.sh" "$AUTOCOMMIT_REPO/hooks/capture-failure.sh"
git -C "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253" init -q
git -C "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253" config user.email autoship@example.invalid
git -C "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253" config user.name AutoShip
mkdir -p "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253/src"
printf 'base\n' > "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253/src/lib.rs"
git -C "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253" add src/lib.rs
git -C "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253" commit -q -m initial
cat > "$AUTOCOMMIT_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-253":{"state":"queued","model":"opencode/test-free","role":"implementer","attempt":1,"task_type":"medium_code"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf 'QUEUED\n' > "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253/status"
printf 'test prompt\n' > "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253/AUTOSHIP_PROMPT.md"
printf 'opencode/test-free\n' > "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253/model"
cat > "$AUTOCOMMIT_REPO/bin/opencode" <<'SH'
#!/usr/bin/env bash
printf 'impl\n' >> src/lib.rs
printf 'COMPLETE\n' > status
printf 'implemented\n' > AUTOSHIP_RESULT.md
exit 0
SH
chmod +x "$AUTOCOMMIT_REPO/bin/opencode"
(
  cd "$AUTOCOMMIT_REPO"
  PATH="$AUTOCOMMIT_REPO/bin:$PATH" bash hooks/opencode/runner.sh >/dev/null
)
for _ in 1 2 3 4 5; do
  [[ "$(tr -d '[:space:]' < "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253/status")" != "RUNNING" ]] && break
  sleep 1
done
assert_eq "COMPLETE" "$(tr -d '[:space:]' < "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253/status")" "runner keeps complete status after auto-committing production changes"
assert_eq "2" "$(git -C "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253" rev-list --count HEAD)" "runner auto-commits worker changes before completion"
git -C "$AUTOCOMMIT_REPO/.autoship/workspaces/issue-253" diff --quiet || fail "runner leaves no unstaged worker changes after auto-commit"

TESTS_ONLY_REPO="$TMP_DIR/tests-only-runner-repo"
mkdir -p "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254" "$TESTS_ONLY_REPO/hooks/opencode" "$TESTS_ONLY_REPO/hooks" "$TESTS_ONLY_REPO/bin"
git init -q "$TESTS_ONLY_REPO"
cp "$SCRIPT_DIR/runner.sh" "$TESTS_ONLY_REPO/hooks/opencode/runner.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$TESTS_ONLY_REPO/hooks/update-state.sh"
cp "$SCRIPT_DIR/../capture-failure.sh" "$TESTS_ONLY_REPO/hooks/capture-failure.sh"
chmod +x "$TESTS_ONLY_REPO/hooks/opencode/runner.sh" "$TESTS_ONLY_REPO/hooks/update-state.sh" "$TESTS_ONLY_REPO/hooks/capture-failure.sh"
git -C "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254" init -q
git -C "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254" config user.email autoship@example.invalid
git -C "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254" config user.name AutoShip
mkdir -p "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254/src"
printf 'base\n' > "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254/src/lib.rs"
git -C "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254" add src/lib.rs
git -C "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254" commit -q -m initial
cat > "$TESTS_ONLY_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-254":{"state":"queued","model":"opencode/test-free","role":"implementer","attempt":1,"task_type":"medium_code"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf 'QUEUED\n' > "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254/status"
printf 'test prompt\n' > "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254/AUTOSHIP_PROMPT.md"
printf 'opencode/test-free\n' > "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254/model"
cat > "$TESTS_ONLY_REPO/bin/opencode" <<'SH'
#!/usr/bin/env bash
mkdir -p tests
printf 'test only\n' > tests/new.test.ts
printf 'COMPLETE\n' > status
printf 'tests only\n' > AUTOSHIP_RESULT.md
exit 0
SH
chmod +x "$TESTS_ONLY_REPO/bin/opencode"
(
  cd "$TESTS_ONLY_REPO"
  PATH="$TESTS_ONLY_REPO/bin:$PATH" bash hooks/opencode/runner.sh >/dev/null
)
for _ in 1 2 3 4 5; do
  [[ "$(tr -d '[:space:]' < "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254/status")" != "RUNNING" ]] && break
  sleep 1
done
assert_eq "STUCK" "$(tr -d '[:space:]' < "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254/status")" "runner rejects tests-only complete results"
grep -F 'REJECT: tests-only diff' "$TESTS_ONLY_REPO/.autoship/workspaces/issue-254/AUTOSHIP_RUNNER.log" >/dev/null || fail "runner records tests-only rejection reason"

SESSION_REPO="$TMP_DIR/session-runner-repo"
mkdir -p "$SESSION_REPO/.autoship/workspaces/issue-997" "$SESSION_REPO/hooks/opencode" "$SESSION_REPO/hooks" "$SESSION_REPO/bin"
git init -q "$SESSION_REPO"
cp "$SCRIPT_DIR/runner.sh" "$SESSION_REPO/hooks/opencode/runner.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$SESSION_REPO/hooks/update-state.sh"
cp "$SCRIPT_DIR/../capture-failure.sh" "$SESSION_REPO/hooks/capture-failure.sh"
chmod +x "$SESSION_REPO/hooks/opencode/runner.sh" "$SESSION_REPO/hooks/update-state.sh" "$SESSION_REPO/hooks/capture-failure.sh"
cat > "$SESSION_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-997":{"state":"queued","model":"opencode/nemotron-3-super-free","role":"implementer","attempt":1,"task_type":"medium_code"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf 'QUEUED\n' > "$SESSION_REPO/.autoship/workspaces/issue-997/status"
printf 'test prompt\n' > "$SESSION_REPO/.autoship/workspaces/issue-997/AUTOSHIP_PROMPT.md"
printf 'opencode/nemotron-3-super-free\n' > "$SESSION_REPO/.autoship/workspaces/issue-997/model"
cat > "$SESSION_REPO/bin/opencode" <<'SH'
#!/usr/bin/env bash
printf 'Session not found\n' >&2
exit 1
SH
chmod +x "$SESSION_REPO/bin/opencode"
(
  cd "$SESSION_REPO"
  PATH="$SESSION_REPO/bin:$PATH" bash hooks/opencode/runner.sh >/dev/null
)
for _ in 1 2 3 4 5; do
  [[ "$(tr -d '[:space:]' < "$SESSION_REPO/.autoship/workspaces/issue-997/status")" != "RUNNING" ]] && break
  sleep 1
done
assert_eq "STUCK" "$(tr -d '[:space:]' < "$SESSION_REPO/.autoship/workspaces/issue-997/status")" "runner marks session failures stuck"
grep -F 'OpenCode returned Session not found' "$SESSION_REPO/.autoship/workspaces/issue-997/AUTOSHIP_RUNNER.log" >/dev/null || fail "runner explains OpenCode session failures"

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

QUEUE_REPO="$TMP_DIR/queue-repo"
mkdir -p "$QUEUE_REPO/.autoship" "$QUEUE_REPO/hooks/opencode" "$QUEUE_REPO/hooks"
git init -q "$QUEUE_REPO"
cp "$SCRIPT_DIR/process-event-queue.sh" "$QUEUE_REPO/hooks/opencode/process-event-queue.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$QUEUE_REPO/hooks/update-state.sh"
chmod +x "$QUEUE_REPO/hooks/opencode/process-event-queue.sh" "$QUEUE_REPO/hooks/update-state.sh"
cat > "$QUEUE_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-991":{"state":"running"},"issue-992":{"state":"running"},"issue-993":{"state":"running"}},"stats":{}}
JSON
printf '{"sessions":[]}' > "$QUEUE_REPO/.autoship/token-ledger.json"
cat > "$QUEUE_REPO/.autoship/event-queue.json" <<'JSON'
[
  {"type":"blocked","issue":"issue-991","priority":2,"data":{"status":"BLOCKED"},"queued_at":"2026-04-24T00:00:00Z"},
  {"type":"blocked","issue":"issue-991","priority":2,"data":{"status":"BLOCKED"},"queued_at":"2026-04-24T00:00:01Z"},
  {"type":"verify","issue":"issue-992","priority":2,"data":{"status":"COMPLETE"},"queued_at":"2026-04-24T00:00:02Z"},
  {"type":"verify","issue":"issue-992","priority":2,"data":{"status":"COMPLETE"},"queued_at":"2026-04-24T00:00:03Z"},
  {"type":"stuck","issue":"issue-993","priority":2,"data":{"status":"STUCK"},"queued_at":"2026-04-24T00:00:04Z"},
  {"type":"stuck","issue":"issue-993","priority":2,"data":{"status":"STUCK"},"queued_at":"2026-04-24T00:00:05Z"}
]
JSON
(
  cd "$QUEUE_REPO"
  bash hooks/opencode/process-event-queue.sh >/dev/null
)
assert_eq "blocked" "$(jq -r '.issues["issue-991"].state' "$QUEUE_REPO/.autoship/state.json")" "event processor applies blocked event"
assert_eq "verifying" "$(jq -r '.issues["issue-992"].state' "$QUEUE_REPO/.autoship/state.json")" "event processor advances COMPLETE event to verification"
assert_eq "stuck" "$(jq -r '.issues["issue-993"].state' "$QUEUE_REPO/.autoship/state.json")" "event processor applies stuck event"
assert_eq "1" "$(jq -r '.stats.blocked' "$QUEUE_REPO/.autoship/state.json")" "duplicate blocked events do not double-update stats"
assert_eq "0" "$(jq -r '.stats.session_completed // 0' "$QUEUE_REPO/.autoship/state.json")" "verify events do not count completion before review"
assert_eq "1" "$(jq -r '.stats.failed' "$QUEUE_REPO/.autoship/state.json")" "duplicate stuck events do not double-update failed stats"
assert_eq "0" "$(jq 'length' "$QUEUE_REPO/.autoship/event-queue.json")" "event processor drains processed duplicate events"
assert_eq "3" "$(jq 'length' "$QUEUE_REPO/.autoship/processed-events.json")" "event processor records only unique semantic events"

VERIFY_FAIL_REPO="$TMP_DIR/verify-fail-repo"
mkdir -p "$VERIFY_FAIL_REPO/.autoship/workspaces/issue-184" "$VERIFY_FAIL_REPO/hooks/opencode" "$VERIFY_FAIL_REPO/hooks" "$VERIFY_FAIL_REPO/bin"
git init -q "$VERIFY_FAIL_REPO"
git -C "$VERIFY_FAIL_REPO" config user.email autoship@example.invalid
git -C "$VERIFY_FAIL_REPO" config user.name AutoShip
printf 'base\n' > "$VERIFY_FAIL_REPO/README.md"
git -C "$VERIFY_FAIL_REPO" add README.md
git -C "$VERIFY_FAIL_REPO" commit -q -m initial
git -C "$VERIFY_FAIL_REPO" checkout -q -b autoship/issue-184
printf 'changed\n' > "$VERIFY_FAIL_REPO/feature.txt"
cp "$SCRIPT_DIR/process-event-queue.sh" "$VERIFY_FAIL_REPO/hooks/opencode/process-event-queue.sh"
cp "$SCRIPT_DIR/pr-title.sh" "$VERIFY_FAIL_REPO/hooks/opencode/pr-title.sh"
cp "$SCRIPT_DIR/verify-result.sh" "$VERIFY_FAIL_REPO/hooks/opencode/verify-result.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$VERIFY_FAIL_REPO/hooks/update-state.sh"
chmod +x "$VERIFY_FAIL_REPO/hooks/opencode/process-event-queue.sh" "$VERIFY_FAIL_REPO/hooks/opencode/pr-title.sh" "$VERIFY_FAIL_REPO/hooks/opencode/verify-result.sh" "$VERIFY_FAIL_REPO/hooks/update-state.sh"
cat > "$VERIFY_FAIL_REPO/hooks/opencode/reviewer.sh" <<'SH'
#!/usr/bin/env bash
printf 'VERDICT: FAIL\n'
exit 1
SH
chmod +x "$VERIFY_FAIL_REPO/hooks/opencode/reviewer.sh"
cat > "$VERIFY_FAIL_REPO/bin/gh" <<'SH'
#!/usr/bin/env bash
if [[ "$1 $2" == "pr create" ]]; then
  printf 'pr create called\n' >> "$GH_PR_LOG"
  exit 0
fi
if [[ "$1 $2" == "issue view" && "$*" == *"title"* ]]; then
  printf 'Create PR after verified PASS\n'
  exit 0
fi
if [[ "$1 $2" == "issue view" && "$*" == *"labels"* ]]; then
  printf 'type:feature\n'
  exit 0
fi
if [[ "$1 $2" == "label list" ]]; then
  printf '%s\n' autoship:blocked autoship:in-progress autoship:done autoship:paused
  exit 0
fi
exit 0
SH
chmod +x "$VERIFY_FAIL_REPO/bin/gh"
cat > "$VERIFY_FAIL_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-184":{"state":"running","title":"Create PR after verified PASS","labels":"type:feature"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf '[]\n' > "$VERIFY_FAIL_REPO/.autoship/processed-events.json"
cat > "$VERIFY_FAIL_REPO/.autoship/event-queue.json" <<'JSON'
[{"type":"verify","issue":"issue-184","priority":2,"data":{"status":"COMPLETE"},"queued_at":"2026-04-24T00:00:00Z"}]
JSON
printf 'Result\n' > "$VERIFY_FAIL_REPO/.autoship/workspaces/issue-184/AUTOSHIP_RESULT.md"
(
  cd "$VERIFY_FAIL_REPO"
  GH_PR_LOG="$VERIFY_FAIL_REPO/gh-pr.log" PATH="$VERIFY_FAIL_REPO/bin:$PATH" bash hooks/opencode/process-event-queue.sh >/dev/null
)
test ! -e "$VERIFY_FAIL_REPO/gh-pr.log" || fail "failed verification must not call gh pr create"
assert_eq "blocked" "$(jq -r '.issues["issue-184"].state' "$VERIFY_FAIL_REPO/.autoship/state.json")" "failed verification blocks issue instead of completing it"

VERIFY_PASS_REPO="$TMP_DIR/verify-pass-repo"
mkdir -p "$VERIFY_PASS_REPO/.autoship/workspaces/issue-184" "$VERIFY_PASS_REPO/hooks/opencode" "$VERIFY_PASS_REPO/hooks" "$VERIFY_PASS_REPO/bin"
git init -q "$VERIFY_PASS_REPO"
git -C "$VERIFY_PASS_REPO" config user.email autoship@example.invalid
git -C "$VERIFY_PASS_REPO" config user.name AutoShip
git -C "$VERIFY_PASS_REPO" remote add origin git@github.com:owner/repo.git
printf 'base\n' > "$VERIFY_PASS_REPO/README.md"
git -C "$VERIFY_PASS_REPO" add README.md
git -C "$VERIFY_PASS_REPO" commit -q -m initial
git -C "$VERIFY_PASS_REPO" checkout -q -b autoship/issue-184
printf 'changed\n' > "$VERIFY_PASS_REPO/feature.txt"
cp "$SCRIPT_DIR/process-event-queue.sh" "$VERIFY_PASS_REPO/hooks/opencode/process-event-queue.sh"
cp "$SCRIPT_DIR/pr-title.sh" "$VERIFY_PASS_REPO/hooks/opencode/pr-title.sh"
cp "$SCRIPT_DIR/verify-result.sh" "$VERIFY_PASS_REPO/hooks/opencode/verify-result.sh"
cp "$SCRIPT_DIR/../update-state.sh" "$VERIFY_PASS_REPO/hooks/update-state.sh"
chmod +x "$VERIFY_PASS_REPO/hooks/opencode/process-event-queue.sh" "$VERIFY_PASS_REPO/hooks/opencode/pr-title.sh" "$VERIFY_PASS_REPO/hooks/opencode/verify-result.sh" "$VERIFY_PASS_REPO/hooks/update-state.sh"
cat > "$VERIFY_PASS_REPO/hooks/opencode/reviewer.sh" <<'SH'
#!/usr/bin/env bash
printf 'VERDICT: PASS\n'
exit 0
SH
chmod +x "$VERIFY_PASS_REPO/hooks/opencode/reviewer.sh"
cat > "$VERIFY_PASS_REPO/bin/gh" <<'SH'
#!/usr/bin/env bash
if [[ "$1 $2" == "pr create" ]]; then
  printf '%s\n' "$*" >> "$GH_PR_LOG"
  printf 'https://github.com/owner/repo/pull/184\n'
  exit 0
fi
if [[ "$1 $2" == "issue view" && "$*" == *"title"* ]]; then
  printf 'Create PR after verified PASS\n'
  exit 0
fi
if [[ "$1 $2" == "issue view" && "$*" == *"labels"* ]]; then
  printf 'type:feature\n'
  exit 0
fi
if [[ "$1 $2" == "label list" ]]; then
  printf '%s\n' autoship:blocked autoship:in-progress autoship:done autoship:paused
  exit 0
fi
exit 0
SH
chmod +x "$VERIFY_PASS_REPO/bin/gh"
cat > "$VERIFY_PASS_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-184":{"state":"running","title":"Create PR after verified PASS","labels":"type:feature"}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf '[]\n' > "$VERIFY_PASS_REPO/.autoship/processed-events.json"
cat > "$VERIFY_PASS_REPO/.autoship/event-queue.json" <<'JSON'
[{"type":"verify","issue":"issue-184","priority":2,"data":{"status":"COMPLETE"},"queued_at":"2026-04-24T00:00:00Z"}]
JSON
printf 'Result summary\n' > "$VERIFY_PASS_REPO/.autoship/workspaces/issue-184/AUTOSHIP_RESULT.md"
(
  cd "$VERIFY_PASS_REPO"
  GH_PR_LOG="$VERIFY_PASS_REPO/gh-pr.log" PATH="$VERIFY_PASS_REPO/bin:$PATH" bash hooks/opencode/process-event-queue.sh >/dev/null
)
grep -F 'pr create --title feat: Create PR after verified PASS (#184)' "$VERIFY_PASS_REPO/gh-pr.log" >/dev/null || fail "verified PASS creates PR with conventional title"
grep -F -- '--body-file .autoship/workspaces/issue-184/AUTOSHIP_PR_BODY.md' "$VERIFY_PASS_REPO/gh-pr.log" >/dev/null || fail "verified PASS creates PR from generated body file"
grep -F 'Reviewer: PASS' "$VERIFY_PASS_REPO/.autoship/workspaces/issue-184/AUTOSHIP_PR_BODY.md" >/dev/null || fail "generated PR body records reviewer pass"
assert_eq "completed" "$(jq -r '.issues["issue-184"].state' "$VERIFY_PASS_REPO/.autoship/state.json")" "verified PASS completes issue after PR creation"

VERIFY_HOOK_PASS_REPO="$TMP_DIR/verify-hook-pass-repo"
mkdir -p "$VERIFY_HOOK_PASS_REPO/.autoship/workspaces/issue-182" "$VERIFY_HOOK_PASS_REPO/hooks/opencode"
git init -q "$VERIFY_HOOK_PASS_REPO"
git -C "$VERIFY_HOOK_PASS_REPO" config user.email autoship@example.invalid
git -C "$VERIFY_HOOK_PASS_REPO" config user.name AutoShip
printf 'base\n' > "$VERIFY_HOOK_PASS_REPO/README.md"
git -C "$VERIFY_HOOK_PASS_REPO" add README.md
git -C "$VERIFY_HOOK_PASS_REPO" commit -q -m initial
git -C "$VERIFY_HOOK_PASS_REPO" checkout -q -b autoship/issue-182
printf 'changed\n' > "$VERIFY_HOOK_PASS_REPO/feature.txt"
git -C "$VERIFY_HOOK_PASS_REPO" add feature.txt
git -C "$VERIFY_HOOK_PASS_REPO" commit -q -m 'feat: issue 182'
cp "$SCRIPT_DIR/verify-result.sh" "$VERIFY_HOOK_PASS_REPO/hooks/opencode/verify-result.sh"
chmod +x "$VERIFY_HOOK_PASS_REPO/hooks/opencode/verify-result.sh"
cat > "$VERIFY_HOOK_PASS_REPO/hooks/opencode/reviewer.sh" <<'SH'
#!/usr/bin/env bash
printf 'VERDICT: PASS\n'
SH
chmod +x "$VERIFY_HOOK_PASS_REPO/hooks/opencode/reviewer.sh"
printf '2026-04-24T00:00:00Z\n' > "$VERIFY_HOOK_PASS_REPO/.autoship/workspaces/issue-182/started_at"
printf 'Result summary\n' > "$VERIFY_HOOK_PASS_REPO/.autoship/workspaces/issue-182/AUTOSHIP_RESULT.md"
touch -t 202604240000 "$VERIFY_HOOK_PASS_REPO/.autoship/workspaces/issue-182/started_at"
touch -t 202604240001 "$VERIFY_HOOK_PASS_REPO/.autoship/workspaces/issue-182/AUTOSHIP_RESULT.md"
(
  cd "$VERIFY_HOOK_PASS_REPO"
  bash hooks/opencode/verify-result.sh issue-182 "$VERIFY_HOOK_PASS_REPO/.autoship/workspaces/issue-182" true >/tmp/autoship-verify-pass.out
)
grep -F 'PASS' /tmp/autoship-verify-pass.out >/dev/null || fail "deterministic verification hook emits PASS"

VERIFY_HOOK_FAIL_REPO="$TMP_DIR/verify-hook-fail-repo"
mkdir -p "$VERIFY_HOOK_FAIL_REPO/.autoship/workspaces/issue-182" "$VERIFY_HOOK_FAIL_REPO/hooks/opencode"
git init -q "$VERIFY_HOOK_FAIL_REPO"
git -C "$VERIFY_HOOK_FAIL_REPO" config user.email autoship@example.invalid
git -C "$VERIFY_HOOK_FAIL_REPO" config user.name AutoShip
printf 'base\n' > "$VERIFY_HOOK_FAIL_REPO/README.md"
git -C "$VERIFY_HOOK_FAIL_REPO" add README.md
git -C "$VERIFY_HOOK_FAIL_REPO" commit -q -m initial
git -C "$VERIFY_HOOK_FAIL_REPO" checkout -q -b autoship/issue-182
printf 'changed\n' > "$VERIFY_HOOK_FAIL_REPO/feature.txt"
git -C "$VERIFY_HOOK_FAIL_REPO" add feature.txt
git -C "$VERIFY_HOOK_FAIL_REPO" commit -q -m 'feat: issue 182'
cp "$SCRIPT_DIR/verify-result.sh" "$VERIFY_HOOK_FAIL_REPO/hooks/opencode/verify-result.sh"
chmod +x "$VERIFY_HOOK_FAIL_REPO/hooks/opencode/verify-result.sh"
cat > "$VERIFY_HOOK_FAIL_REPO/hooks/opencode/reviewer.sh" <<'SH'
#!/usr/bin/env bash
printf 'VERDICT: PASS\n'
SH
chmod +x "$VERIFY_HOOK_FAIL_REPO/hooks/opencode/reviewer.sh"
printf '2026-04-24T00:00:00Z\n' > "$VERIFY_HOOK_FAIL_REPO/.autoship/workspaces/issue-182/started_at"
printf 'stale result\n' > "$VERIFY_HOOK_FAIL_REPO/.autoship/workspaces/issue-182/AUTOSHIP_RESULT.md"
touch -t 202604240001 "$VERIFY_HOOK_FAIL_REPO/.autoship/workspaces/issue-182/started_at"
touch -t 202604240000 "$VERIFY_HOOK_FAIL_REPO/.autoship/workspaces/issue-182/AUTOSHIP_RESULT.md"
if (
  cd "$VERIFY_HOOK_FAIL_REPO"
  bash hooks/opencode/verify-result.sh issue-182 "$VERIFY_HOOK_FAIL_REPO/.autoship/workspaces/issue-182" true >/tmp/autoship-verify-fail.out 2>&1
); then
  fail "deterministic verification hook fails stale results"
fi
grep -F 'FAIL' /tmp/autoship-verify-fail.out >/dev/null || fail "deterministic verification hook emits FAIL"

VERIFY_HOOK_TEST_FAIL_REPO="$TMP_DIR/verify-hook-test-fail-repo"
mkdir -p "$VERIFY_HOOK_TEST_FAIL_REPO/.autoship/workspaces/issue-182" "$VERIFY_HOOK_TEST_FAIL_REPO/hooks/opencode"
git init -q "$VERIFY_HOOK_TEST_FAIL_REPO"
git -C "$VERIFY_HOOK_TEST_FAIL_REPO" config user.email autoship@example.invalid
git -C "$VERIFY_HOOK_TEST_FAIL_REPO" config user.name AutoShip
printf 'base\n' > "$VERIFY_HOOK_TEST_FAIL_REPO/README.md"
git -C "$VERIFY_HOOK_TEST_FAIL_REPO" add README.md
git -C "$VERIFY_HOOK_TEST_FAIL_REPO" commit -q -m initial
git -C "$VERIFY_HOOK_TEST_FAIL_REPO" checkout -q -b autoship/issue-182
printf 'changed\n' > "$VERIFY_HOOK_TEST_FAIL_REPO/feature.txt"
git -C "$VERIFY_HOOK_TEST_FAIL_REPO" add feature.txt
git -C "$VERIFY_HOOK_TEST_FAIL_REPO" commit -q -m 'feat: issue 182'
cp "$SCRIPT_DIR/verify-result.sh" "$VERIFY_HOOK_TEST_FAIL_REPO/hooks/opencode/verify-result.sh"
chmod +x "$VERIFY_HOOK_TEST_FAIL_REPO/hooks/opencode/verify-result.sh"
cat > "$VERIFY_HOOK_TEST_FAIL_REPO/hooks/opencode/reviewer.sh" <<'SH'
#!/usr/bin/env bash
printf 'VERDICT: PASS\n'
SH
chmod +x "$VERIFY_HOOK_TEST_FAIL_REPO/hooks/opencode/reviewer.sh"
printf '2026-04-24T00:00:00Z\n' > "$VERIFY_HOOK_TEST_FAIL_REPO/.autoship/workspaces/issue-182/started_at"
printf 'Result summary\n' > "$VERIFY_HOOK_TEST_FAIL_REPO/.autoship/workspaces/issue-182/AUTOSHIP_RESULT.md"
touch -t 202604240000 "$VERIFY_HOOK_TEST_FAIL_REPO/.autoship/workspaces/issue-182/started_at"
touch -t 202604240001 "$VERIFY_HOOK_TEST_FAIL_REPO/.autoship/workspaces/issue-182/AUTOSHIP_RESULT.md"
if (
  cd "$VERIFY_HOOK_TEST_FAIL_REPO"
  bash hooks/opencode/verify-result.sh issue-182 "$VERIFY_HOOK_TEST_FAIL_REPO/.autoship/workspaces/issue-182" false >/tmp/autoship-verify-test-fail.out 2>&1
); then
  fail "deterministic verification hook fails failing test command"
fi
grep -F 'test command failed' "$VERIFY_HOOK_TEST_FAIL_REPO/.autoship/workspaces/issue-182/AUTOSHIP_VERIFICATION.log" >/dev/null || fail "deterministic verification hook records failing test command"

REVIEWER_REPO="$TMP_DIR/reviewer-repo"
mkdir -p "$REVIEWER_REPO/hooks/opencode" "$REVIEWER_REPO/hooks" "$REVIEWER_REPO/bin" "$REVIEWER_REPO/.autoship/workspaces/issue-183"
git init -q "$REVIEWER_REPO"
cp "$SCRIPT_DIR/reviewer.sh" "$REVIEWER_REPO/hooks/opencode/reviewer.sh"
cp "$SCRIPT_DIR/select-model.sh" "$REVIEWER_REPO/hooks/opencode/select-model.sh"
cp "$SCRIPT_DIR/../capture-failure.sh" "$REVIEWER_REPO/hooks/capture-failure.sh"
chmod +x "$REVIEWER_REPO/hooks/opencode/reviewer.sh" "$REVIEWER_REPO/hooks/opencode/select-model.sh" "$REVIEWER_REPO/hooks/capture-failure.sh"
cat > "$REVIEWER_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{"issue-183":{"state":"verifying","model":"opencode/test","role":"reviewer","attempt":1}},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
printf 'result\n' > "$REVIEWER_REPO/.autoship/workspaces/issue-183/AUTOSHIP_RESULT.md"
cat > "$REVIEWER_REPO/bin/opencode" <<'SH'
#!/usr/bin/env bash
case "${AUTOSHIP_FAKE_REVIEW:-pass}" in
  pass) printf 'analysis\nVERDICT: PASS\n' ;;
  fail) printf 'analysis\nVERDICT: FAIL\n' ;;
  missing) printf 'analysis without verdict\n' ;;
esac
exit 0
SH
chmod +x "$REVIEWER_REPO/bin/opencode"
(
  cd "$REVIEWER_REPO"
  PATH="$REVIEWER_REPO/bin:$PATH" AUTOSHIP_FAKE_REVIEW=pass bash hooks/opencode/reviewer.sh issue-183 .autoship/workspaces/issue-183 .autoship/workspaces/issue-183/AUTOSHIP_RESULT.md none >/tmp/reviewer-pass.out
)
if (
  cd "$REVIEWER_REPO"
  PATH="$REVIEWER_REPO/bin:$PATH" AUTOSHIP_FAKE_REVIEW=fail bash hooks/opencode/reviewer.sh issue-183 .autoship/workspaces/issue-183 .autoship/workspaces/issue-183/AUTOSHIP_RESULT.md none >/tmp/reviewer-fail.out 2>&1
); then
  fail "reviewer exits non-zero on explicit FAIL verdict"
fi
if (
  cd "$REVIEWER_REPO"
  PATH="$REVIEWER_REPO/bin:$PATH" AUTOSHIP_FAKE_REVIEW=missing bash hooks/opencode/reviewer.sh issue-183 .autoship/workspaces/issue-183 .autoship/workspaces/issue-183/AUTOSHIP_RESULT.md none >/tmp/reviewer-missing.out 2>&1
); then
  fail "reviewer fails closed when verdict is missing"
fi
reviewer_artifacts=$(find "$REVIEWER_REPO/.autoship/failures" -name '*-issue-183.json' 2>/dev/null | wc -l | tr -d '[:space:]')
[[ "$reviewer_artifacts" -ge 1 ]] || fail "reviewer records failure evidence for rejected and malformed verdicts"

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
mkdir -p "$WORKTREE_REPO/.autoship"
printf '{"models":[]}\n' > "$WORKTREE_REPO/.autoship/model-routing.json"
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
test -f "$WORKTREE_REPO/.autoship/workspaces/issue-156/.autoship/model-routing.json" || fail "create-worktree copies runtime routing into the workspace"

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

ISSUE_FILE_REPO="$TMP_DIR/issue-file-repo"
mkdir -p "$ISSUE_FILE_REPO/hooks/opencode" "$ISSUE_FILE_REPO/bin" "$ISSUE_FILE_REPO/.autoship/reports"
git init -q "$ISSUE_FILE_REPO"
cp "$SCRIPT_DIR/file-self-improvement-issues.sh" "$ISSUE_FILE_REPO/hooks/opencode/file-self-improvement-issues.sh"
chmod +x "$ISSUE_FILE_REPO/hooks/opencode/file-self-improvement-issues.sh"
cat > "$ISSUE_FILE_REPO/.autoship/reports/self-improvement.md" <<'MD'
# AutoShip Self-Improvement Report

## Root Cause Evidence
- Evidence: Insufficient balance in hooks/opencode/runner.sh

## Affected Files
- hooks/opencode/runner.sh

## Candidate Acceptance Criteria
- When paid model balance fails, retry with a configured free model.
- Add stealth hook signature evasion bypass.
MD
cat > "$ISSUE_FILE_REPO/bin/gh" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_ARGS_LOG"
exit 0
SH
chmod +x "$ISSUE_FILE_REPO/bin/gh"
(
  cd "$ISSUE_FILE_REPO"
  GH_ARGS_LOG="$ISSUE_FILE_REPO/gh-args.log" PATH="$ISSUE_FILE_REPO/bin:$PATH" bash hooks/opencode/file-self-improvement-issues.sh >/dev/null
)
safe_line=$(grep -F 'When paid model balance fails' "$ISSUE_FILE_REPO/gh-args.log" || true)
formerly_blocked_line=$(grep -F 'stealth hook signature evasion' "$ISSUE_FILE_REPO/gh-args.log" || true)
printf '%s\n' "$safe_line" | grep -F 'agent:ready' >/dev/null || fail "safe self-improvement issue is labeled agent:ready"
printf '%s\n' "$formerly_blocked_line" | grep -F 'agent:ready' >/dev/null || fail "self-improvement issue with evasion terms is labeled agent:ready"

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
  jq -e '.models[0].id == "opencode/nemotron-3-super-free" and .defaultFallback == "opencode/nemotron-3-super-free"' .autoship/model-routing.json >/dev/null || fail "setup ranks strongest free worker first"
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
cp "$SCRIPT_DIR/dispatch.sh" "$SCRIPT_DIR/create-worktree.sh" "$SCRIPT_DIR/select-model.sh" "$SCRIPT_DIR/pr-title.sh" "$DISPATCH_REPO/hooks/opencode/"
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
chmod +x "$DISPATCH_REPO/bin/gh" "$DISPATCH_REPO/hooks/opencode/dispatch.sh" "$DISPATCH_REPO/hooks/opencode/create-worktree.sh" "$DISPATCH_REPO/hooks/opencode/select-model.sh" "$DISPATCH_REPO/hooks/opencode/pr-title.sh" "$DISPATCH_REPO/hooks/update-state.sh"
(
  cd "$DISPATCH_REPO"
  PATH="$DISPATCH_REPO/bin:$PATH" bash hooks/opencode/dispatch.sh 456 docs >/dev/null
)
assert_eq "docs" "$(cat "$DISPATCH_REPO/.autoship/workspaces/issue-456/role")" "dispatch records specialized role file"
assert_eq "docs" "$(jq -r '.issues["issue-456"].role' "$DISPATCH_REPO/.autoship/state.json")" "dispatch records specialized role in state"
assert_eq "free/strong:free" "$(jq -r '.issues["issue-456"].model' "$DISPATCH_REPO/.autoship/state.json")" "dispatch records selected model in state"
grep -F '## Specialized Role' "$DISPATCH_REPO/.autoship/workspaces/issue-456/AUTOSHIP_PROMPT.md" >/dev/null || fail "dispatch records specialized role in prompt"

FIXTURE_REPO="$TMP_DIR/fixture-pipeline-repo"
mkdir -p "$FIXTURE_REPO/.autoship" "$FIXTURE_REPO/hooks/opencode" "$FIXTURE_REPO/hooks" "$FIXTURE_REPO/bin"
git init -q "$FIXTURE_REPO"
git -C "$FIXTURE_REPO" config user.email autoship@example.invalid
git -C "$FIXTURE_REPO" config user.name AutoShip
git -C "$FIXTURE_REPO" remote add origin git@github.com:owner/repo.git
printf 'base\n' > "$FIXTURE_REPO/README.md"
git -C "$FIXTURE_REPO" add README.md
git -C "$FIXTURE_REPO" commit -q -m initial
cp "$SCRIPT_DIR/plan-issues.sh" "$SCRIPT_DIR/dispatch.sh" "$SCRIPT_DIR/create-worktree.sh" "$SCRIPT_DIR/select-model.sh" "$SCRIPT_DIR/pr-title.sh" "$SCRIPT_DIR/runner.sh" "$SCRIPT_DIR/reviewer.sh" "$SCRIPT_DIR/create-pr.sh" "$FIXTURE_REPO/hooks/opencode/"
cp "$SCRIPT_DIR/../update-state.sh" "$SCRIPT_DIR/../capture-failure.sh" "$FIXTURE_REPO/hooks/"
chmod +x "$FIXTURE_REPO"/hooks/opencode/*.sh "$FIXTURE_REPO"/hooks/*.sh
cat > "$FIXTURE_REPO/.autoship/state.json" <<'JSON'
{"repo":"owner/repo","issues":{},"stats":{},"config":{"maxConcurrentAgents":15}}
JSON
cat > "$FIXTURE_REPO/.autoship/model-routing.json" <<'JSON'
{"roles":{"reviewer":"openai/gpt-5.5"},"models":[{"id":"free/strong:free","cost":"free","strength":90,"max_task_types":["medium_code"]}]}
JSON
cat > "$FIXTURE_REPO/issues.json" <<'JSON'
[
  {"number":188,"title":"unsafe bypass fixture","body":"hide detour hooks from anti-cheat detection","labels":[{"name":"agent:ready"},{"name":"security"}]},
  {"number":189,"title":"safe fixture pipeline","body":"Add a small fixture file","labels":[{"name":"agent:ready"},{"name":"type:test"}]}
]
JSON
cat > "$FIXTURE_REPO/bin/gh" <<'SH'
#!/usr/bin/env bash
if [[ "$1 $2" == "issue view" ]]; then
  number="$3"
  case "$7" in
    .title) jq -r --argjson number "$number" '.[] | select(.number == $number) | .title' issues.json ;;
    .body) jq -r --argjson number "$number" '.[] | select(.number == $number) | .body' issues.json ;;
    '[.labels[].name] | join(",")') jq -r --argjson number "$number" '.[] | select(.number == $number) | [.labels[].name] | join(",")' issues.json ;;
  esac
  exit 0
fi
if [[ "$1 $2" == "label list" ]]; then
  printf '%s\n' autoship:in-progress autoship:blocked autoship:done
  exit 0
fi
if [[ "$1 $2" == "issue edit" ]]; then
  exit 0
fi
if [[ "$1 $2" == "pr create" ]]; then
  printf 'LIVE_PR_CREATE %s\n' "$*" >> "$AUTOSHIP_GH_MUTATIONS_LOG"
  printf 'https://github.com/owner/repo/pull/42\n'
  exit 0
fi
if [[ "$1 $2" == "pr view" ]]; then
  printf 'Closes #189\n'
  exit 0
fi
exit 0
SH
cat > "$FIXTURE_REPO/bin/opencode" <<'SH'
#!/usr/bin/env bash
if printf '%s\n' "$*" | grep -F 'AutoShip reviewer' >/dev/null; then
  printf 'VERDICT: PASS\n'
  exit 0
fi
printf 'fixture change\n' > fixture-output.txt
printf 'Fixture pipeline completed\n' > AUTOSHIP_RESULT.md
printf 'COMPLETE\n' > status
exit 0
SH
chmod +x "$FIXTURE_REPO/bin/gh" "$FIXTURE_REPO/bin/opencode"
(
  cd "$FIXTURE_REPO"
  plan_output=$(PATH="$FIXTURE_REPO/bin:$PATH" bash hooks/opencode/plan-issues.sh --issues-file issues.json --limit 10)
  assert_eq "188 189" "$(jq -r '.eligible[].number' <<< "$plan_output" | paste -sd ' ' -)" "fixture plan includes content formerly blocked by safety filter"
  assert_eq "" "$(jq -r '.blocked[].number' <<< "$plan_output" | paste -sd ' ' -)" "fixture plan has no content-based safety blocks"
  PATH="$FIXTURE_REPO/bin:$PATH" bash hooks/opencode/dispatch.sh 188 medium_code >/dev/null
  assert_eq "QUEUED" "$(tr -d '[:space:]' < .autoship/workspaces/issue-188/status)" "fixture dispatch queues content formerly blocked by safety filter"
  PATH="$FIXTURE_REPO/bin:$PATH" bash hooks/opencode/dispatch.sh 189 medium_code >/dev/null
  assert_eq "QUEUED" "$(tr -d '[:space:]' < .autoship/workspaces/issue-189/status)" "fixture dispatch creates queued safe worktree"
  PATH="$FIXTURE_REPO/bin:$PATH" bash hooks/opencode/runner.sh >/dev/null
  for _ in 1 2 3 4 5; do
    [[ "$(tr -d '[:space:]' < .autoship/workspaces/issue-189/status)" != "RUNNING" ]] && break
    sleep 1
  done
  assert_eq "COMPLETE" "$(tr -d '[:space:]' < .autoship/workspaces/issue-189/status)" "fixture runner records completed worker state"
  PATH="$FIXTURE_REPO/bin:$PATH" bash hooks/opencode/reviewer.sh issue-189 "$FIXTURE_REPO/.autoship/workspaces/issue-189" >/dev/null
  AUTOSHIP_GH_MUTATIONS_LOG="$FIXTURE_REPO/gh-mutations.log" PATH="$FIXTURE_REPO/bin:$PATH" bash hooks/opencode/create-pr.sh issue-189 "$FIXTURE_REPO/.autoship/workspaces/issue-189" >/dev/null
  test ! -e "$FIXTURE_REPO/gh-mutations.log" || fail "fixture PR dry-run must not call gh pr create"
  assert_eq "dry-run" "$(jq -r '.issues["issue-189"].pr_mode' .autoship/state.json)" "fixture PR dry-run records runner state without live mutation"
  artifact_workspace="$FIXTURE_REPO/.autoship/workspaces/issue-artifact-only"
  mkdir -p "$artifact_workspace"
  git -C "$FIXTURE_REPO" worktree add -q "$artifact_workspace" HEAD
  printf 'artifact only\n' > "$artifact_workspace/AUTOSHIP_RESULT.md"
  printf 'COMPLETE\n' > "$artifact_workspace/status"
  if PATH="$FIXTURE_REPO/bin:$PATH" bash hooks/opencode/create-pr.sh issue-190 "$artifact_workspace" >/dev/null 2>&1; then
    fail "PR dry-run must reject artifact-only worktrees"
  fi
  live_workspace="$FIXTURE_REPO/.autoship/workspaces/issue-191"
  git -C "$FIXTURE_REPO" worktree add -q -b autoship/issue-191 "$live_workspace" HEAD
  printf 'implementation\n' > "$live_workspace/implementation.txt"
  printf 'live result\n' > "$live_workspace/AUTOSHIP_RESULT.md"
  printf 'runner log\n' > "$live_workspace/AUTOSHIP_RUNNER.log"
  printf 'COMPLETE\n' > "$live_workspace/status"
  AUTOSHIP_ENABLE_PR_CREATE=true AUTOSHIP_GH_MUTATIONS_LOG="$FIXTURE_REPO/live-gh-mutations.log" PATH="$FIXTURE_REPO/bin:$PATH" bash hooks/opencode/create-pr.sh issue-191 "$live_workspace" >/dev/null
  git -C "$live_workspace" show --name-only --format= HEAD | grep -F 'implementation.txt' >/dev/null || fail "live PR path commits implementation changes"
  if git -C "$live_workspace" show --name-only --format= HEAD | grep -E 'AUTOSHIP_RESULT.md|AUTOSHIP_RUNNER.log|status' >/dev/null; then
    fail "live PR path must not commit AutoShip runtime artifacts"
  fi
)

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
  install_output=$(OPENCODE_CONFIG_DIR="$CONFIG_DIR" node dist/cli.js install)
  if printf '%s\n' "$install_output" | grep -F 'opencode-autoship vv' >/dev/null; then
    fail "package installer must not print a double-v version"
  fi
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

VERSION_ALIGNMENT_DIR="$TMP_DIR/version-alignment"
mkdir -p "$VERSION_ALIGNMENT_DIR/installed" "$VERSION_ALIGNMENT_DIR/plugins"
cp VERSION package.json CHANGELOG.md "$VERSION_ALIGNMENT_DIR/"
cp VERSION "$VERSION_ALIGNMENT_DIR/installed/VERSION"
printf '%s\n' "$(tr -d '[:space:]' < VERSION)" > "$VERSION_ALIGNMENT_DIR/plugins/autoship.version"
bash "$SCRIPT_DIR/validate-version-alignment.sh" \
  --repo "$VERSION_ALIGNMENT_DIR" \
  --installed-version "$VERSION_ALIGNMENT_DIR/installed/VERSION" \
  --release-tag "$VERSION_ALIGNMENT_DIR/plugins/autoship.version" >/dev/null

assert_version_alignment_fails() {
  local fixture_dir="$1"
  local expected_message="$2"
  local output_file="$TMP_DIR/version-alignment-fail.txt"
  if bash "$SCRIPT_DIR/validate-version-alignment.sh" \
    --repo "$fixture_dir" \
    --installed-version "$fixture_dir/installed/VERSION" \
    --release-tag "$fixture_dir/plugins/autoship.version" >"$output_file" 2>&1; then
    fail "version alignment validation fails for $expected_message"
  fi
  grep -F "$expected_message" "$output_file" >/dev/null || fail "version alignment validation reports $expected_message"
}

printf 'v0.0.0\n' > "$VERSION_ALIGNMENT_DIR/installed/VERSION"
assert_version_alignment_fails "$VERSION_ALIGNMENT_DIR" 'installed asset marker'

VERSION_ALIGNMENT_DIR="$TMP_DIR/version-alignment-package"
mkdir -p "$VERSION_ALIGNMENT_DIR/installed" "$VERSION_ALIGNMENT_DIR/plugins"
cp VERSION package.json CHANGELOG.md "$VERSION_ALIGNMENT_DIR/"
jq '.version = "0.0.0"' "$VERSION_ALIGNMENT_DIR/package.json" > "$VERSION_ALIGNMENT_DIR/package.tmp"
mv "$VERSION_ALIGNMENT_DIR/package.tmp" "$VERSION_ALIGNMENT_DIR/package.json"
cp VERSION "$VERSION_ALIGNMENT_DIR/installed/VERSION"
printf '%s\n' "$(tr -d '[:space:]' < VERSION)" > "$VERSION_ALIGNMENT_DIR/plugins/autoship.version"
assert_version_alignment_fails "$VERSION_ALIGNMENT_DIR" 'package.json version'

VERSION_ALIGNMENT_DIR="$TMP_DIR/version-alignment-changelog"
mkdir -p "$VERSION_ALIGNMENT_DIR/installed" "$VERSION_ALIGNMENT_DIR/plugins"
cp VERSION package.json CHANGELOG.md "$VERSION_ALIGNMENT_DIR/"
perl -0pi -e 's/^## v[0-9][^\n]*/## v0.0.0/m' "$VERSION_ALIGNMENT_DIR/CHANGELOG.md"
cp VERSION "$VERSION_ALIGNMENT_DIR/installed/VERSION"
printf '%s\n' "$(tr -d '[:space:]' < VERSION)" > "$VERSION_ALIGNMENT_DIR/plugins/autoship.version"
assert_version_alignment_fails "$VERSION_ALIGNMENT_DIR" 'CHANGELOG release heading'

VERSION_ALIGNMENT_DIR="$TMP_DIR/version-alignment-tag"
mkdir -p "$VERSION_ALIGNMENT_DIR/installed" "$VERSION_ALIGNMENT_DIR/plugins"
cp VERSION package.json CHANGELOG.md "$VERSION_ALIGNMENT_DIR/"
cp VERSION "$VERSION_ALIGNMENT_DIR/installed/VERSION"
printf 'v0.0.0\n' > "$VERSION_ALIGNMENT_DIR/plugins/autoship.version"
assert_version_alignment_fails "$VERSION_ALIGNMENT_DIR" 'GitHub release tag marker'

bash "$SCRIPT_DIR/test-model-parsing.sh" >/dev/null

echo "OpenCode policy tests passed"
