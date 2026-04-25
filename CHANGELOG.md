# Changelog

## v2.0.1

- Fix package CLI version output so `VERSION` values that already include `v` do not print as `vv...`.
- Add package installer regression coverage for double-`v` version output.

## v2.0.0

- Package AutoShip as `opencode-autoship` with package-style install and doctor flows.
- Add first-run setup, free-model-first routing, frontier lead roles, specialized role assets, and package diagnostics.
- Add deterministic verification, reviewer verdict parsing, PR creation, event queue processing, retry/escalation state, and E2E dry-run coverage.
- Add structured failure artifacts, self-improvement reports, safe issue filing, stale worker detection, paid-model fallback, and hardened cleanup.
- Refresh README, GitHub Pages, wiki, command aliases, package publish checklist, version alignment checks, and npm pack verification.

## v1.6.2

- Fix OpenCode worker launches by clearing inherited parent OpenCode session environment variables.
- Mark workers stuck when they exit without writing a terminal workspace status.
- Load initialized runtime version from `VERSION` instead of a stale hardcoded value.
- Prevent zero-running status checks from failing under `set -euo pipefail`.
- Clear and validate AutoShip workspace artifacts so stale results do not pollute new runs.

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
