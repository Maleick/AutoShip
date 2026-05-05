# [2.4.0](https://github.com/Maleick/AutoShip/compare/v2.3.0...v2.4.0) (2026-05-05)


### Features

* raise worker concurrency defaults ([#351](https://github.com/Maleick/AutoShip/issues/351)) ([1343475](https://github.com/Maleick/AutoShip/commit/13434758e0775ebacdf47112c9d53d2977ece068))

# [2.3.0](https://github.com/Maleick/AutoShip/compare/v2.2.1...v2.3.0) (2026-05-05)

## v2.3.0


### Bug Fixes

* **autoship:** harden release and Hermes runtime ([7cc644a](https://github.com/Maleick/AutoShip/commit/7cc644a96db8fad50c81f70eec492246e28f6714))
* **hermes:** add gh CLI instructions to HERMES_PROMPT.md ([#318](https://github.com/Maleick/AutoShip/issues/318)) ([7e90063](https://github.com/Maleick/AutoShip/commit/7e90063f9485fdada8d849bb289bc0b234e84608))
* **hermes:** correct workspace counting in auto-prune and protect queued workspaces ([3cedb63](https://github.com/Maleick/AutoShip/commit/3cedb63b3cb014fc9d7887babe3cc12bf1a66005))
* **hermes:** correct workspace counting in auto-prune and protect queued workspaces ([#329](https://github.com/Maleick/AutoShip/issues/329)) ([2e84802](https://github.com/Maleick/AutoShip/commit/2e8480256ebbbeddb81cbe3728863b761dc5b6eb))
* **hermes:** extract model ID from router output correctly ([66405d9](https://github.com/Maleick/AutoShip/commit/66405d97ce038a140cb2d5e281e07a8d8355045f))
* **hermes:** proper delegate_task dispatch with timeout and config-driven max_concurrent ([90df8b9](https://github.com/Maleick/AutoShip/commit/90df8b9ed9bbbd2fe91b4fa9fac71bb9ffbb6e86))
* **hermes:** respect MODEL_OVERRIDE parameter for complex tasks ([3ffac9a](https://github.com/Maleick/AutoShip/commit/3ffac9a5af5d2ea991bb8113d30df7fe8e2e036b))
* **hooks:** enforce clean shell verification ([002dd32](https://github.com/Maleick/AutoShip/commit/002dd32150396f50200ef0a16c32ac0899d598b1))
* **hooks:** Hermes robustness fixes — set -e guards, model validation, auto-prune portability, label alignment ([6a2ba52](https://github.com/Maleick/AutoShip/commit/6a2ba520e31a2c56ee5215a5717252369a40d549)), closes [#322](https://github.com/Maleick/AutoShip/issues/322) [#323](https://github.com/Maleick/AutoShip/issues/323) [#324](https://github.com/Maleick/AutoShip/issues/324)
* **runner:** add macOS gtimeout support, fix status file path ([c1dd32e](https://github.com/Maleick/AutoShip/commit/c1dd32eadd054f71cb96688be0cbd7660d3050ef))
* **runner:** auto-mark COMPLETE after successful hermes chat ([aeb4a7f](https://github.com/Maleick/AutoShip/commit/aeb4a7fe4894b9e78f5ab2154f4e7d0a6cd6a5f3))
* **runner:** correct hermes chat CLI syntax and status file path ([c0aeedd](https://github.com/Maleick/AutoShip/commit/c0aeeddf97d3e5eba4b8b4889c604820295b4bb9))
* **runner:** detach workers from terminal session ([05c9378](https://github.com/Maleick/AutoShip/commit/05c93788f571f927bb0b11e6e2b20a8a3f15050e))
* **runner:** ensure background workers survive parent exit ([71dd583](https://github.com/Maleick/AutoShip/commit/71dd583297cb9349a181780d70263aa488ee9abb))
* **runner:** fix worktree detection, status checks, and env var defaults ([bb5fdd7](https://github.com/Maleick/AutoShip/commit/bb5fdd7bd9d7ece76e6c0b268d13d726c86e0792))
* **runner:** remove wait, fire-and-forget dispatch ([f045da7](https://github.com/Maleick/AutoShip/commit/f045da7d29697c18741a190e9d5d0bdf8a634ea5))
* **runner:** use absolute paths for AUTOSHIP_DIR ([5206a74](https://github.com/Maleick/AutoShip/commit/5206a74f6a7c244f88024076f694ed36180e7f5f))


### Features

* **assets:** add runtime icons to banner SVG ([9fb8b3c](https://github.com/Maleick/AutoShip/commit/9fb8b3cb4aeeebdbdf0c578f1a558d0e76d5df36)), closes [#hermes-support](https://github.com/Maleick/AutoShip/issues/hermes-support) [#branding](https://github.com/Maleick/AutoShip/issues/branding)
* **assets:** complete banner SVG rework from scratch ([562e1b0](https://github.com/Maleick/AutoShip/commit/562e1b06e26c42ee0b9e00904b2b5b9a58413265)), closes [#branding](https://github.com/Maleick/AutoShip/issues/branding) [#hermes-support](https://github.com/Maleick/AutoShip/issues/hermes-support)
* **bridge:** Kira-Kara Discord cross-agent communication ([e6dade1](https://github.com/Maleick/AutoShip/commit/e6dade132559758398cd24c8708961f00c014a76)), closes [#vanguard](https://github.com/Maleick/AutoShip/issues/vanguard)
* **hermes:** add auto-prune with configurable thresholds ([#320](https://github.com/Maleick/AutoShip/issues/320)) ([be66595](https://github.com/Maleick/AutoShip/commit/be6659593b1dad45c43c2d2c0609b857566c1abd))
* **hermes:** add automatic worktree cleanup after burn-down batches ([#317](https://github.com/Maleick/AutoShip/issues/317)) ([3e88b94](https://github.com/Maleick/AutoShip/commit/3e88b9433b005a37ec22a6da1adf00fafd265473))
* **hermes:** add delegate_task mode for inside-session execution ([52ac20e](https://github.com/Maleick/AutoShip/commit/52ac20ed1ff8feca78a324aa457a88633ce1886a))
* **hermes:** add Hermes agent runtime support ([14cc45b](https://github.com/Maleick/AutoShip/commit/14cc45b577c9a0853c169eda50f516ef991d584e))
* **hermes:** add post-merge cleanup hook ([#319](https://github.com/Maleick/AutoShip/issues/319)) ([7d60487](https://github.com/Maleick/AutoShip/commit/7d604876eb5e58513c47b3b1434f242fa5dde30e))
* **hermes:** auto-close issues after burn-down completion ([d59ece8](https://github.com/Maleick/AutoShip/commit/d59ece884706b2574139e9bc0eecc7a928abf294))
* **hermes:** intelligent model routing based on task analysis ([7e3c6ae](https://github.com/Maleick/AutoShip/commit/7e3c6aec8e1781dd940af66861e534e02c96c592))
* **hermes:** intelligent model routing based on task analysis ([#330](https://github.com/Maleick/AutoShip/issues/330)) ([858b922](https://github.com/Maleick/AutoShip/commit/858b922719132a9ccf591c4bd20aa5b290667f68))
* **hermes:** wire model routing, add Kara skill, cleanup hooks ([dc9e1f6](https://github.com/Maleick/AutoShip/commit/dc9e1f6f9fabe9f5acc825b77ed9539821df9ea1)), closes [#317](https://github.com/Maleick/AutoShip/issues/317)

# Changelog

## v2.2.1

- Add root and OpenCode-readable install entrypoints for agent-assisted installs.
- Link the raw install guide from README and OpenCode install docs.
- Align release metadata and lockfile handling for npm package publishing.
- Harden package verification and reviewed shell entrypoints before release.

## v2.2.0

- Add burndown policy profiles, overlap-aware planning, enriched worker prompts, and policy hazard verification.
- Add runtime metrics, circuit breaker, retry backoff, resource monitoring, A/B testing, and auto-documentation hooks.
- Harden runner salvage, cargo isolation, model routing defaults, event handling, and macOS-compatible locking.

## v2.1.0

- Harden issue records, quota guard inputs, create-pr result paths, and anti-flake retry handling.
- Preserve verified PR staging while excluding runtime `.autoship` state.
- Refresh release publishing guidance.

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
