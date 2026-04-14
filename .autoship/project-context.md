## Project Context

### Project Configuration
- **test_command**: 
- **verify_command**: 

### Project Conventions (from CLAUDE.md)
## Gotchas

- **`.autoship/` is gitignored** — `state.json`, `event-queue.json`, and worktree dirs are runtime artifacts. Never commit them.
- **`AUTOSHIP_RESULT.md` / `AUTOSHIP_PROMPT.md`** — written per worktree, also gitignored. Orchestrator reads the file, never conversation output.
- **SessionStart hook failure** — if `hooks/activate.sh` errors, AutoShip silently won't initialize. Run `bash hooks/activate.sh` directly to debug.
- **Quota exhaustion** — Codex/Gemini have daily limits tracked in `detect-tools.sh`. If a dispatch fails unexpectedly, check quota before assuming a skill bug.
- **Skill cross-references** — skills reference each other by relative path. If you move or rename a skill file, grep for its name across all `.md` files before deleting.
- **`quota-update.sh` owns all `quota.json` mutations** — Never write `.exhausted`, `.tool_stuck_count`, or quota fields directly via `jq` in other hooks. Always call `bash hooks/quota-update.sh stuck <tool>` (3-strike threshold) or the relevant subcommand. Bypassing breaks event emission and stuck-count history.
- **awk on macOS uses BSD awk, not gawk** — `IGNORECASE = 1` is silently ignored. Use `tolower($0) ~` for case-insensitive matching in hook awk scripts.
- **`jq map_values` overwrites all keys** — Use `with_entries(if .key == "X" then . else .value = Y end)` when transforming routing keys but preserving specific ones (e.g. `rust_unsafe`).

### Agent Constraints (from AGENTS.md)
# AutoShip

Autonomous multi-agent orchestration plugin for Codex.

## Architecture (v3 — Advisor + Monitor)

Four-tier model: Bash watches → Haiku thinks → Sonnet orchestrates → Opus advises

- **Executor**: Sonnet — event-driven orchestration, dispatch, verification pipeline
- **Advisor**: Opus — spawned at strategic decision points (UltraPlan, phase checkpoints, escalations)
- **Workers**: Third-party first (Codex/Gemini/Copilot) for simple/medium; Codex Haiku/Sonnet as fallback
- **Triage**: Haiku — interprets Monitor events, categorizes PR comments, queues actions
- **Reviewer**: Sonnet — verifies work against acceptance criteria
- **Monitors**: 3 bash scripts (agent 5s, PR 30s, issues 60s) via Monitor tool

**Full specs:** `AUTOSHIP_SPEC.md` (design decisions, v2→v3 evolution) · `AUTOSHIP_ARCHITECTURE.md` (v3 architecture detail) · `AUTOSHIP.md` (operator runbook)

## Prerequisites

- `jq` — JSON query tool, required for state updates and completion tracking
  - Install: `brew install jq`

## Plugin Structure

- `commands/autoship.md` — `/autoship:autoship` help command
- `commands/start.md` — `/autoship:start` launch orchestration
- `commands/stop.md` — `/autoship:stop` graceful shutdown
- `commands/plan.md` — `/autoship:plan` dry-run issue analysis
- `skills/orchestrate/` — Core orchestration protocol (v3: Sonnet executor + O