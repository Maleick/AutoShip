# Beacon

Autonomous multi-agent orchestration plugin for Claude Code.

## Prerequisites

- **gh** — GitHub CLI (authenticated)
- **jq** — JSON processor for state file manipulation
- **tmux** — Terminal multiplexer for agent visibility
- **git** — Version control

## Architecture

- **Orchestrator**: Opus — makes all dispatch decisions, never reads/writes code
- **Workers**: Sonnet/Haiku (Claude), Codex Spark/GPT, Gemini Flash/Pro
- **Reviewer**: Sonnet agent — verifies all work against acceptance criteria
- **Monitor**: Sonnet agent — watches CI, PR comments, merge status

## Plugin Structure

- `commands/beacon.md` — `/beacon start|status|stop|plan` entry point
- `skills/beacon/` — Core orchestration protocol
- `skills/beacon-dispatch/` — Agent dispatch (worktrees, tmux, prompts)
- `skills/beacon-verify/` — Post-completion pipeline (verify, simplify, PR, merge)
- `skills/beacon-status/` — Status display with quota bars
- `agents/reviewer.md` — Sonnet verification reviewer
- `agents/monitor.md` — CI/PR monitor agent
- `hooks/beacon-init.sh` — Initialize `.beacon/` directory and state file
- `hooks/detect-tools.sh` — Detect available AI CLI tools (Claude/Codex/Gemini)
- `hooks/check-completion.sh` — Poll tmux for completed agent panes
- `hooks/update-state.sh` — Update `.beacon/state.json` issue states and stats

## Development

### Adding a new skill

1. Create `skills/<name>/SKILL.md` with frontmatter (name, description, tools)
2. Write the protocol as markdown instructions
3. Reference it from the orchestrator skill or command

### Testing locally

1. Install: `/install-plugin /Users/maleick/Projects/Beacon`
2. Run: `/beacon plan` to test issue analysis without dispatching
3. Run: `/beacon start` for full orchestration

### Key conventions

- Skills are markdown protocols, not code — they instruct Claude how to behave
- All state persists in `.beacon/state.json` (local) and GitHub labels (durable)
- Opus orchestrates only — never reads or writes code directly
- Every agent writes `BEACON_RESULT.md` — never trust conversation output

## Commands

| Command          | Purpose                               |
| ---------------- | ------------------------------------- |
| `/beacon start`  | Launch orchestration for current repo |
| `/beacon status` | Show running agents, quota, progress  |
| `/beacon stop`   | Gracefully stop all agents            |
| `/beacon plan`   | Analyze issues without dispatching    |

See BEACON_SPEC.md for the full specification.
