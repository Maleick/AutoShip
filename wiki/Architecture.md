# Architecture

<p align="center">
  <img src="https://raw.githubusercontent.com/Maleick/AutoShip/main/assets/autoship-banner.svg" width="600" alt="AutoShip" />
</p>

AutoShip uses an **Advisor + Monitor** pattern. Sonnet runs the event loop. Opus advises at strategic decision points. Haiku handles triage. Bash scripts watch for events.

---

## Four-Tier Model

| Tier       | Model  | Responsibility                                                        |
| ---------- | ------ | --------------------------------------------------------------------- |
| **Bash**   | —      | Raw polling, tmux pane tailing, file watching                         |
| **Haiku**  | Haiku  | Event interpretation, triage, PR comment categorization, simple tasks |
| **Sonnet** | Sonnet | Pipeline execution, dispatch decisions, complex work, reviewing       |
| **Opus**   | Opus   | UltraPlan, phase checkpoints, escalation decisions                    |

Each tier operates in its optimal domain. Bash doesn't burn tokens. Haiku doesn't make architectural decisions. Opus doesn't write code.

---

## Event Flow

```
GitHub Issues / PRs / Agent completions
        ↓
  Bash Monitor Scripts (3 processes)
  ├── monitor-agents.sh  — 5s poll, pane.log tail
  ├── monitor-prs.sh     — 30s poll, CI/merge status
  └── monitor-issues.sh  — 60s poll, new/closed issues
        ↓
  Haiku Triage Agent
  └── Interprets raw events → writes to .autoship/event-queue.json
        ↓
  Sonnet Orchestrator
  └── Pulls from queue → runs pipeline → dispatches agents
        ↓ (strategic decisions only)
  Opus Advisor
  └── UltraPlan, phase checkpoints, 2+ failures, LOW_CONFIDENCE
```

---

## Three Monitor Processes

| Monitor             | Poll Interval | Emits                                                          |
| ------------------- | ------------- | -------------------------------------------------------------- |
| `monitor-agents.sh` | 5 seconds     | `[AGENT_STATUS] key=issue-25 status=COMPLETE`                  |
| `monitor-prs.sh`    | 30 seconds    | `[PR_CI_PASS]`, `[PR_CI_FAIL]`, `[PR_CONFLICT]`, `[PR_MERGED]` |
| `monitor-issues.sh` | 60 seconds    | `[ISSUE_NEW]`, `[ISSUE_CLOSED]`                                |

Three separate monitors allows each to be tuned independently. A single monitor would either waste GitHub API quota (5s for everything) or delay agent detection (60s for everything).

---

## Agent Completion Detection

Agents emit a status word as their final output line:

| Word       | Meaning                                   | Next action             |
| ---------- | ----------------------------------------- | ----------------------- |
| `COMPLETE` | Work done, `AUTOSHIP_RESULT.md` written     | Run verify pipeline     |
| `BLOCKED`  | External blocker (permission, dependency) | Mark blocked, notify    |
| `STUCK`    | Agent attempted but cannot solve the task | Re-dispatch or escalate |

`monitor-agents.sh` pipes tmux pane output to `.autoship/workspaces/<key>/pane.log` and tails it for these words in real-time — no polling delay.

For third-party agents (Codex/Gemini/Grok): `pane_dead=1` + `AUTOSHIP_RESULT.md` exists = COMPLETE. `pane_dead=1` + no file = crash, re-dispatch.

---

## Dispatch Priority

Third-party tools are dispatched first to maximize external quota:

| Complexity | Primary                             | Fallback            |
| ---------- | ----------------------------------- | ------------------- |
| Simple     | Codex / Gemini / Grok (quota > 10%) | Claude Haiku        |
| Medium     | Codex / Gemini / Grok (quota > 10%) | Claude Sonnet       |
| Complex    | Claude Sonnet + autoresearch        | Sonnet retry → Opus |

