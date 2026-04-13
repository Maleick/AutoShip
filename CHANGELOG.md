## v1.4.2 — v1.4.2
_2026-04-13_

## Fixes

### dispatch-codex-appserver.sh — tool resolution

`TOOL` is now resolved once at script startup (reading `state.json` with `2>/dev/null || echo "codex-spark"` fallback) and reused across all five stuck/exhausted paths:

- **Before:** 3 early-exit branches (crash, init timeout, init failure) hardcoded `"codex-spark"` — incorrect for `codex-gpt` dispatches. The wrong tool's stuck count was incremented.
- **Before:** 2 dynamic-lookup branches ran `jq` again mid-script and were missing `|| true` under `set -euo pipefail`, meaning a missing `state.json` could abort the script before emitting the stuck event.
- **After:** Single lookup at top, `|| true` on all five call sites.

### extract-context.sh — awk heading pattern

- Removed `.*` before keyword anchor: `^#+[[:space:]]*.*(keyword)` → `^#+[[:space:]]*(keyword)`. The old pattern matched any header that *contained* the keyword anywhere in the line; the new pattern requires the heading to *start* with it.
- Removed redundant `rm -f "$TEMP_FILE"` — the `trap EXIT` added in v1.4.1 already handles cleanup on all exit paths.
- Fixed comment: "deduplicate sections" was inaccurate (no deduplication occurs).

## Upgrade

No breaking changes. Drop-in replacement for v1.4.1.

---

## v1.4.1 — v1.4.1
_2026-04-13_

## Fixes

### Correctness

- **`mark_exhausted()` removed from `dispatch-codex-appserver.sh`** — All crash/timeout exhaustion paths now route through `quota-update.sh stuck`, which enforces the 3-strike threshold, emits `TOOL_DEGRADED` events, and uses the canonical schema. The local function bypassed all of this.

- **`detect-tools.sh` app-server probe** — Replaced inline jq exhausted write with `quota-update.sh stuck` calls for both `codex-spark` and `codex-gpt`. Both tools share the same binary; if `codex app-server help` fails, both are equally non-functional.

- **`init.sh` rust_windows profile** — `map_values` was overwriting `rust_unsafe` routing with `["claude-haiku", "claude-sonnet"]` when a rust_windows profile was detected, defeating its purpose. Fixed with `with_entries` to preserve the `rust_unsafe` key.

- **`extract-context.sh` awk portability** — `IGNORECASE = 1` is a gawk-only extension silently ignored by macOS system awk. Replaced with `tolower($0) ~` pattern matching (works on all awk implementations). Also added `trap 'rm -f "$TEMP_FILE"' EXIT` to prevent temp file leaks on error exit.

### Efficiency

- **`quota-update.sh` ensure_init** — Upgrade pass (adding `tool_stuck_count`/`exhausted` fields) now skips if `advisor_calls_today` sentinel is already present, avoiding a redundant jq rewrite on every command invocation.

## Upgrade

No breaking changes. State files are backwards-compatible with v1.4.0.

```bash
/install-plugin /path/to/AutoShip
```

---

## v1.4.0 — v1.4.0 — 9 Self-Improvement Issues
_2026-04-13_

## What's New in v1.4.0

AutoShip shipped all 9 of its own self-improvement issues in a single session. Zero manual PRs. Zero manual merges.

### New Features

