# AutoShip Specification v2 → v3

Status: v3 Architecture Locked — Implementation In Progress
Updated: 2026-04-12
Platform: Claude Code Plugin (macOS-first, tmux-native)

> **v3 Update:** Architecture redesigned to Advisor + Monitor pattern. See `AUTOSHIP_ARCHITECTURE.md` for the full v3 spec. This document is updated below to reflect v3 decisions; the original v2 design is preserved for reference.

---

## 1. Overview

AutoShip is a Claude Code plugin that provides autonomous multi-agent orchestration. It reads work from GitHub Issues, routes tasks to the best available AI CLI tool, verifies results through dedicated reviewer agents, and auto-merges approved work.

**v3 model:** Sonnet is the executor and event-driven orchestrator. Opus is the advisor — spawned only at strategic decision points (UltraPlan, phase checkpoints, escalations). Haiku handles lightweight event triage and simple tasks. Bash scripts run the Monitor processes.

---

## 2. Design Decisions

### v2 Decisions (original)

| #   | Decision           | Choice                                                 | Rationale                                                   |
| --- | ------------------ | ------------------------------------------------------ | ----------------------------------------------------------- |
| 1   | Work discovery     | Hybrid: Discord webhooks + 10-min gh poll              | Real-time reaction via existing infra + safety net          |
| 2   | Agent dispatch     | TeamCreate for Claude, tmux CLI for Codex/Gemini       | Native protocol where available, direct CLI otherwise       |
| 3   | Result capture     | AUTOSHIP_RESULT.md per worktree + git diff             | Agent never reports via conversation — structured file only |
| 4   | State management   | .autoship/state.json + GitHub labels                   | Fast local access + durable recovery from GitHub            |
| 5   | Verification       | Dedicated Sonnet reviewer agent                        | Keeps Opus free for orchestration                           |
| 6   | Concurrency        | Dynamic, soft cap 20, hard cap 50                      | Opus decides based on workload and available quota          |
| 7   | Post-completion    | verify → simplify → verify → PR → monitor CI → cleanup | Full quality pipeline                                       |
| 8   | Autoresearch       | Automatic for Claude agents only                       | Codex/Gemini can't use Claude Code plugins                  |
| 9   | Merge gates        | CI green (simple), CI + Sonnet review (medium/complex) | Trust CI for simple, extra review for complex               |
| 10  | Repo scope         | Single repo per session                                | Clean isolation, multi-repo via multiple tmux sessions      |
| 11  | UltraPlan          | Once at startup + at phase checkpoints                 | Master planning pass, not per-issue                         |
| 12  | Opus role          | Never reads/writes code                                | Orchestration decisions only                                |
| 13  | Plugin format      | Multi-skill Claude Code plugin                         | Auto-update via marketplace                                 |
| 14  | Tmux layout        | Grid (tiled) not stacked                               | Visual clarity at 20 panes                                  |
| 15  | PR review comments | Sonnet agent addresses automated reviewer feedback     | Copilot etc. leave comments, AutoShip handles them          |

### v3 Decisions (locked — supersede v2 where different)

| #   | Decision                 | Choice                                                           |
| --- | ------------------------ | ---------------------------------------------------------------- |
| 1   | Orchestrator             | Sonnet (event-driven executor, not Opus)                         |
| 2   | Opus role                | Advisor only — spawned at hardcoded triggers + Sonnet escalation |
| 3   | Event detection          | 3 Monitor scripts (5s agents, 30s PRs, 60s issues)               |
| 4   | Dispatch priority        | Third-party first (Codex/Gemini) for simple/medium               |
| 5   | Haiku scope              | Simple tasks (2-3 files) + event triage + nit fixing             |
| 6   | Haiku failure escalation | 1 retry with context, then promote to Sonnet                     |
| 7   | Agent completion signal  | COMPLETE/BLOCKED/STUCK status words via pipe-pane log            |
| 8   | Third-party completion   | pane_dead + AUTOSHIP_RESULT.md existence (exit codes unreliable) |
| 9   | Event queue pattern      | Haiku produces → Sonnet consumes after each pipeline step        |
| 10  | PR comment triage        | Haiku (nits) → Sonnet (bugs) → Opus (design)                     |
| 11  | CI autofix               | Tiered: Haiku (lint/format) → Sonnet (logic) → Opus (2+ fails)   |

