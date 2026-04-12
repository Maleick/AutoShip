---
name: beacon-dispatch
description: Agent dispatch protocol — worktree creation, prompt generation, tmux pane management, and quota-aware routing (third-party first)
tools: ["Bash", "Agent", "Write", "Read", "TeamCreate"]
---

# Beacon Dispatch Protocol — v3

Third-party tools (Codex/Gemini/Grok) are dispatched first for simple and medium issues to maximize external quota usage. Claude agents are reserved for complex work and fallback.

---

## Dispatch Priority Matrix

| Complexity | Primary                         | Fallback              | Last Resort              |
| ---------- | ------------------------------- | --------------------- | ------------------------ |
| Simple     | Codex/Gemini/Grok (quota > 10%) | Claude Haiku          | Claude Haiku (rate-lim)  |
| Medium     | Codex/Gemini/Grok (quota > 10%) | Claude Sonnet         | Claude Sonnet (rate-lim) |
| Complex    | Claude Sonnet + autoresearch    | Claude Sonnet (retry) | Opus advisor: re-slice   |

Check quota before dispatch:

```bash
# Read current quota from state
jq '.tools' .beacon/state.json

# Refresh if > 30 minutes old
bash hooks/detect-tools.sh | jq '.'
```

Skip any tool with `quota_pct < 10` or `status == "unavailable"`.

---

## Step 1: Create Worktree

```bash
ISSUE_KEY="issue-<number>"
git worktree add .beacon/workspaces/$ISSUE_KEY -b beacon/$ISSUE_KEY main
```

**If branch already exists (previous attempt):**

```bash
git worktree remove .beacon/workspaces/$ISSUE_KEY --force 2>/dev/null
git branch -D beacon/$ISSUE_KEY 2>/dev/null
git worktree add .beacon/workspaces/$ISSUE_KEY -b beacon/$ISSUE_KEY main
```

**If disk/lock failure:** Mark issue blocked, skip to next.

---

## Step 2: Set Up Pane Log (for real-time completion detection)

Before spawning any tmux-based agent, create the pane log file:

```bash
mkdir -p .beacon/workspaces/$ISSUE_KEY
touch .beacon/workspaces/$ISSUE_KEY/pane.log
```

After spawning the pane, attach pipe-pane:

```bash
tmux pipe-pane -t $PANE_ID "cat >> .beacon/workspaces/$ISSUE_KEY/pane.log"
```

Monitor 1 watches these log files for `COMPLETE`, `BLOCKED`, or `STUCK` on their own line.

---

## Step 3A: Dispatch Third-Party Agent (Codex/Gemini)

Write the prompt file:

```bash
cat > .beacon/workspaces/$ISSUE_KEY/BEACON_PROMPT.md << 'EOF'
Implement the following GitHub issue in this repository.

## Issue: #<number> — <title>

<full issue body>

## Acceptance Criteria

<parsed from issue body, or generated from description>

## Instructions

- Run tests after changes: `<test-command>`
- Work only in the scope of this issue
- Commit your changes to the current branch
- Do NOT push, merge, or close the issue

## When Finished

Write BEACON_RESULT.md in the repo root:

```

# Result: #<number> — <title>

## Status: DONE | PARTIAL | STUCK

## Changes Made

- <file>: <what changed and why>

## Tests

- Command: `<test-command>`
- Result: PASS | FAIL
- New tests added: yes/no

## Notes

<anything the reviewer should know>
```

When done, print exactly one of these words on its own line as your final output:
COMPLETE
BLOCKED
STUCK
EOF

````

Spawn tmux pane:

```bash
PANE_ID=$(tmux split-window -t beacon -c .beacon/workspaces/$ISSUE_KEY -P -F '#{pane_id}')
tmux select-layout -t beacon tiled
tmux select-pane -t $PANE_ID -T "<TOOL>: $ISSUE_KEY"
tmux pipe-pane -t $PANE_ID "cat >> .beacon/workspaces/$ISSUE_KEY/pane.log"
````

Send command:

```bash
# Codex
tmux send-keys -t $PANE_ID "codex -p \"$(cat BEACON_PROMPT.md)\" --auto-edit && echo COMPLETE || echo STUCK" Enter

# Gemini
tmux send-keys -t $PANE_ID "gemini -p \"$(cat BEACON_PROMPT.md)\" && echo COMPLETE || echo STUCK" Enter
```