**🦀 Rust/unsafe routing** (#101)
- New `rust_unsafe` task type routes unsafe Rust, DLL injection, and Windows-specific issues directly to Claude
- `rust_windows` project profile detection: checks for `Cargo.toml` + `#[cfg(windows)]` in `src/` and overrides all routing to prefer Claude
- Documented keyword-based promotion: `unsafe`, `DLL`, `cdylib`, `winapi`, `retour`, `#[cfg(windows)]`

**📝 test_command auto-detection** (#102)
- `hooks/init.sh` now scans `CLAUDE.md` and `AGENTS.md` for test command patterns at startup
- Detects: `cargo test`, `pytest`, `npm test`, `make test`, `./gradlew test`, `python3 scripts/dev-preflight.py`
- Writes `test_command` and `verify_command` to `config.json`; warns if nothing detected; idempotent

**🏗️ Project context injection** (#103)
- New `hooks/extract-context.sh` extracts Patterns/Conventions/Gotchas sections from `CLAUDE.md` and `AGENTS.md`
- Output capped at 3000 chars (~500 tokens), written to `.autoship/project-context.md`
- Injected as `## Project Context` into every dispatch prompt (Gemini, Codex, Haiku, Sonnet)

**⚕️ Codex fast-fail + stuck tracking** (#104)
- `hooks/dispatch-codex-appserver.sh`: fast-fail `codex --version` health check (10s timeout) at top of script
- `hooks/quota-update.sh`: new `stuck <tool>` subcommand increments `tool_stuck_count`; marks `exhausted: true` at ≥3; emits `TOOL_DEGRADED` event

**🎯 Opus pre-dispatch advisor for risky issues** (#105)
- Gate condition in dispatch Step 3C: `complexity==complex` + `unsafe`/`DLL`/`hook`/`injection` keyword or `risk:high` label triggers Opus call
- Opus returns JSON `{key_files, invariants, approach, risks}` (≤200 words) prepended as `## Architectural Guidance`
- Budget protected: `advisor_calls_today` tracked in quota.json (cap: 10/day, daily auto-reset)

### Bug Fixes & Improvements

**🔧 Codex app-server auto-fallback** (#93)
- `hooks/dispatch-codex-appserver.sh`: detect init failure/timeout, mark codex-spark exhausted, exit STUCK
- `hooks/detect-tools.sh`: probe app-server availability before recording quota; auto-mark exhausted on failure

**🗂️ Monitor state file initialization** (#94)
- `hooks/init.sh` now creates `.autoship/.pr-monitor-seen.json` and `event-queue.json` on first run
- `hooks/monitor-prs.sh` checks for file existence with clear error message

**🔑 Compound issue sub-key support** (#95)
- All hooks now accept compound keys like `issue-757a` or `issue-757-1`
- Numeric prefix extracted for GitHub API calls; documented in dispatch skill

**⚡ 20-agent dynamic cap** (#96)
- Enforced and documented across orchestrate, dispatch skills, AUTOSHIP.md, README, and landing page

---

## Upgrade

```bash
/install-plugin /path/to/AutoShip
```

No breaking changes. All state files are backwards-compatible.

---

## v1.3.0 — v1.3.0 — AutoShip Rebrand + 20-Agent Cap + Security Hardening
_2026-04-13_

## What's New in v1.3.0

### Features
- **AutoShip rebrand** — Complete rename from Beacon → AutoShip across 40+ files. New brand identity, slogan, sponsor badge, and tier diagram. All hooks, skills, state variables, and env vars updated.
- **20-agent dynamic cap** — Dispatch now enforces a 50-agent hard cap (up from the implied 6). Claude agents up to 50 in parallel; Gemini 20+ via `even-vertical` layout; Codex app-server documented as non-functional with auto-fallback protocol.
- **Codex app-server failure protocol** — On first STUCK from codex-spark/gpt, immediately escalate to next agent in priority list. No stall wait. Tool marked `exhausted: true` in quota.json for session.

### Bug Fixes
- **Dynamic hook path resolution** — `start.md` and all skills now use `$(cat .autoship/hooks_dir)` instead of hardcoded user-specific paths. Fully portable across machines.
- **jq arg name fix in dispatch** — Corrected `--argjson` parameter name in turn/start JSON-RPC call (#92).
- **Monitor path resolution** — Orchestrate skill uses `git rev-parse --show-toplevel` for dynamic hook path resolution at Monitor startup.

### Security
- **Prompt injection disclaimers** — All dispatch prompt templates now wrap issue body in `UNTRUSTED CONTENT` block with explicit warning to agents.
- **TOCTOU fix + path canonicalization** — Verify pipeline temp files use unique names; all paths canonicalized before use.

### CI/Infra
- **GitHub Pages custom domain** — `autoship.teamoperator.red` configured via CNAME.
- **CLAUDE.md gitignored** — Project instructions kept local, not committed.

## Upgrade Notes

If upgrading from v1.2.x: re-run `/install-plugin` to pick up the renamed hooks and skills. The `.autoship/` state directory structure is unchanged and compatible.

## Stats
- 11 commits since v1.2.1
- 40+ files renamed/updated in the rebrand
- 0 breaking API changes

---

## v1.2.1 — v1.2.1 — Bug fixes
_2026-04-13_

## Bug Fixes

### P0 — Runtime failures

- **`emit-event.sh`**: Replaced bare `flock` (Linux-only) with `flock → lockf → best-effort` fallback. On macOS, `flock` is absent; under `set -e` this silently aborted the script, dropping all queued events.
- **`monitor-agents.sh`**: Same flock/lockf fallback applied to the crash handler's event-queue write.
- **`update-state.sh`**: Fixed ISSUE_ID regex to accept `issue-88` format. Previous regex `^[0-9]+(-[a-z0-9-]+)?$` rejected keys starting with letters, causing all callers from `cleanup-worktree.sh` and `dispatch-codex-appserver.sh` to fail validation.

### P1 — Safety / correctness

- **`beacon-verify/SKILL.md`**: Guard now checks `[[ -e "$BEACON_RESULT_PATH" ]]` before calling `realpath`. On macOS/BSD, `realpath` on a non-existent file returns empty, causing a missing result file to be misreported as a symlink escape.
- **`beacon-init.sh`**: Replaced three fixed `${STATE_FILE}.tmp` paths in the reconcile loop with a single `mktemp`-allocated temp file, consistent with the TOCTOU fixes applied elsewhere.

### Cosmetic

- **`README.md`**: Quoted `/autoship:start` mermaid node label with `["..."]` syntax to fix "Unable to render rich display" error on GitHub.

---

## v1.2.0 — v1.2.0 — AutoShip rebrand + security hardening
_2026-04-13_

## AutoShip v1.2.0

This release rebrands the plugin from Beacon to **AutoShip** and ships a full security hardening pass across all shell hooks.

### Rebrand

- Plugin renamed from Beacon to AutoShip. Install command is now:
  ```
  claude plugin marketplace add Maleick/AutoShip && claude plugin install autoship@autoship
  ```
- All commands renamed: `/autoship:start`, `/autoship:plan`, `/autoship:stop`, `/autoship:status`
- New banner graphic, full README rewrite with mermaid diagrams and benchmarks
- GitHub Pages landing site at https://maleick.github.io/AutoShip/
- Wiki published: Architecture, Configuration, Design Decisions, Troubleshooting

### Security Hardening (9 findings addressed)

**Critical**
- Added `^issue-[0-9]+$` validation at all script entry points to prevent path traversal via malformed ISSUE_KEY (cleanup-worktree.sh, dispatch-codex-appserver.sh, update-state.sh)
- Eliminated rm -rf path traversal vector in cleanup-worktree.sh

**High**
- Replaced manual JSON-RPC string construction with `jq -n --arg` in dispatch-codex-appserver.sh and monitor-agents.sh to prevent JSON injection
- Fixed lockf subshell injection in beacon-init.sh and update-state.sh by passing paths as positional arguments instead of interpolating into bash -c strings
- Added credential exclusions to .gitignore: `.env`, `*.pem`, `*.key`, `credentials.json`

**Medium**
- FIFOs created with `mkfifo -m 600` (owner-only permissions)
- Added `chmod 600` to temp files before writing in beacon-init.sh and update-state.sh
- Added Step 0.5 to verification pipeline: path canonicalization guard + mandatory FAIL on empty git diff

### Other

- Added prompt injection disclaimers to all 3 agent dispatch templates
- Wiki link and security badge added to README
- ISSUE_ID format validation added to update-state.sh
- AutoShip ran on itself to ship 3 issues in this release

---

## v1.1.0 — v1.1.0 — Symphony dispatch, routing matrix, token ledger
_2026-04-13_

## What's New in v1.1.0

### Architecture
- **Codex app-server dispatch** — Codex now runs via JSON-RPC app-server protocol. No tmux panes required. 300s stall watchdog, atomic event-queue writes.
- **Symphony protocol** — All third-party agents communicate via standardized `turn/completed` / `thread/tokenUsage/updated` events.

### Routing
- **BEACON.md routing matrix** — Configure agent priority per task type in YAML front matter. Changes hot-reload without restarting Beacon.
- **Task-type classifier** — 7 task types: `research`, `docs`, `simple_code`, `medium_code`, `complex`, `mechanical`, `ci_fix`. Label → complexity → body heuristics → title keywords → default.
- **Copilot CLI support** — GitHub Copilot (`gh copilot` or standalone) added to dispatch routing. Grok CLI removed (no OAuth support).

### Observability
- **Token ledger** — Every issue's token spend tracked in `.beacon/token-ledger.json`. Per-session and all-time aggregates.
- **Stats scopes** — `/beacon:status` shows session vs all-time dispatch/completion counts.
- **BEACON_RESULT.md archival** — Completed results saved to `.beacon/results/<N>-<slug>.md`.

### Reliability
- **Pre-dispatch exhaustion gate** — Skips agents whose quota is exhausted before creating a worktree.
- **Event queue** — All agent completions route through `.beacon/event-queue.json` with atomic flock writes.

## Upgrade

```bash
claude plugin update beacon
```

---

## v0.1 — v0.1 — Initial Release
_2026-04-12_

## Beacon v0.1

First public release of Beacon — autonomous multi-agent GitHub issue orchestration for Claude Code.

### What's included

- **v3 Advisor + Monitor architecture** — Sonnet executor, Opus advisor, Haiku triage
- **Third-party-first dispatch** — Routes to Codex, Gemini, or Grok before consuming Claude quota
- **Worktree isolation** — Each issue gets its own git worktree and tmux pane
- **Tiered escalation** — Haiku → Sonnet → Opus based on complexity and failure count
- **Verification pipeline** — Sonnet review → simplify → PR → CI monitor → auto-merge
- **Discord integration** — Webhook event handling and command channel support
- **Quota tracking** — Decay-based estimation with ASCII progress bars in `/beacon status`
- **Error recovery** — Session restart, stale worktree cleanup, crash detection, API retry

### Install

\`\`\`bash
/install-plugin https://github.com/Maleick/beacon
\`\`\`

### Requirements

- Claude Code with plugin support
- `jq`, `gh` (authenticated)
- Optional: `codex`, `gemini`, or `grok` CLI for quota efficiency

### Commands

| Command | Purpose |
|---------|---------|
| `/beacon start` | Launch orchestration |
| `/beacon status` | Show agents, quota, progress |
| `/beacon stop` | Stop all agents |
| `/beacon plan` | Analyze without dispatching |

---

