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

**If file is missing**: Display "No active Beacon session. Run `/autoship:start` to begin." and stop.

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

where N comes from `state.json` → `stats.session_dispatched` (or `0` if not set). This accurately reflects session usage without fabricating a percentage.

**If file is corrupted** (invalid JSON): Display error and suggest recovery:

```
BEACON STATUS: ERROR
State file corrupted. Recovery options:
  1. Run `/autoship:start` to rebuild state from GitHub labels
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

Store results as `pr-open` and `pr_merged` for use in the PROGRESS section.

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

### Step 5: Gather Token Usage

Read token spend from `.beacon/token-ledger.json`. If the file is missing, display zeros for all models.

**Get the current session ID:**

```bash
SESSION_ID=$(jq -r '.sessions[-1].session_id' .beacon/token-ledger.json 2>/dev/null || echo "")
```

**Per-model session totals** — aggregate `tokens_used` for each agent in the current session:

```bash
jq -r --arg sid "$SESSION_ID" '
  .sessions[] | select(.session_id == $sid) |
  .issues[] | [.agent, (.tokens_used // 0)] | @tsv
' .beacon/token-ledger.json 2>/dev/null \
  | awk '{sum[$1]+=$2} END {for(a in sum) print a, sum[a]}'
```

**All-time total** — sum all `tokens_used` across every session:

```bash
jq '[.sessions[].issues[].tokens_used // 0] | add // 0' .beacon/token-ledger.json 2>/dev/null || echo 0
```

Store per-model counts keyed by agent name. Known models to always display (show `0` if no data):
`codex-spark`, `codex-gpt`, `claude-haiku`, `gemini`, `copilot`.

### Step 6: Render Output

**PROGRESS fields** — read from `state.json` → `stats`:

| Display label          | JSON key                    |
| ---------------------- | --------------------------- |
| Session Dispatched     | `session_dispatched`        |
| Session Completed      | `session_completed`         |
| All-time Dispatched    | `total_dispatched_all_time` |
| All-time Completed     | `total_completed_all_time`  |
| Failed                 | `failed`                    |
| Blocked                | `blocked`                   |

`session_*` counters reset to 0 on each `/autoship:start`. `total_*` counters grow monotonically across sessions.

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
  Session:   Dispatched: 4   Completed: 3   Failed: 1  Blocked: 0
  All-time:  Dispatched: 45  Completed: 40
  PRs open: 3  PRs merged: 5

═══ Token Usage ══════════════════════════
  codex-spark    12,400  tokens  (session)
  codex-gpt       8,200  tokens  (session)
  claude-haiku    3,100  tokens  (session)
  gemini              0  tokens  (session)
  copilot             0  tokens  (session)

  Session total:  23,700 tokens
  All-time total: 187,400 tokens
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

### Token Usage Rendering

Always display all five known models in this order: `codex-spark`, `codex-gpt`, `claude-haiku`, `gemini`, `copilot`. Show `0` for models with no recorded usage in the current session. Format token counts with comma thousands separators, right-aligned in an 8-character field. Label each row with `(session)` to distinguish from all-time.

The `Session total` line sums all per-model session values. The `All-time total` line comes from the all-time aggregation query and includes all past sessions.

If `.beacon/token-ledger.json` is missing, show all zeros:

```
═══ Token Usage ══════════════════════════
  codex-spark         0  tokens  (session)
  codex-gpt           0  tokens  (session)
  claude-haiku        0  tokens  (session)
  gemini              0  tokens  (session)
  copilot             0  tokens  (session)

  Session total:      0 tokens
  All-time total:     0 tokens
```

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
  Session:   Dispatched: 12  Completed: 12  Failed: 0  Blocked: 0
  All-time:  Dispatched: 68  Completed: 68
  PRs open: 0  PRs merged: 12
  All issues in current phase complete. Run checkpoint to continue.

═══ Token Usage ══════════════════════════
  codex-spark    45,200  tokens  (session)
  codex-gpt      31,800  tokens  (session)
  claude-haiku    9,400  tokens  (session)
  gemini          2,100  tokens  (session)
  copilot             0  tokens  (session)

  Session total:  88,500 tokens
  All-time total: 312,900 tokens
```
