---
routing:
  research: [opencode]
  docs: [opencode]
  simple_code: [opencode]
  medium_code: [opencode]
  complex: [opencode]
  mechanical: [opencode]
  ci_fix: [opencode]
  rust_unsafe: [opencode]
quota_thresholds:
  low: 10
  exhausted: 0
stall_timeout_ms: 300000
max_concurrent_agents: 20
---

# AutoShip Configuration

Routing matrix and quota thresholds for the AutoShip orchestration system.
OpenCode is the only worker runtime; model selection lives in `.autoship/model-routing.json`.

## Project Context

AutoShip automatically extracts project-specific conventions from `CLAUDE.md`, `AGENTS.md`, and `.autoship/config.json` to provide agents with language and platform-specific guidance.

This context is stored in `.autoship/project-context.md` and included in every agent dispatch prompt. To update the context, edit the source files and re-run `hooks/init.sh`.