---

## 3. Tool Registry

### Supported Tools

| Tool   | Binary   | Models                                  | Subscription | Quota Tracking            |
| ------ | -------- | --------------------------------------- | ------------ | ------------------------- |
| Claude | `claude` | Opus (advisor), Sonnet, Haiku (workers) | Max          | Per-model, generous       |
| Codex  | `codex`  | Spark, GPT                              | $20/mo       | Separate quotas per model |
| Gemini | `gemini` | Flash/Pro                               | $20/mo       | Single quota pool         |

### Tool Selection Matrix

> **v3 update:** Third-party tools dispatch first to maximize external quota usage. Claude is the fallback and sole option for complex work.

| Complexity | Primary (v3)                    | Fallback              | Last resort              |
| ---------- | ------------------------------- | --------------------- | ------------------------ |
| Simple     | Codex/Gemini/Grok (quota > 10%) | Claude Haiku          | Claude Haiku (rate-lim)  |
| Medium     | Codex/Gemini/Grok (quota > 10%) | Claude Sonnet         | Claude Sonnet (rate-lim) |
| Complex    | Claude Sonnet + autoresearch    | Claude Sonnet (retry) | Opus advisor: re-slice   |

Claude is always available (Max subscription). Codex and Gemini are prioritized for volume work to burn external quota first.

---

## 4. Startup Sequence

### 4.1 Prerequisites

- tmux session running in the target repo
- Claude Code launched inside the session
- `gh auth status` passes
- At least one AI CLI tool detected

### 4.2 Startup Flow

> **v3 update:** CronCreate replaced by three Monitor processes. Opus advisor spawned for UltraPlan instead of running it directly.

```
1. Validate environment (gh, tmux, git repo)
2. Detect available tools + check quota (hooks/detect-tools.sh)
3. Load .autoship/state.json or run hooks/init.sh
4. Fetch open issues via gh
5. → ADVISOR CALL: Spawn Opus for UltraPlan
   - Classify complexity per issue
   - Parse dependencies (blocks:/depends-on:)
   - Build dependency graph (topological sort)
   - Assign tools based on complexity + quota (third-party first)
   - Plan dispatch phases + checkpoints
   - Returns structured plan JSON → stored in .autoship/state.json
6. Start 3 Monitor processes (agents 5s, PRs 30s, issues 60s)
7. Initialize event queue: .autoship/event-queue.json
8. Begin dispatching Phase 1 (third-party tools first)
9. Enter reactive mode — Haiku queues events, Sonnet pulls + acts
```

---

## 5. Dispatch Protocol

### 5.1 Claude Agents

- Create git worktree: `.autoship/workspaces/<issue-key>`
- Use TeamCreate for tmux visibility
- Dispatch prompt includes:
  - Full issue context + acceptance criteria
  - Instruction to use `/autoresearch:fix` (Sonnet, complex tasks)
  - Instruction to write `AUTOSHIP_RESULT.md` on completion
  - Instruction to NOT merge, push, or close issues
- All Claude workers run as Sonnet or Haiku (CLAUDE_CODE_SUBAGENT_MODEL=sonnet)

### 5.2 Codex/Gemini Agents

- Create git worktree (same as above)
- Write `AUTOSHIP_PROMPT.md` to worktree with full issue context
- Spawn tmux pane: `tmux split-window -t autoship -c <worktree-path>`
- Rebalance: `tmux select-layout -t autoship tiled`
- Send command: `tmux send-keys -t <pane> "<tool> -p \"$(cat AUTOSHIP_PROMPT.md)\"" Enter`
- Title pane: `tmux select-pane -t <pane> -T "<TOOL>: <issue-key>"`

### 5.3 Quota Check

Before dispatching to Codex or Gemini:

1. Run tool's status command
2. Parse remaining quota
3. If < 10%, skip tool, fall through to next
4. Track Codex Spark and Codex GPT as separate quota pools
5. If all non-Claude tools exhausted, route everything through Claude

### 5.4 Completion Detection

