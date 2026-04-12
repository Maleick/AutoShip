---
name: beacon-status
description: Display current Beacon orchestration status — running agents, quota, progress, and issue states
tools: ["Bash", "Read"]
---

# Beacon Status

Display the current state of the Beacon orchestration session.

## Process

### Step 1: Read State File

```bash
cat .beacon/state.json 2>/dev/null
```

**If file is missing**: Display "No active Beacon session. Run `/beacon:start` to begin." and stop.

Also read the quota file:

```bash
cat .beacon/quota.json 2>/dev/null || echo '{}'
```

`quota.json` has the structure:

```json
{
  "codex-spark": {
    "quota_pct": 40,
    "dispatches": 7,
    "reset_date": "2026-04-15"
  },
  "codex-gpt": { "quota_pct": 70, "dispatches": 3, "reset_date": "2026-04-15" },
  "gemini": { "quota_pct": 30, "dispatches": 5, "reset_date": "2026-04-15" }
}
```

Use this file — not `state.json .tools` — as the authoritative source for the QUOTA section.

For Claude's quota line: check if `quota.json` has a `claude` entry. If it does, render the bar from that entry's `quota_pct`. If no `claude` entry exists (the common case with Claude Max subscription), do **not** show a hardcoded full bar — instead display:

```
Claude:      Claude Max — N dispatches (session)
```

where N comes from `state.json` → `stats.claude_dispatches` (or `0` if not set). This accurately reflects session usage without fabricating a percentage.

**If file is corrupted** (invalid JSON): Display error and suggest recovery:

```
BEACON STATUS: ERROR
State file corrupted. Recovery options:
  1. Run `/beacon:start` to rebuild state from GitHub labels
  2. Delete .beacon/state.json and restart
```

### Step 2: Query Tmux

```bash
tmux list-panes -t beacon -F '#{pane_id} #{pane_title} #{pane_dead} #{pane_start_command}' 2>/dev/null
```

**If tmux session doesn't exist**: Note "tmux session 'beacon' not found" — agents may have died. Show state file data only.

### Step 2.5: Fetch PR Counts

```bash
gh pr list --state open --json number --jq 'length'
gh pr list --state merged --json number --jq 'length'
```

Store results as `pr_open` and `pr_merged` for use in the PROGRESS section.

### Step 3: Reconcile

Cross-reference state file entries with tmux panes:

- State says "running" but pane is dead → flag as "completed (unchecked)" or "crashed"
- Pane exists but no state entry → flag as "orphaned pane"

### Step 4: Calculate Uptime

Read `started_at` from `state.json` (ISO 8601 string). Calculate elapsed time from `started_at` to now:

```
elapsed_seconds = now - started_at
hours = floor(elapsed_seconds / 3600)
minutes = floor((elapsed_seconds % 3600) / 60)
uptime = "{h}h {m}m"
```

If `started_at` is missing, display `Uptime: unknown`.

### Step 5: Render Output

## Output Format

```
BEACON STATUS
─────────────
Repo: owner/repo
Uptime: Xh Ym
Phase: 2/4 (checkpoint pending: no)

AGENTS (3 active / 20 max)
  [Sonnet] #42 — Fix login validation     (12m)
  [Codex]  #45 — Add rate limiting         (8m)
  [Gemini] #48 — Update docs              (3m)

QUOTA
  Claude:      Claude Max — 4 dispatches (session)
  Codex Spark: ████████░░░░░░░░░░░░ ~40%  (7 dispatches, est.)
  Codex GPT:   ██████████████░░░░░░ ~70%  (3 dispatches, est.)
  Gemini:      ██████░░░░░░░░░░░░░░ ~30%  (5 dispatches, est.)

PROGRESS
  Dispatched: 12  Completed: 8  Failed: 1  Blocked: 0
  PRs open: 3  PRs merged: 5
```

### Quota Bar Rendering

Each bar is exactly 20 characters wide. Calculate filled blocks:

```
filled = round(quota_pct / 5)  # 100% = 20 blocks, 50% = 10 blocks
bar = "█" * filled + "░" * (20 - filled)
```

Color hints (for terminal output):

- `> 50%`: show as "available" or exact percentage
- `10-50%`: show as "~N%" (warning zone)
- `< 10%`: show as "LOW" (dispatch will skip this tool)
- `0%`: show as "EXHAUSTED"

Append `(N dispatches, est.)` from `quota.json` after the percentage label for each third-party tool. Use `est.` always for Codex and Gemini — neither CLI exposes a subscription quota API, so the value is a decay estimate. Claude has no dispatch count or "est." shown (Max subscription, always available). If `quota.json` is missing or a tool has no entry, omit the dispatch count for that tool.

### When No Agents Are Running

```
BEACON STATUS
─────────────
Repo: owner/repo
Uptime: Xh Ym
Phase: 3/4 (checkpoint pending: yes)

AGENTS (0 active / 20 max)
  No agents currently running.
  Next phase waiting on checkpoint review.

QUOTA
  <same format>

PROGRESS
  Dispatched: 12  Completed: 12  Failed: 0  Blocked: 0
  PRs open: 0  PRs merged: 12
  All issues in current phase complete. Run checkpoint to continue.
```
