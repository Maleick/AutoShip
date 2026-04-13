# AutoShip

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

- `commands/autoship.md` — `/autoship:autoship` help command
- `commands/start.md` — `/autoship:start` launch orchestration
- `commands/stop.md` — `/autoship:stop` graceful shutdown
- `commands/plan.md` — `/autoship:plan` dry-run issue analysis
- `skills/orchestrate/` — Core orchestration protocol (v3: Sonnet executor + Opus advisor)
- `skills/dispatch/` — Agent dispatch (third-party first, pipe-pane, status words)
- `skills/verify/` — Post-completion pipeline (verify, simplify, PR, merge)
- `skills/status/` — Status display with quota bars
- `skills/poll/` — GitHub issue sync safety net
- `agents/reviewer.md` — Sonnet verification reviewer
- `agents/monitor.md` — CI/PR monitor agent
- `agents/haiku-triage.md` — Haiku event interpreter (Monitor → event queue)
- `hooks/init.sh` — Initialize `.autoship/` directory and state file
- `hooks/detect-tools.sh` — Detect available AI CLI tools + quota
- `hooks/update-state.sh` — Update `.autoship/state.json` issue states and stats
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

1. Install: `/install-plugin /Users/maleick/Projects/AutoShip`
2. Run: `/autoship:plan` to test issue analysis without dispatching
3. Run: `/autoship:start` for full orchestration

### Key conventions

- Skills are markdown protocols, not code — they instruct Claude how to behave
- All state persists in `.autoship/state.json` (local) and GitHub labels (durable)
- Event queue in `.autoship/event-queue.json` — Haiku writes, Sonnet reads
- Sonnet orchestrates; Opus is called only at strategic decision points
- Every agent emits `COMPLETE`, `BLOCKED`, or `STUCK` as its final line
- Every agent writes `AUTOSHIP_RESULT.md` — never trust conversation output

## Commands

| Command          | Purpose                               |
| ---------------- | ------------------------------------- |
| `/autoship:start`  | Launch orchestration for current repo |
| `/autoship:status` | Show running agents, quota, progress  |
| `/autoship:stop`   | Gracefully stop all agents            |
| `/autoship:plan`   | Analyze issues without dispatching    |

See AUTOSHIP_SPEC.md for the full specification.
