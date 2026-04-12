# Beacon Specification v2

Status: Design Complete — Ready for Implementation
Updated: 2026-04-11
Platform: Claude Code Plugin (macOS-first, tmux-native)

---

## 1. Overview

Beacon is a Claude Code plugin that provides autonomous multi-agent orchestration. It reads work from GitHub Issues, routes tasks to the best available AI CLI tool, verifies results through dedicated reviewer agents, and auto-merges approved work.

Opus is the sole decision-maker. It never reads or writes code.

---

## 2. Design Decisions

| #   | Decision           | Choice                                                      | Rationale                                                   |
| --- | ------------------ | ----------------------------------------------------------- | ----------------------------------------------------------- |
| 1   | Work discovery     | Hybrid: Discord webhooks + 10-min gh poll                   | Real-time reaction via existing infra + safety net          |
| 2   | Agent dispatch     | TeamCreate for Claude, tmux CLI for Codex/Gemini            | Native protocol where available, direct CLI otherwise       |
| 3   | Result capture     | BEACON_RESULT.md per worktree + git diff                    | Agent never reports via conversation — structured file only |
| 4   | State management   | .beacon/state.json + GitHub labels                          | Fast local access + durable recovery from GitHub            |
| 5   | Verification       | Dedicated Sonnet reviewer agent                             | Keeps Opus free for orchestration                           |
| 6   | Concurrency        | Dynamic, soft cap 20, hard cap 50                           | Opus decides based on workload and available quota          |
| 7   | Post-completion    | verify -> simplify -> verify -> PR -> monitor CI -> cleanup | Full quality pipeline                                       |
| 8   | Autoresearch       | Automatic for Claude agents only                            | Codex/Gemini can't use Claude Code plugins                  |
| 9   | Merge gates        | CI green (simple), CI + Sonnet review (medium/complex)      | Trust CI for simple, extra review for complex               |
| 10  | Repo scope         | Single repo per session                                     | Clean isolation, multi-repo via multiple tmux sessions      |
| 11  | UltraPlan          | Once at startup + at phase checkpoints                      | Master planning pass, not per-issue                         |
| 12  | Opus role          | Never reads/writes code                                     | Orchestration decisions only                                |
| 13  | Plugin format      | Multi-skill Claude Code plugin                              | Auto-update via marketplace                                 |
| 14  | Tmux layout        | Grid (tiled) not stacked                                    | Visual clarity at 20 panes                                  |
| 15  | PR review comments | Sonnet agent addresses automated reviewer feedback          | Copilot etc. leave comments, Beacon handles them            |

---

## 3. Tool Registry

### Supported Tools

| Tool   | Binary   | Models                                       | Subscription | Quota Tracking            |
| ------ | -------- | -------------------------------------------- | ------------ | ------------------------- |
| Claude | `claude` | Opus (orchestrator), Sonnet, Haiku (workers) | Max          | Per-model, generous       |
| Codex  | `codex`  | Spark, GPT                                   | $20/mo       | Separate quotas per model |
| Gemini | `gemini` | Flash/Pro                                    | $20/mo       | Single quota pool         |

### Tool Selection Matrix

| Complexity | Primary                      | Fallback 1           | Fallback 2                   |
| ---------- | ---------------------------- | -------------------- | ---------------------------- |
| Simple     | Claude Haiku                 | Gemini (if quota)    | Codex Spark (if quota)       |
| Medium     | Claude Sonnet                | Codex GPT (if quota) | Gemini (if quota)            |
| Complex    | Claude Sonnet + autoresearch | Codex GPT (if quota) | Re-slice into smaller issues |

Claude is the backbone. Codex and Gemini are tactical — always check quota before dispatch.

---

## 4. Startup Sequence

### 4.1 Prerequisites

- tmux session running in the target repo
- Claude Code launched inside the session
- `gh auth status` passes
- At least one AI CLI tool detected

### 4.2 Startup Flow

```
1. Validate environment (gh, tmux, git repo)
2. Detect available tools + check quota
3. Load .beacon/state.json or initialize fresh
4. Fetch open issues via gh
5. UltraPlan analysis:
   - Classify complexity per issue
   - Parse dependencies (blocks:/depends-on:)
   - Build dependency graph (topological sort)
   - Assign tools based on complexity + quota
   - Plan dispatch phases
   - Set checkpoints
   - Estimate concurrency
6. Display plan
7. Start CronCreate for 10-minute poll safety net
8. Listen for Discord webhook events (if --channels active)
9. Begin dispatching Phase 1
```

---

## 5. Dispatch Protocol

### 5.1 Claude Agents

- Create git worktree: `.beacon/workspaces/<issue-key>`
- Use TeamCreate for tmux visibility
- Dispatch prompt includes:
  - Full issue context + acceptance criteria
  - Instruction to use `/autoresearch:fix` (Sonnet, complex tasks)
  - Instruction to write `BEACON_RESULT.md` on completion
  - Instruction to NOT merge, push, or close issues
- All Claude workers run as Sonnet or Haiku (CLAUDE_CODE_SUBAGENT_MODEL=sonnet)

