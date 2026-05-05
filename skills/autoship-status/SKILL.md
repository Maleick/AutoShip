---
name: autoship-status
description: Display current AutoShip orchestration status — running agents, quota bars, progress, token usage
platform: opencode
tools: ["Bash", "Read"]
---

# AutoShip Status — OpenCode Port

Display the current state of the AutoShip orchestration session.

---

## Process

Prefer the real status hook:

```bash
bash hooks/opencode/status.sh
```

### Step 1: Read State File

```bash
cat .autoship/state.json 2>/dev/null
```

**If missing**: Display "No active AutoShip session. Run `/autoship` to begin."

Also read quota file:
```bash
cat .autoship/quota.json 2>/dev/null || echo '{}'
```

### Step 2: Query Active Agents

```bash
# Count running agents
jq '[.issues | to_entries[] | select(.value.state == "running")] | length' .autoship/state.json

# List running agents with details
jq -r '.issues | to_entries[] | select(.value.state == "running") | "\(.key): \(.value.agent // "unknown")"' .autoship/state.json
```

### Step 3: Fetch PR Counts

```bash
gh pr list --state open --json number --jq 'length'
gh pr list --state merged --json number --jq 'length'
```

### Step 4: Calculate Uptime

Read `started_at` from state.json:
```bash
started_at=$(jq -r '.started_at // empty' .autoship/state.json)
elapsed=$(($(date +%s) - $(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$started_at" +%s 2>/dev/null || echo 0)))
hours=$((elapsed / 3600))
minutes=$(((elapsed % 3600) / 60))
```

### Step 5: Gather Token Usage

```bash
cat .autoship/token-ledger.json 2>/dev/null || echo '{"sessions": []}'
```

---

## Output Format

```
═══════════════════════════════════════════
              AUTOSHIP STATUS
═══════════════════════════════════════════
Repo:         owner/repo
Uptime:       Xh Ym
Phase:        N/M (checkpoint: yes/no)

───────────────────────────────────────────
AGENTS (N active / 20 max)
───────────────────────────────────────────
  [OpenCode] #42 — Fix login validation    (12m)
  [OpenCode] #45 — Add rate limiting       (8m)

───────────────────────────────────────────
QUOTA
───────────────────────────────────────────
  OpenCode:    provider-managed

───────────────────────────────────────────
PROGRESS
───────────────────────────────────────────
  Session:   Dispatched: N   Completed: N   Failed: N
  All-time:  Dispatched: N   Completed: N
  PRs open:  N   PRs merged: N

═══════════════════════════════════════════
```

---

## Quota Bar Rendering

Each bar is 20 characters:
```bash
filled=$((quota_pct / 5))
bar=$(printf '%*s' "$filled" | tr ' ' '█')$(printf '%*s' $((20 - filled)) | tr ' ' '░')
```

Color hints:
- `> 50%`: show percentage
- `10-50%`: show "~N%"
- `< 10%`: show "LOW"
- `0%`: show "EXHAUSTED"

---

## Running Issues Detail

For each running issue, show:
- Issue key and title (truncated to 40 chars)
- Agent type
- Elapsed time since `started_at`

---

## Error States

If state file is corrupted:
```
AUTOSHIP STATUS: ERROR
State file corrupted. Recovery options:
  1. Run /autoship to rebuild state
  2. Delete .autoship/state.json and restart
```

---

## No Active Session

```
═══════════════════════════════════════════
              AUTOSHIP STATUS
═══════════════════════════════════════════
No active AutoShip session.

Run /autoship to start orchestration.
═══════════════════════════════════════════
```
