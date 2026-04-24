---
name: autoship-dispatch
description: OpenCode dispatch — creates worktrees, generates prompts, queues work, and starts workers through the runner
platform: opencode
tools: ["Bash", "Write", "Read"]
---

# AutoShip Dispatch Protocol — OpenCode Port

Configured free OpenCode models are dispatched first when they are capable of the task. Operator-selected models are used only when they exist in the live OpenCode model inventory.

---

## Dispatch Priority Matrix

| Complexity | Primary | Fallback | Last Resort |
| ---------- | ------- | -------- | ----------- |
| Simple | Live free OpenCode models | Operator-selected model | Human review |
| Medium | Free capable OpenCode models | Operator-selected model | Human review |
| Complex | Free capable OpenCode models | Operator-selected model | Human review |

---

## Step 0: Enforce Safety and Concurrency

Use the repo hooks before creating work:

```bash
bash hooks/opencode/safety-filter.sh <issue-number>
MAX=$(jq -r '.config.maxConcurrentAgents // .max_concurrent_agents // 10' .autoship/state.json)
```

Unsafe/evasion work must be blocked or marked human-required, not auto-dispatched.

## Step 1: Create Worktree

```bash
ISSUE_KEY="issue-<number>"
git worktree add .autoship/workspaces/$ISSUE_KEY -b autoship/$ISSUE_KEY main
```

If branch already exists:
```bash
git worktree remove .autoship/workspaces/$ISSUE_KEY --force 2>/dev/null
git branch -D autoship/$ISSUE_KEY 2>/dev/null
git worktree add .autoship/workspaces/$ISSUE_KEY -b autoship/$ISSUE_KEY main
```

---

## Step 2: Check OpenCode Runtime

```bash
opencode_available=$(jq -r '.opencode.available // false' .autoship/quota.json 2>/dev/null || echo false)
```

Model cost/capability routing comes from `.autoship/model-routing.json`, with free OpenCode models sorted before paid fallbacks.

---

## Step 3: Dispatch or Queue Agent

The persistent OpenCode path is:

```bash
bash hooks/opencode/dispatch.sh <issue-number> <task-type>
bash hooks/opencode/runner.sh
```

`dispatch.sh` creates the worktree and prompt, marks the issue queued in state, and writes `QUEUED` status. `runner.sh` starts queued workspaces up to the configured concurrency cap.

## Step 4: OpenCode Worker Execution

`runner.sh` starts queued workspaces with `opencode run --model <selected-model>`.

### Dispatch Prompt Template

```markdown
You are an AutoShip worker agent. Implement the following GitHub issue.

## Issue: #<number> — <title>

## Issue Body
<full issue body>

## Acceptance Criteria
<parsed from issue body>

## Project Context
$(cat .autoship/project-context.md 2>/dev/null || echo "No project context available.")

## Working Context
- Worktree: `.autoship/workspaces/<issue-key>`
- Branch: `autoship/<issue-key>`
- Base: `main`
- Test command: `<test-command>`

## Instructions
- Stay within the scope of this issue
- Run tests after making changes
- Commit your work to `autoship/<issue-key>`
- Do NOT push, merge, or close the issue

## When Finished
Write `AUTOSHIP_RESULT.md` to `.autoship/workspaces/<issue-key>/`

```markdown
# Result: #<number> — <title>

## Status: DONE | PARTIAL | STUCK

## Changes Made
- <file>: <what changed>

## Tests
- Command: `<test-command>`
- Result: PASS | FAIL

## Notes
<anything the reviewer should know>
```

Then write your status to `.autoship/workspaces/<issue-key>/status`:
- `COMPLETE` — if successful
- `BLOCKED` — if external dependency
- `STUCK` — if cannot solve

Print COMPLETE, BLOCKED, or STUCK as your final output.
```

## Step 4: Update State

```bash
# Update state
bash hooks/update-state.sh set-running <issue-id> agent=<agent-name>

# Update quota
bash hooks/quota-update.sh decrement <tool> <complexity>

# Initialize status file
echo "RUNNING" > .autoship/workspaces/<issue-key>/status
```

---

## Agent Completion Requirements

**Before printing COMPLETE/BLOCKED/STUCK, agents MUST:**

1. Commit all work to git:
   ```bash
   git add -A && git commit -m 'feat: <issue-title> (#<number>)'
   ```

2. Write `AUTOSHIP_RESULT.md`:
   ```bash
   cat > .autoship/workspaces/<issue-key>/AUTOSHIP_RESULT.md << 'EOF'
   # Result: #<number> — <title>
   ## Status: DONE | PARTIAL | STUCK
   ## Changes Made
   - <file>: <what changed>
   ## Tests
   - Result: PASS | FAIL
   ## Notes
   <notes>
   EOF
   ```

3. Write status file:
   ```bash
   echo "COMPLETE" > .autoship/workspaces/<issue-key>/status
   ```

---

## Failure Escalation

If an OpenCode worker fails verification:
- **Attempt 1 fail**: Re-dispatch with failure context
- **Attempt 2 fail**: Retry on a stronger configured OpenCode model
- **Attempt 3+ fail**: Mark blocked for human review

---

## Human Review Escalation

For unsafe, repeatedly failing, or ambiguous work, mark the issue blocked and require human review before another automated attempt.

---

## Concurrency Cap

Before dispatching, check running count:

```bash
RUNNING=$(jq '[.issues | to_entries[] | select((.value.state // .value.status) == "running")] | length' .autoship/state.json)
MAX=$(jq -r '.config.maxConcurrentAgents // .max_concurrent_agents // 10' .autoship/state.json)
if (( RUNNING >= MAX )); then
  echo "CAP_REACHED: $RUNNING agents running"
  # Queue for next poll cycle
  exit 0
fi
```
