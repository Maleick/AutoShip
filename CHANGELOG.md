# Changelog

## v1.6.1

- Sync full OpenCode plugin assets from the latest GitHub release, not just `autoship.ts`.
- Keep installed hooks, commands, skills, AGENTS guidance, and version aligned under `~/.config/opencode/.autoship/`.
- Preserve the 15-worker default when model routing is auto-generated.

## v1.6.0

- OpenCode-only runtime.
- Live model discovery from `opencode models`.
- `openai/gpt-5.5` planner/coordinator/orchestrator/reviewer roles.
- Free worker models by default, with operator-selected Spark, Go-provider, Nvidia, OpenRouter, and other OpenCode models allowed when available.
- Learned worker model selection using task fit, configured strength, cost class, and prior success/failure history.
- Default active worker cap increased to 15.
- Unsafe/evasion-prone issues block for human review.
- Removed stale runtime, assistant, and pre-rename references from docs and public pages.
