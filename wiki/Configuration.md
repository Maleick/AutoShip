# Configuration

---

## `.beacon/state.json`

The primary runtime state file. Created by `hooks/beacon-init.sh`. Never edit directly — always use `hooks/update-state.sh`.

```json
{
  "version": 1,
  "repo": "owner/repo",
  "started_at": "2026-04-12T00:00:00Z",
  "updated_at": "2026-04-12T00:00:00Z",
  "plan": {
    "phases": [
      {
        "phase": 1,
        "issues": [
          { "number": 42, "complexity": "simple", "tool": "codex-spark" }
        ]
      }
    ],
    "current_phase": 1,
    "checkpoint_pending": false
  },
  "issues": {
    "42": {
      "state": "running",
      "complexity": "simple",
      "agent": "codex-spark",
      "attempt": 1,
      "worktree": ".beacon/workspaces/issue-42",
      "pane_id": "%3",
      "started_at": "2026-04-12T00:00:00Z",
      "attempts_history": []
    }
  },
  "tools": {
    "claude": { "status": "available", "quota_pct": 100 },
    "codex-spark": { "status": "available", "quota_pct": 74 },
    "codex-gpt": { "status": "available", "quota_pct": 61 },
    "gemini": { "status": "unavailable", "quota_pct": -1 }
  },
  "stats": {
    "dispatched": 5,
    "completed": 3,
    "failed": 1,
    "blocked": 0
  },
  "paused": false,
  "excluded_issues": []
}
```

### Issue States

| State       | Meaning                                 |
| ----------- | --------------------------------------- |
| `unclaimed` | In plan, not yet dispatched             |
| `claimed`   | Reserved for dispatch                   |
| `running`   | Agent actively working                  |
| `verifying` | Reviewer agent checking the work        |
| `approved`  | Passed verification, PR being created   |
| `merged`    | PR merged, cleanup complete             |
| `blocked`   | All attempts failed, needs human review |
| `paused`    | Orchestration halted                    |

---

## `.beacon/config.json`

Operator overrides. Created empty (`{}`) on first run. All fields optional.

```json
{
  "test_command": "npm test",
  "base_branch": "main",
  "max_concurrent": 10,
  "excluded_labels": ["wontfix", "invalid"],
  "auto_merge": true,
  "discord_channel_id": "1234567890"
}
```

| Field                | Default     | Description                                      |
| -------------------- | ----------- | ------------------------------------------------ |
| `test_command`       | auto-detect | Override test runner (e.g. `pytest`, `npm test`) |
| `base_branch`        | `main`      | Branch to base worktrees on                      |
| `max_concurrent`     | 20          | Soft cap on simultaneous agents                  |
| `excluded_labels`    | `[]`        | Issues with these labels are skipped             |
| `auto_merge`         | `true`      | Auto-merge simple issues after CI pass           |
| `discord_channel_id` | —           | Discord channel ID for webhook events + commands |

---

## `.beacon/event-queue.json`

Producer-consumer event queue. Haiku writes, Sonnet reads. Initialized as `[]`.

```json
[
  {
    "type": "verify",
    "issue": "issue-42",
    "priority": 2,
    "data": {},
    "queued_at": "2026-04-12T00:00:00Z"
  },
  {
    "type": "stuck",
    "issue": "issue-17",
    "priority": 1,
    "data": {},
    "queued_at": "2026-04-12T00:00:05Z"
  }
]
```

**Priority:** 1 = urgent (stuck/blocked/CI fail), 2 = normal, 3 = low (new issues)

Sonnet always processes priority 1 events first.

---

## GitHub Labels

Created automatically on first run in the target repo:

| Label                | Color  | Meaning                          |
| -------------------- | ------ | -------------------------------- |
| `beacon:in-progress` | Yellow | Agent actively working           |
| `beacon:blocked`     | Red    | Failed, needs human intervention |
| `beacon:paused`      | Orange | Orchestration halted             |
| `beacon:done`        | Green  | Completed and merged             |

Labels are the **durable recovery layer** — if `.beacon/state.json` is lost or stale, GitHub labels are the source of truth.

On restart:

- `beacon:in-progress` + worktree exists → resume running
- `beacon:in-progress` + no worktree → re-dispatch
- `beacon:blocked` → restore blocked state, notify operator
- `beacon:done` → verify PR was merged, clean up

---

## `.beacon/discord-last-seen.json`

Tracks the last-processed Discord message timestamp to avoid reprocessing commands or webhook events.

```json
{
  "webhook_channel": "2026-04-12T00:00:00Z",
  "command_channel": "2026-04-12T00:00:00Z"
}
```

---

## Environment

No shell environment variables are required at the project level. Global Claude Code settings at `~/.claude/settings.json` control model routing:

```json
{
  "env": {
    "CLAUDE_CODE_SUBAGENT_MODEL": "sonnet",
    "CLAUDE_CODE_DISABLE_1M_CONTEXT": "1",
    "CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING": "1",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-sonnet-4-6"
  }
}
```

These settings make Sonnet the default for all subagents while keeping Opus available for advisor calls.

---

## Prerequisites

```bash
# Required
brew install gh jq
gh auth login
brew install tmux   # for third-party agent dispatch

# Optional (expand tool roster)
npm install -g @openai/codex
# gemini CLI — see Google AI docs
```
