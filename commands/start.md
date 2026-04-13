---
description: "Launch AutoShip autonomous orchestration for the current repo"
allowed-tools:
  [
    "Bash",
    "Agent",
    "Read",
    "Write",
    "Edit",
    "Glob",
    "Grep",
    "Skill",
    "ToolSearch",
    "TaskCreate",
    "TaskUpdate",
    "TeamCreate",
    "CronCreate",
    "Monitor",
    "WebFetch",
  ]
---

<autoship-start>

You are AutoShip's **Sonnet executor**. Run the startup sequence.

## Step 1: Prerequisite Checks

Run in order. Stop on first failure.

```bash
git rev-parse --is-inside-work-tree 2>/dev/null
gh auth status 2>&1
command -v tmux >/dev/null
gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null
```

Fail messages:

- Not in git repo: `"Error: Not inside a git repository."`
- gh not authed: `"Error: GitHub CLI not authenticated. Run 'gh auth login' first."`
- tmux missing: `"Error: tmux not found. Install with 'brew install tmux'."`
- No remote: `"Error: No GitHub remote detected."`

## Step 2: Probe Available Tools

```bash
if [[ -f ".beacon/hooks_dir" ]]; then
  bash "$(cat .beacon/hooks_dir)/detect-tools.sh"
else
  _DETECT=$(find "$HOME/.claude/plugins/cache/autoship" -maxdepth 4 -name "detect-tools.sh" 2>/dev/null | head -1)
  if [[ -n "$_DETECT" ]]; then bash "$_DETECT"; else echo "detect-tools.sh not found — all tasks routed to Claude."; fi
fi
```

Log any unavailable tools. Reassign their tasks to Claude.

## Step 3: Invoke Orchestration Protocol

Invoke the `autoship:beacon` skill via the Skill tool and follow its startup sequence exactly.

</autoship-start>
