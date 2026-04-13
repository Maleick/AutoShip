# Beacon

Autonomous multi-agent orchestration plugin for Claude Code.

## Architecture (v3 — Advisor + Monitor)

Four-tier model: Bash watches → Haiku thinks → Sonnet orchestrates → Opus advises

- **Executor**: Sonnet — event-driven orchestration, dispatch, verification pipeline
- **Advisor**: Opus — spawned at strategic decision points (UltraPlan, phase checkpoints, escalations)
- **Workers**: Third-party first (Codex/Gemini/Copilot) for simple/medium; Claude Haiku/Sonnet as fallback
- **Triage**: Haiku — interprets Monitor events, categorizes PR comments, queues actions
- **Reviewer**: Sonnet — verifies work against acceptance criteria
- **Monitors**: 3 bash scripts (agent 5s, PR 30s, issues 60s) via Monitor tool

## Prerequisites

- `jq` — JSON query tool, required for state updates and completion tracking
  - Install: `brew install jq`

## Plugin Structure

- `commands/beacon.md` — `/beacon:beacon` help command
- `commands/start.md` — `/beacon:start` launch orchestration
- `commands/stop.md` — `/beacon:stop` graceful shutdown
- `commands/plan.md` — `/beacon:plan` dry-run issue analysis
- `skills/beacon/` — Core orchestration protocol (v3: Sonnet executor + Opus advisor)
- `skills/beacon-dispatch/` — Agent dispatch (third-party first, pipe-pane, status words)
- `skills/beacon-verify/` — Post-completion pipeline (verify, simplify, PR, merge)
- `skills/beacon-status/` — Status display with quota bars
- `skills/beacon-poll/` — GitHub issue sync safety net
- `agents/reviewer.md` — Sonnet verification reviewer
- `agents/monitor.md` — CI/PR monitor agent
- `agents/haiku-triage.md` — Haiku event interpreter (Monitor → event queue)
- `hooks/beacon-init.sh` — Initialize `.beacon/` directory and state file
- `hooks/detect-tools.sh` — Detect available AI CLI tools + quota
- `hooks/update-state.sh` — Update `.beacon/state.json` issue states and stats
- `hooks/monitor-agents.sh` — Agent completion watcher (5s, via Monitor tool)
- `hooks/monitor-prs.sh` — PR CI/merge status watcher (30s, via Monitor tool)
- `hooks/monitor-issues.sh` — GitHub issue new/closed watcher (60s, via Monitor tool)
- `hooks/cleanup-worktree.sh` — Remove worktree, branch, close issue
- `hooks/sweep-stale.sh` — Clean up orphaned worktrees on startup

## Development

### Adding a new skill

1. Create `skills/<name>/SKILL.md` with frontmatter (name, description, tools)
2. Write the protocol as markdown instructions
3. Reference it from the orchestrator skill or command

### Testing locally

1. Install: `/install-plugin /Users/maleick/Projects/Beacon`
2. Run: `/beacon:plan` to test issue analysis without dispatching
3. Run: `/beacon:start` for full orchestration

### Key conventions

- Skills are markdown protocols, not code — they instruct Claude how to behave
- All state persists in `.beacon/state.json` (local) and GitHub labels (durable)
- Event queue in `.beacon/event-queue.json` — Haiku writes, Sonnet reads
- Sonnet orchestrates; Opus is called only at strategic decision points
- Every agent emits `COMPLETE`, `BLOCKED`, or `STUCK` as its final line
- Every agent writes `BEACON_RESULT.md` — never trust conversation output

## Commands

| Command          | Purpose                               |
| ---------------- | ------------------------------------- |
| `/beacon:start`  | Launch orchestration for current repo |
| `/beacon:status` | Show running agents, quota, progress  |
| `/beacon:stop`   | Gracefully stop all agents            |
| `/beacon:plan`   | Analyze issues without dispatching    |

See BEACON_SPEC.md for the full specification.