Complex tasks always go to Claude — they need autoresearch, plugins, and the native verify pipeline.

---

## Opus Advisor Call Points

Opus is spawned (fresh context each time) at these hardcoded moments:

1. **UltraPlan** — Initial issue classification + dispatch phasing
2. **Phase checkpoint** — After each dispatch phase completes
3. **Repeated failure** — Same issue failed 2+ times
4. **LOW_CONFIDENCE verdict** — Reviewer uncertain, Opus decides
5. **New issue (live)** — `[ISSUE_NEW]` during session → classify + insert
6. **PR conflict** — `[PR_CONFLICT]` → resolution strategy

Sonnet can also escalate to Opus for ambiguous scope, conflicting acceptance criteria, or unexpected tool behavior.

---

## Post-Completion Pipeline

```
COMPLETE signal received
  → Read AUTOSHIP_RESULT.md + git diff
  → Spawn Sonnet reviewer (agents/reviewer.md)
  → PASS: spawn Sonnet simplifier → re-verify → create PR
  → FAIL (attempt 1): re-dispatch with failure context
  → FAIL (attempt 2+): Opus advisor decides
  → PR created → Monitor 2 watches CI
  → CI pass + simple: auto-merge
  → CI pass + complex: Sonnet review → merge
  → Merged: hooks/cleanup-worktree.sh
  → Metrics snapshot refresh (state + ledger + PR metadata)
```

Metrics should be derived from the machine-readable state files, not from chat logs. The merge/cleanup boundary is where AutoShip should update lifecycle counters, and any scheduled report should render the current backlog, merge latency, and token spend from `.autoship/state.json`, `.autoship/token-ledger.json`, and GitHub PR metadata.

---

## State Layers

| Layer              | Location                   | Survives restart? |
| ------------------ | -------------------------- | ----------------- |
| Orchestration plan | `.autoship/state.json`       | Yes (disk)        |
| Event queue        | `.autoship/event-queue.json` | Yes (disk)        |
| Metrics snapshot   | README/wiki or CI artifact  | Yes, if regenerated |
| Agent labels       | GitHub labels on issues    | Yes (GitHub)      |
| Pane state         | tmux (in-memory)           | No                |
| Monitor processes  | tmux / Monitor tool        | No — restart them |

On session restart: read state.json, reconcile GitHub labels, re-initialize event queue, restart 3 Monitor processes. Metrics automation should treat `.autoship/state.json` as the lifecycle source of truth and `.autoship/token-ledger.json` as the spend source of truth; higher-level reporting fields can be rendered into the snapshot layer without changing the conversation flow.

---

## File Map

```
orchestrate/
  skills/
    orchestrate/SKILL.md                 Sonnet executor + Opus advisor protocol
    dispatch/SKILL.md        Dispatch (third-party first, pipe-pane)
    verify/SKILL.md          Verify → simplify → PR pipeline
    status/SKILL.md          /autoship:status display
    poll/SKILL.md            GitHub issue sync (safety net)
    discord-webhook/SKILL.md Parse GitHub webhook embeds from Discord
    discord-commands/SKILL.md Operator control via Discord
  agents/
    reviewer.md                     Sonnet verification agent
    monitor.md                      CI/PR watcher agent
    haiku-triage.md                 Haiku event interpreter
  hooks/
    init.sh                  Initialize .autoship/ workspace
    detect-tools.sh                 Detect AI CLIs + quota
    update-state.sh                 State machine transitions + GitHub labels
    monitor-agents.sh               Agent completion watcher (5s)
    monitor-prs.sh                  PR CI/merge watcher (30s)
    monitor-issues.sh               GitHub issue watcher (60s)
    cleanup-worktree.sh             Post-merge cleanup
    sweep-stale.sh                  Orphaned worktree cleanup
  commands/
    autoship.md                     /autoship:autoship (help)
    start.md                        /autoship:start
    stop.md                         /autoship:stop
    plan.md                         /autoship:plan
```
