---
routing:
  research: [gemini, claude-haiku]
  docs: [gemini, claude-haiku]
  simple_code: [codex-spark, gemini]
  medium_code: [codex-gpt, claude-sonnet]
  complex: [claude-sonnet, codex-gpt]
  mechanical: [claude-haiku, gemini]
  ci_fix: [claude-haiku, gemini]
  rust_unsafe: [claude-haiku, claude-sonnet]
quota_thresholds:
  low: 10
  exhausted: 0
stall_timeout_ms: 300000
max_concurrent_agents: 20
---

# AutoShip Configuration

Routing matrix and quota thresholds for the AutoShip orchestration system.
Edit the front matter above to configure agent assignments per task type.

## Project Context

AutoShip automatically extracts project-specific conventions from `CLAUDE.md`, `AGENTS.md`, and `.autoship/config.json` to provide agents with language and platform-specific guidance.

This context is stored in `.autoship/project-context.md` and included in every agent dispatch prompt. To update the context, edit the source files and re-run `hooks/init.sh`.