Update state:

```bash
bash hooks/update-state.sh set-running <issue-id> agent=codex-spark pane_id=$PANE_ID
```

**Completion detection for third-party agents:**

- Monitor 1 tails `pane.log` for `COMPLETE`, `BLOCKED`, or `STUCK`
- As backup: if `pane_dead=1` and `BEACON_RESULT.md` exists → treat as COMPLETE
- If `pane_dead=1` and no `BEACON_RESULT.md` → crash, re-dispatch

---

## Step 3B: Dispatch Claude Haiku Agent (Simple)

Use TeamCreate for visibility:

```
TeamCreate({
  name: "beacon-<issue-key>",
  teammateMode: "auto"
})
```

Agent prompt template:

````markdown
You are a Beacon worker agent. Implement the following GitHub issue.

## Issue: #<number> — <title>

<full issue body>

## Acceptance Criteria

<parsed from issue body, or generated from description>

## Working Context

- Worktree: `.beacon/workspaces/<issue-key>`
- Branch: `beacon/<issue-key>`
- Base: `main`
- Test command: `<test-command>`

## Instructions

- Stay within the scope of this issue — do not modify unrelated files
- Run tests after making changes
- Commit your work to `beacon/<issue-key>`
- Do NOT push, merge, or close the issue
- When finished, write `BEACON_RESULT.md` (template below)

## BEACON_RESULT.md Template

```markdown
# Result: #<number> — <title>

## Status: DONE | PARTIAL | STUCK

## Changes Made

- <file>: <what changed and why>

## Tests

- Command: `<test-command>`
- Result: PASS | FAIL
- New tests added: yes/no

## Notes

<anything the reviewer should know>
```

When you are completely finished, print exactly one of these words on its own line as your final output:
COMPLETE
BLOCKED
STUCK
````

Dispatch:

```
Agent({
  model: "haiku",
  prompt: "<the prompt above>",
  team_name: "beacon-<issue-key>",
  mode: "auto"
})
```

Update state:

```bash
bash hooks/update-state.sh set-running <issue-id> agent=claude-haiku
```

---

## Step 3C: Dispatch Claude Sonnet Agent (Medium/Complex)

Same structure as Haiku, but with autoresearch and more context:

````markdown
You are a Beacon worker agent. Implement the following GitHub issue.

## Issue: #<number> — <title>

<full issue body>

## Acceptance Criteria

<parsed from issue body, or generated from description>

## Working Context

- Worktree: `.beacon/workspaces/<issue-key>`
- Branch: `beacon/<issue-key>`
- Base: `main`
- Test command: `<test-command>`
- Complexity: <medium | complex>

## Instructions

- Use `/autoresearch:fix` for iterative development: fix → verify → keep/discard → repeat
- Read related code before making changes — understand the context
- Run tests after making changes
- Commit your work to `beacon/<issue-key>`
- Do NOT push, merge, or close the issue
- When finished, write `BEACON_RESULT.md` (template below)

## BEACON_RESULT.md Template

```markdown
# Result: #<number> — <title>

## Status: DONE | PARTIAL | STUCK

## Changes Made

- <file>: <what changed and why>

## Tests

- Command: `<test-command>`
- Result: PASS | FAIL
- New tests added: yes/no

## Notes

<anything the reviewer should know>
```

When completely finished, print exactly one of these words on its own line:
COMPLETE
BLOCKED
STUCK
````

Dispatch:

```
Agent({
  model: "sonnet",
  prompt: "<the prompt above>",
  team_name: "beacon-<issue-key>",
  mode: "auto"
})
```

Update state:

```bash
bash hooks/update-state.sh set-running <issue-id> agent=claude-sonnet
```

---

## Haiku Failure Escalation

If Haiku fails verification:

- **Attempt 1 fail**: Re-dispatch Haiku with failure context appended:
  ```
  ## Previous Attempt Failed
  Reviewer verdict: FAIL
  Issues found: <SPECIFIC_ISSUES from reviewer output>
  Please address these specifically.
  ```
- **Attempt 2 fail**: Automatically escalate to Sonnet — no Opus consultation needed
- **Attempt 3+ fail (Sonnet)**: Spawn Opus advisor

Update attempt count in state:

```bash
bash hooks/update-state.sh set-running <issue-id> attempt=<N>
```
