---
name: autoship-verify
description: Post-completion verification pipeline for OpenCode — review, simplify, re-verify, PR creation, CI monitoring, cleanup
platform: opencode
tools: ["Bash", "Agent", "Read", "Write"]
---

# AutoShip Verification Pipeline — OpenCode Port

Invoked after an agent reports COMPLETE. Runs verify → simplify → verify → PR → monitor → cleanup.

---

## Step 0: Discover Test Command

Check in order:
1. `.autoship/config.json` → `test_command`
2. `package.json` → `npm test` / `yarn test`
3. `Makefile` → `make test`
4. `Cargo.toml` → `cargo test`
5. `pyproject.toml` → `pytest`
6. `go.mod` → `go test ./...`
7. If none found: skip test step

---

## Step 0.5: Pre-Verification Guards

Before spawning reviewer, run these assertions:

### 1. Path canonicalization

```bash
if [[ ! -e "$AUTOSHIP_RESULT_PATH" ]]; then
  echo "VERDICT: FAIL — AUTOSHIP_RESULT.md missing"
  exit 1
fi
REAL_RESULT=$(realpath "$AUTOSHIP_RESULT_PATH" 2>/dev/null)
REAL_WORKTREE=$(realpath "$WORKTREE_PATH" 2>/dev/null)
if [[ "$REAL_RESULT" != "$REAL_WORKTREE"/* ]]; then
  echo "VERDICT: FAIL — path outside worktree"
  exit 1
fi
```

### 2. Non-empty diff

```bash
DIFF_OUTPUT=$(git -C "$WORKTREE_PATH" diff main...HEAD 2>/dev/null)
if [[ -z "$DIFF_OUTPUT" ]]; then
  echo "VERDICT: FAIL — git diff is empty"
  exit 1
fi
```

---

## Step 1: Initial Verification

Spawn reviewer agent:

```
Agent({
  model: "<configured OpenCode reviewer model>",
  prompt: "You are an AutoShip verification reviewer. Evaluate whether the agent's work meets acceptance criteria.

## Issue: <issue-title>

## Acceptance Criteria
<criteria>

## Result File
$(cat <result-path>)

## Diff
$(git -C <worktree-path> diff main...HEAD)

## Test Command
<test-command>

## Process
1. Validate AUTOSHIP_RESULT.md exists
2. Review the diff against acceptance criteria
3. Run tests if command provided
4. Cross-reference claims in result file

## Output Format
VERDICT: PASS | FAIL
CONFIDENCE: HIGH | MEDIUM | LOW
REASON: <explanation>
FILES_CHANGED: <count>
TEST_RESULT: PASS | FAIL | SKIPPED | ERROR
SPECIFIC_ISSUES:
  - <issue 1>
  - <issue 2>
ACCEPTANCE_CRITERIA_MET:
  - [x] <criterion 1>
  - [ ] <criterion 2>",
  description: "AutoShip reviewer: <issue-key>"
})
```

---

## Step 2: On FAIL

- Attempt < 2: Re-dispatch with failure context appended
- Attempt >= 2: Retry on a stronger configured OpenCode model
- Attempt >= 3: Mark blocked for human review

---

## Step 3: On PASS

### Simplify (Optional)

Create rollback point:
```bash
cd .autoship/workspaces/<issue-key>
git tag autoship-pre-simplify-<issue-key>
```

Get changed files:
```bash
SIMPLIFY_FILES=$(git -C .autoship/workspaces/<issue-key> diff --name-only main...HEAD)
```

Spawn an OpenCode worker to simplify. Must not break tests.

### Re-verify

Confirm simplification preserved correctness. On FAIL: rollback.

---

## Step 4: Create PR

```bash
cd .autoship/workspaces/<issue-key>

# Stage changed files only
CHANGED_FILES=$(git diff --name-only -z main...HEAD)
git add -- ${CHANGED_FILES}

# Create commit
git commit -m "feat: <issue-title> (#<number>)

Closes #<number>
Dispatched by AutoShip."

# Create PR
gh pr create \
  --title "$(bash hooks/opencode/pr-title.sh --issue <number>)" \
  --body "## Summary
<from AUTOSHIP_RESULT.md>

## Verification
- Tests: passing
- Reviewer: PASS

Closes #<number>

Dispatched by AutoShip." \
  --label autoship \
  --head autoship/issue-<number>
```

---

## Step 5: Monitor CI

```bash
# Wait for CI checks
gh pr view <number> --json statusCheckRollup

# For simple issues: auto-merge when CI passes
# For complex issues: spawn reviewer first
```

---

## Step 6: Cleanup

After merge:
```bash
git worktree remove .autoship/workspaces/<issue-key> --force
git branch -D autoship/issue-<number>
bash hooks/update-state.sh set-merged <issue-id>
gh issue close <number>
```

---

## Error Handling

| Error | Recovery |
|-------|----------|
| Missing result file | FAIL, re-dispatch |
| Empty diff | FAIL, re-dispatch |
| Tests fail | FAIL, re-dispatch with fix |
| PR creation fails | Retry once, then block |
| Merge conflict | Mark blocked for human review |
