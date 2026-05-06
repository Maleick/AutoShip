# Changelog

## v2.2.0

- Release metadata bump for v2.2.0.

## v2.0.11

- Consolidate shared runtime configuration lookup for worker concurrency across dispatch, runner, and status scripts.
- Add shared TypeScript config and routing types, tightening CLI handling of malformed config and unsafe package asset errors.
- Remove stale legacy hook, placeholder release note page, and completed in-progress planning documents.
- Refresh OpenCode install docs around npm-first installation and trim stale comments from shell hooks.

## v2.0.10

- Rotate compatible free worker models deterministically by issue number so parallel dispatches do not overload one free provider.
- Classify bundled free Zen models as free even when model IDs do not include a `free` suffix, and include `opencode-go/*` models as low-cost subscription fallbacks.
- Prefer capable free or OpenCode Go Kimi/Kimmy/Ling 2.6-family role models from live OpenCode inventory instead of assuming `openai/gpt-5.5`.
- Prompt for orchestrator and reviewer models during first-run setup, with separate CLI/env overrides for each role.
- Route complex tasks without a strong compatible worker to the configured orchestrator model as an advisor fallback.
- Add worker prompt guardrails against repeating the same failing command loop.
- Refresh README, docs, wiki, commands, skills, and agent guidance for free-first OpenCode-only routing.

## v2.0.9

- Add AutoShip audit, dashboard, retry, cancel, clean, and apply workflows for issue-to-PR orchestration operations.
- Add safety guardrails for GitHub API retries, protected label classification, diff size limits, prompt sanitization, acceptance criteria extraction, worktree checksums, quota pauses, and anti-flake retries.
- Harden CI and verification behavior for monitor polling, setup without GitHub auth, package diagnostics, and no-check PR merge blocking.

## v2.0.8

- Refresh worker monitoring from `status.sh` so dead worker PIDs are marked stuck during normal status checks.
- Make worker timeout parsing portable across macOS and Linux and configurable with `workerTimeoutMs` / `stall_timeout_ms`.
- Harden monitor event queue creation and lock fallback so missing queues or non-GNU `flock` do not stop reconciliation.

## v2.0.7

- Make `opencode-autoship doctor` validate project-local `.autoship/config.json` and `.autoship/model-routing.json` instead of global installed assets.
- Update installed OpenCode skills and commands to call hooks from the installed AutoShip asset directory when used outside the AutoShip repo.
- Extend smoke coverage for installed-project `doctor` after installed-project initialization.

## v2.0.6

- Fix installed `init.sh` so running from `~/.config/opencode/.autoship` no longer deletes installed hooks, commands, skills, or plugins.
- Add smoke coverage for initializing a project through the installed OpenCode asset copy.

## v2.0.5

- Add `opencode-autoship --version` and `opencode-autoship -v` for global CLI version checks.

## v2.0.4

- Normalize npm package metadata so the global `opencode-autoship` binary is preserved during publish.

## v2.0.3

- Make global npm install the primary long-term install path across README, Pages, and wiki docs.
- Document `bunx` as the one-time/no-global install path.
- Remove stale Pages safety-policy copy from the model routing table.

## v2.0.2

- Remove content-based unsafe issue blockers from planning, dispatch, classification, and self-improvement issue filing.
- Rank default free worker models from the live OpenCode provider list before writing model routing.
- Package and install plugin assets under `.autoship/plugins`, initialize model history, and add diagnostics for OpenCode `Session not found` worker failures.

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
- Removed stale runtime, assistant, and pre-rename references from docs and public pages.