- Claude agents: TeamCreate protocol signals completion natively
- Codex/Gemini: `tmux list-panes -F '#{pane_id} #{pane_dead}'` — pane_dead=1 means process exited
- On completion: read `AUTOSHIP_RESULT.md` + `git diff` from worktree (not pane output)

---

## 6. Post-Completion Pipeline

```
Agent completes → writes AUTOSHIP_RESULT.md
  ↓
Step 1: VERIFY
  Sonnet reviewer agent:
  - Read AUTOSHIP_RESULT.md
  - Read git diff
  - Run test suite
  - Compare to acceptance criteria
  - Return: PASS / FAIL / LOW_CONFIDENCE
  ↓ (FAIL → re-dispatch to different tool, max 3 attempts)
  ↓ (LOW_CONFIDENCE → Opus reviews with UltraPlan)
  ↓ (PASS → continue)

Step 2: SIMPLIFY
  Sonnet agent runs code-simplifier on changed files
  ↓

Step 3: RE-VERIFY
  Sonnet reviewer confirms simplification preserved correctness
  ↓ (FAIL → revert simplification, use pre-simplification code)

Step 4: CREATE PR
  gh pr create --label autoship
  ↓

Step 5: MONITOR
  Sonnet monitor agent watches:
  - CI status
  - Automated review comments (Copilot, etc.)
  - Merge conflicts
  ↓ (review comments → dispatch Sonnet to address them)
  ↓ (CI fail → report to Opus)
  ↓ (CI pass, simple → auto-merge)
  ↓ (CI pass, medium/complex → Sonnet review → merge)

Step 6: CLEANUP
  - git worktree remove
  - Remove GitHub labels
  - Update state file
  - Close issue if not auto-closed
```

---

## 7. State Management

### 7.1 Local State: `.autoship/state.json`

```json
{
  "version": 1,
  "repo": "owner/repo",
  "started_at": "ISO-8601",
  "updated_at": "ISO-8601",
  "plan": {
    "phases": [],
    "current_phase": 0,
    "checkpoint_pending": false
  },
  "issues": {
    "<id>": {
      "state": "unclaimed|claimed|running|verifying|approved|merged|blocked",
      "complexity": "simple|medium|complex",
      "agent": "claude-sonnet|claude-haiku|codex-spark|codex-gpt|gemini",
      "attempt": 1,
      "worktree": ".autoship/workspaces/<key>",
      "pane_id": "<tmux-pane-id>",
      "started_at": "ISO-8601",
      "attempts_history": []
    }
  },
  "tools": {
    "claude": { "status": "available", "quota_pct": 100 },
    "codex-spark": { "status": "available", "quota_pct": 100 },
    "codex-gpt": { "status": "available", "quota_pct": 100 },
    "gemini": { "status": "available", "quota_pct": 100 }
  },
  "stats": {
    "dispatched": 0,
    "completed": 0,
    "failed": 0,
    "blocked": 0
  }
}
```

### 7.2 GitHub Labels (Durable Recovery)

- `autoship:in-progress` — agent working on it (color: yellow)
- `autoship:blocked` — all agents failed, needs human review (color: red)
- `autoship:paused` — orchestration halted, awaiting resume (color: orange)
- `autoship:done` — completed and merged (color: green)

Labels are created automatically on first run. On restart: reconcile local state with GitHub labels + PR status.

---

## 8. Work Discovery

### 8.1 Discord Webhooks (Real-Time)

GitHub repo webhook → Discord channel → Claude Code session (--channels flag).
AutoShip parses the webhook embed to extract issue number and event type.
Triggers immediate dispatch evaluation.

### 8.2 Polling Safety Net (10-Minute Interval)

CronCreate fires every 10 minutes:

1. `gh issue list --state open` for full issue sync
2. Diff against known state
3. New issues → UltraPlan integration into existing plan
4. Closed issues → cancel running agents if applicable

### 8.3 Discord Command Channel

AutoShip responds to Discord commands:

- "work on #42" → immediate dispatch
- "skip #15" → exclude from plan
- "pause" / "resume" → halt/restart dispatch loop
- "status" → post status summary to Discord

---

## 9. Tmux Layout

