---
name: autoship-status
description: Show AutoShip orchestration status
platform: opencode
---

# /autoship-status — Status Dashboard

Display current AutoShip orchestration status.

Run the real status hook:

```bash
bash hooks/opencode/status.sh
```

## Run Status Skill

Invoke the `autoship-status` skill which reads:
- `.autoship/state.json` — issue states, stats
- `.autoship/quota.json` — tool quotas
- `.autoship/token-ledger.json` — token usage
- GitHub PR counts

## Output

```
═══════════════════════════════════════════
              AUTOSHIP STATUS
═══════════════════════════════════════════
Repo:         owner/repo
Uptime:       Xh Ym
Phase:        N/M

───────────────────────────────────────────
AGENTS (N active / 15 max)
───────────────────────────────────────────
  [OpenCode] #42 — Fix login validation
  [OpenCode] #45 — Add rate limiting
  ...

───────────────────────────────────────────
QUOTA
───────────────────────────────────────────
  OpenCode: provider-managed

───────────────────────────────────────────
PROGRESS
───────────────────────────────────────────
  Session:   Dispatched: N   Completed: N
  PRs open:  N   PRs merged: N
═══════════════════════════════════════════
```

## No Session

If no active session:
```
No active AutoShip session.
Run /autoship to start.
```