### 5.2 Codex/Gemini Agents

- Create git worktree (same as above)
- Write `BEACON_PROMPT.md` to worktree with full issue context
- Spawn tmux pane: `tmux split-window -t beacon -c <worktree-path>`
- Rebalance: `tmux select-layout -t beacon tiled`
- Send command: `tmux send-keys -t <pane> "<tool> -p \"$(cat BEACON_PROMPT.md)\"" Enter`
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
- On completion: read `BEACON_RESULT.md` + `git diff` from worktree (not pane output)

---

## 6. Post-Completion Pipeline

```
Agent completes → writes BEACON_RESULT.md
  ↓
Step 1: VERIFY
  Sonnet reviewer agent:
  - Read BEACON_RESULT.md
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
  gh pr create --label beacon
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

### 7.1 Local State: `.beacon/state.json`

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
      "worktree": ".beacon/workspaces/<key>",
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

- `beacon:in-progress` — agent working on it (yellow)
- `beacon:blocked` — all agents failed, needs human review (red)
- `beacon:paused` — orchestration halted, awaiting resume (orange)
- `beacon:done` — completed and merged (green)

On restart: reconcile local state with GitHub labels + PR status.

---

## 8. Work Discovery

### 8.1 Discord Webhooks (Real-Time)

GitHub repo webhook → Discord channel → Claude Code session (--channels flag).
Beacon parses the webhook embed to extract issue number and event type.
Triggers immediate dispatch evaluation.

### 8.2 Polling Safety Net (10-Minute Interval)

CronCreate fires every 10 minutes:

1. `gh issue list --state open` for full issue sync
2. Diff against known state
3. New issues → UltraPlan integration into existing plan
4. Closed issues → cancel running agents if applicable

### 8.3 Discord Command Channel

Beacon responds to Discord commands:

- "work on #42" → immediate dispatch
- "skip #15" → exclude from plan
- "pause" / "resume" → halt/restart dispatch loop
- "status" → post status summary to Discord

---

## 9. Tmux Layout

- Pane 0: Opus orchestrator (main)
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
- Codex/Gemini rely on Beacon's external retry loop

---

## 12. Plugin Structure

```
beacon/
├── .claude-plugin/
│   ├── plugin.json
│   └── marketplace.json
├── commands/
│   └── beacon.md               # /beacon start|status|stop|plan
├── skills/
│   ├── beacon/SKILL.md          # Core orchestration protocol
│   ├── beacon-dispatch/SKILL.md # Agent dispatch logic
│   ├── beacon-verify/SKILL.md   # Verification pipeline
│   └── beacon-status/SKILL.md   # Status display
├── agents/
│   ├── reviewer.md              # Sonnet verification reviewer
│   └── monitor.md               # CI/PR monitor
├── README.md
├── LICENSE
├── BEACON_SPEC.md               # This file
└── .gitignore
```

---

## 13. Recovery

### Session Restart

1. Read `.beacon/state.json`
2. Check tmux for surviving panes
3. Poll GitHub for current issue/PR state
4. Reconcile local state with GitHub labels
5. Resume from last checkpoint

### Context Compaction

When the session hits the 80% compaction threshold:

1. State file persists on disk (survives compaction)
2. After compaction, re-read `.beacon/state.json`
3. Resume orchestration without UltraPlan re-run (plan is in state file)

### Power Loss

- tmux session dies → all agents die
- On next start: rebuild state from GitHub labels + worktree existence
- Stale worktrees cleaned up on startup

---

## 14. Security

- Worktree paths must remain under `.beacon/workspaces/`
- Agent subprocess cwd must be the per-issue worktree path
- Never log GitHub tokens or CLI credentials
- Validate `gh auth status` before GitHub operations
- All git operations scoped to per-issue worktree branch
- Auto-merge only after GitHub CI checks pass (never bypassed)

---

## 15. Implementation Milestones

### M1: Foundation

- Plugin skeleton + marketplace registration
- /beacon command + core skill
- GitHub adapter (fetch issues, milestones, blockers)
- Tool detection + quota checking
- Git worktree manager
- State file management

### M2: Dispatch + Verification

- UltraPlan analysis (complexity, dependencies, tool assignment)
- Claude agent dispatch via TeamCreate
- Codex/Gemini dispatch via tmux CLI
- Sonnet reviewer agent
- Post-completion pipeline (verify → simplify → verify)
- PR creation + GitHub labels

### M3: Monitoring + Merge

- Sonnet monitor agent (CI, review comments, conflicts)
- Auto-merge logic (simple: CI green, medium/complex: + review)
- PR review comment handling (dispatch Sonnet to address)
- Worktree cleanup after merge
- CronCreate polling (10-minute interval)

### M4: Discord + Autoresearch

- Discord webhook event handling
- Discord command channel (work on, skip, pause, resume, status)
- Autoresearch integration for Claude agents
- Quota tracking refinement (Codex Spark/GPT split)

### M5: Polish + Dogfood

- Status display with quota bars
- Tmux grid layout optimization
- Error recovery + edge cases
- Run Beacon on its own repo with autoresearch
- Documentation + README badges