- Pane 0: Sonnet executor (main) — v3; was Opus in v2
- Agent panes: `tmux select-layout tiled` after each spawn
- At 20 panes: roughly 5x4 grid
- Each pane titled: `<TOOL>: <issue-key>`
- Status line: updated on agent checkin events only (no polling)
- Format: `[Claude:OK] [Codex-S:40%] [Codex-G:70%] [Gemini:30%] | Active: 5 | Done: 12`

---

## 10. Concurrency

- Dynamic: Opus decides based on workload and available quota
- Soft cap: 20 concurrent agents
- Hard cap: 50 (safety valve)
- Scale down when tools are exhausted
- One agent per issue (no duplicate dispatch)
- Reuse pane slots after agent completion

---

## 11. Autoresearch Integration

Claude agents (Sonnet/Haiku) automatically invoke `/autoresearch:fix`:

- Modify → verify → keep/discard → repeat
- Converges on passing solution before reporting completion
- Only available for Claude agents (plugin ecosystem)
- Codex/Gemini rely on AutoShip's external retry loop

---

## 12. Plugin Structure

```
orchestrate/
├── .claude-plugin/
│   ├── plugin.json
│   └── marketplace.json
├── commands/
│   ├── autoship.md               # /autoship:orchestrate (help)
│   ├── start.md                # /autoship:start
│   ├── stop.md                 # /autoship:stop
│   └── plan.md                 # /autoship:plan
├── skills/
│   ├── orchestrate/SKILL.md          # Core orchestration protocol
│   ├── dispatch/SKILL.md # Agent dispatch logic
│   ├── verify/SKILL.md   # Verification pipeline
│   └── status/SKILL.md   # Status display
├── agents/
│   ├── reviewer.md              # Sonnet verification reviewer
│   └── monitor.md               # CI/PR monitor
├── README.md
├── LICENSE
├── AUTOSHIP_SPEC.md               # This file
└── .gitignore
```

---

## 13. Recovery

### Session Restart

1. Read `.autoship/state.json`
2. Check tmux for surviving panes
3. Poll GitHub for current issue/PR state
4. Reconcile local state with GitHub labels
5. Resume from last checkpoint

### Context Compaction

When the session hits the 80% compaction threshold:

1. State file persists on disk (survives compaction)
2. After compaction, re-read `.autoship/state.json`
3. Resume orchestration without UltraPlan re-run (plan is in state file)

### Power Loss

- tmux session dies → all agents die
- On next start: rebuild state from GitHub labels + worktree existence
- Stale worktrees cleaned up on startup

---

## 14. Security

- Worktree paths must remain under `.autoship/workspaces/`
- Agent subprocess cwd must be the per-issue worktree path
- Never log GitHub tokens or CLI credentials
- Validate `gh auth status` before GitHub operations
- All git operations scoped to per-issue worktree branch
- Auto-merge only after GitHub CI checks pass (never bypassed)

---

## 15. Implementation Milestones

### M1: Foundation ✅ Complete

- Plugin skeleton + marketplace registration
- /autoship command + core skill
- GitHub adapter (fetch issues, milestones, blockers)
- Tool detection + quota checking
- Git worktree manager
- State file management

### M2: Dispatch + Verification ✅ Complete

- UltraPlan analysis (complexity, dependencies, tool assignment)
- Claude agent dispatch via TeamCreate
- Codex/Gemini dispatch via tmux CLI
- Sonnet reviewer agent
- Post-completion pipeline (verify → simplify → verify)
- PR creation + GitHub labels

### M3: Monitoring + Merge ✅ Complete

- Sonnet monitor agent (CI, review comments, conflicts)
- Auto-merge logic (simple: CI green, medium/complex: + review)
- PR review comment handling (dispatch Sonnet to address)
- Worktree cleanup after merge
- CronCreate polling (10-minute interval)

### M4: Discord + Autoresearch 🔄 In Progress

- Discord webhook event handling (skills/discord-webhook/) — in progress
- Discord command channel (skills/discord-commands/) — in progress
- Autoresearch integration ✅ (included in v3 dispatch skill)
- Quota tracking refinement ✅ (detect-tools.sh Spark/GPT split)

### M5: Polish + Dogfood 📋 Planned

- Status display with quota bars
- Tmux grid layout optimization
- Error recovery + edge cases
- Run AutoShip on its own repo with autoresearch
- Documentation + README badges
