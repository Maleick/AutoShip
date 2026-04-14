# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**TextQuest** is a Rust-based EverQuest multibox controller (targeting large-scale multibox on a TLP server). Four workspace crates: an external orchestrator (`textquest`) that reads game state via `ReadProcessMemory` and displays a TUI dashboard, an injected DLL (`textquest-dll`, cdylib) that hooks internal EQ functions for direct control, shared types (`textquest-common`), and a web dashboard (`textquest-web`, axum + React SPA).

## Documentation Surfaces

- `README.md` is the usage-first entry point.
- `docs/wiki/` holds the long-lived operator and developer reference.
- `docs/implementation-roadmap.md` is the canonical milestone and evidence document.
- Do not introduce new docs or script assumptions that require a vendored reference tree inside this repo.

## Quick Start

```bash
cargo build          # Debug build (macOS OK — stubs Windows APIs)
cargo run            # TUI with demo data (macOS) or live data (Windows)
cargo test           # Full workspace test suite
python3 scripts/dev-preflight.py  # Same checks as CI — ALWAYS run before pushing
```

## Effort & Thinking Budget

- **Default (`/effort high`)**: 16k thinking tokens. Sufficient for M7/M8 architectural decisions, complex refactors, and multi-step debugging. Use for most work.
- **Low effort (`/effort low`)**: 8k tokens. Use only for: trivial edits, single-line fixes, obvious typo corrections.
- **Ultra effort (`/effort ultra`)**: 32k tokens (override cap). Use rarely — only when truly stuck on unfamiliar domain (e.g., first-time EQ offset RE, novel race condition debugging).
- **Note**: Thinking tokens cost ~3x regular tokens in context accounting. Thinking is compressed as context fills; prepare handoffs early if approaching 80% capacity.

## Build Commands

```bash
# CMAKE_POLICY_VERSION_MINIMUM is set automatically via .cargo/config.toml

cargo build                          # Debug build (works on macOS — stubs out Windows APIs)
cargo build --release                # Release build (requires nightly MSVC on Windows — retour dep)
cargo run                            # Run TUI mode (demo mode on macOS, live on Windows)
cargo run -- --dump                  # One-shot CLI dump mode
cargo clippy --all-targets --all-features -- -D warnings  # Lint (CI uses these flags)
cargo fmt --check                    # Check formatting
cargo test                           # Run full workspace test suite
cargo test -p textquest -- test_name # Run a single test in a specific crate
cargo test test_name                 # Run matching tests across all crates
```

```bash
# Python tests (CI also runs these)
python3 -m unittest discover -s tests -p 'test_*.py' -v
```

```bash
# Per-crate tests
cargo test -p textquest          # Orchestrator only
cargo test -p textquest-common   # Shared types only
cargo test -p textquest-dll      # DLL only (Windows, or stubs on macOS)

# Single test by name (substring match)
cargo test -p textquest test_name_here

# Run a specific integration test
cargo test -p textquest --test integration test_name
```

Platform-independent tests run on macOS; Windows-only tests are behind `#[cfg(windows)]`. Rust edition 2024. Nightly toolchain required on Windows because the hook stack depends on `retour`.

**CI gate**: Required check is `PR gate (fmt + clippy + test + python)`. All jobs run on self-hosted runners — Windows builds on Frostreaver (2 runners), Linux jobs on DigitalOcean (2 runners). Branch protection requires conversation resolution. Dev preflight: `python3 scripts/dev-preflight.py`.

**Dev preflight** runs the same fmt → clippy → test → Python test sequence as CI. Run it before pushing to catch failures locally.

## CI Infrastructure

All workflows run on self-hosted runners except the fork PR path in `ci.yml`, which uses `windows-latest` (GitHub-hosted) to avoid running untrusted fork code on self-hosted infrastructure.

- **Windows** (Frostreaver, Tailscale): `dmft-ci-service-01`, `dmft-ci-service-02` — Rust builds, release, nightly, wiki, README metrics
- **Linux** (DigitalOcean nyc1, Tailscale): `textquest-gha-linux-01`, `textquest-gha-linux-02` — merge gate, secrets scan, automation, Claude/Copilot agents
- Runner labels: `[self-hosted, Windows/Linux, X64, textquest]`
- Composite actions in `.github/actions/`: `setup-rust-nightly`, `setup-verified-python`, `ensure-cmake`, `configure-safe-directory`. Use these in workflows — never inline toolchain provisioning.
- Nightly release has a 3am CT time gate via PowerShell timezone check
- `readme-metrics.yml` requires both `contents: write` and `pull-requests: write`
- Release workflow triggers on `v*` tags (e.g., `git tag v0.1.0 -m "..." && git push origin v0.1.0`)
- Wiki sync: `python3 scripts/sync_wiki.py --push` — run after any `docs/wiki/` edits to publish to GitHub wiki
- `fmt-autofix.yml` must run on `[self-hosted, Windows, X64, textquest]` — `rustup`/`cargo fmt` are not on Linux DO runners

## Autonomous Agent Pipeline

Claude is an optional issue worker. Full protocol is in [`AGENTS.md`](AGENTS.md) — follow it for any GitHub-routed work. Key rule: agents never merge PRs; the AutoShip orchestrator manages PR lifecycle and merge decisions.

### Subagent Model Strategy

- **Haiku** (default, fast dispatch): TaskList polling, quick searches, issue triage, status checks.
- **Codex** (medium/complex, override per-task): M7/M8 feature dev, multi-file refactors, complex bug investigation. Use `Agent(subagent_type: "...", model: "codex", ...)` when a task requires sustained reasoning.
- **Opus** (strategic design, rare escalations): Cross-crate architecture reviews, novel design decisions. Use only for T.O.P. launch decisions, not execution.

## Architecture

### Runtime modes

- **TUI mode** (default): ratatui-based live dashboard with spawn list, player/target panels, hex dump, map, navigation, group views
- **Dump mode** (`--dump`): one-shot CLI output of player, target, and spawn data
- **Inject mode** (`--inject` / `--inject-pid <pid>`): DLL injection into EQ clients
- **Status mode** (`--status <pid>` / `--statusall`): query shared memory state
- **Command mode** (`--cmd <pid> "/slash"`): send a slash command to a client
- **Login mode** (`--login-pid <pid> <account> <password> [server] [character]`)

### Cross-platform strategy

All Windows process APIs are behind `#[cfg(windows)]` with macOS/Linux stubs. The TUI runs on macOS with demo data (`textquest/src/tui/run.rs:load_demo_data`), making UI development possible without a live EQ client.

### Module map

- `textquest`: external orchestrator binary; reads EQ state, manages the TUI, CLI modes, and process-facing control flow.
- `textquest-dll`: injected `cdylib`; hooks internal EverQuest functions and exposes direct in-process control/state capture.
- `textquest-common`: shared types and protocol surface used by the orchestrator, DLL, and other consumers.
- `textquest-web`: axum + React web dashboard for browser-based status and control surfaces.

### Milestones

See `docs/implementation-roadmap.md` for current milestone status and sequencing.

## Configuration Files

| File                        | Purpose                                                                                 |
| --------------------------- | --------------------------------------------------------------------------------------- |
| `config/textquest.toml`     | Main config — process, launch, polling, group settings                                  |
| `config/frostreaver.toml`   | **Deprecated** — legacy alias, superseded by textquest.toml; do not add new config here |
| `config/accounts.toml`      | Per-account name, server, character, class, group                                       |
| `config/camps/*.toml`       | Camp definitions — zone, pull point/radius, mana thresholds                             |
| `config/classes/*.toml`     | 16 class ability configs with cooldowns, priorities, level overrides                    |
| `config/hvt_watchlist.toml` | High-value target alerts (named mob tracking + Discord)                                 |

## Log Locations

- **Orchestrator**: `./logs/textquest.log` (daily rolling)
- **DLL**: `%TEMP%/textquest/textquest-dll.log` (daily rolling)
- **Web**: stdout (axum default) — no file logging configured yet

## Patterns & Conventions

- **Documentation & Polish**: Follow the [Documentation & Polish Standards](docs/dev/polish-standards.md) for all work. This includes specific issue structures, commit message formats, and code quality expectations (no `unwrap`, >70% coverage, etc.).
- **Offset rebasing**: All EQ pointers in `offsets.rs` are absolute preferred-base addresses. Use `offsets::rebase(preferred_addr, actual_base)` to convert to runtime addresses.
- **Spawn linked list**: `TList<PlayerClient*>` via SpawnManager. `read_all_spawns` walks `NEXT` pointers with a max-count safety limit.
- **Field-by-field reads**: `SpawnInfo` is populated by individual `proc.read::<T>(addr + OFFSET)` calls, not by reading a C struct wholesale. This is intentional — field offsets from MQ2 headers may not be contiguous.
- **Logging**: Use `tracing` + `tracing-appender` for file-based structured logging; do not use the `log` crate. `println!` / `eprintln!` are reserved for user-facing CLI/TUI output only (e.g., `textquest/src/main.rs`), not for structured logs.
- **Platform gates**: All OS APIs behind `#[cfg(windows)]` with macOS/Linux stubs. Never use `#[cfg(target_os)]` directly — use `#[cfg(windows)]` / `#[cfg(not(windows))]`.
- **DLL injection approach**: Custom Rust DLL (like MacroQuest) rather than PostMessage — enables direct EQ function calls, navmesh access, and game memory writes.
- **Tests**: Unit tests in-file (#[cfg(test)]); core logic tests are platform-independent and run on macOS, while Windows-specific tests are gated behind #[cfg(windows)].
- **IPC dual-channel**: Named pipe and shared memory names are derived via `textquest_common::ipc::pipe_name(session_id, client_id)` / `shared_memory_name(session_id, client_id)`. In practice this is `\\.\pipe\{session_id:x}_cmd_{client_id}` for commands (bidirectional, request/response) and `{session_id:x}_state_{client_id}` for state frames (broadcast, write-once per frame). `session_id` is required; do not assume the old predictable `textquest_cmd_` / `textquest_state_` prefixes.

## Gotchas

### Critical for M7/M8 Feature Work (Read These First)

- **Offset rebasing**: All pointers in `offsets.rs` need `offsets::rebase(preferred_base, actual_base)` before use. Values in offsets.rs are preferred-base hex addresses, not ready-to-use pointers. Skipped rebasing → crash or silent data corruption (hard to debug).
- **Field reads, not struct casts**: `SpawnInfo` populated field-by-field via `proc.read::<T>(addr + OFFSET)`, not wholesale struct cast. MQ2 struct layouts have gaps. Whole-struct reads → offset misalignment → broken AI decisions.
- **IPC dual-channel naming**: Pipe/shared-mem names derived via `pipe_name(session_id, client_id)` → `\\.\pipe\{session_id:x}_cmd_{client_id}` and `{session_id:x}_state_{client_id}`. Never hardcode `textquest_cmd_` / `textquest_state_` prefixes. Hardcoded names → multi-session collisions, lost state frames.
- **Platform gates**: All Windows APIs behind `#[cfg(windows)]` with macOS/Linux stubs. Never use `#[cfg(target_os)]` directly. Stubs return dummy data; code paths unreachable on macOS. Don't chase bugs in stub implementations.
- **const fn constraints**: Drop `const` from functions that allocate (`Vec`, `String`, `Box`), take `&mut self`, or call non-const functions. `#[cfg]`-gated blocks inside `const fn` are also const-incompatible. Compiler errors on innocent-looking allocations.

### Other Gotchas (Less Frequent)

- **CMAKE env var**: `CMAKE_POLICY_VERSION_MINIMUM=3.5` is already set via `.cargo/config.toml`. Only export it manually if you are troubleshooting outside the normal Cargo flow.
- **MacroQuest/eqlib references are optional local checkouts**: keep them outside the repo if you use them for offset or struct-reference work. Routine `cargo build` / `cargo test` work does not require them. Derived offsets still live in `textquest-common/src/offsets.rs`.
- **Nightly MSVC toolchain**: Windows builds require nightly Rust because `retour` (function hooking) uses unstable features. macOS builds work on stable.
- **Edit tool + post-write automation**: Some local agent/editor setups run post-write automation that touches additional files and updates mtimes after an edit. This can trip Claude Code's "file modified since read" guard on subsequent edits in the same session. Fix: make all edits to a file in one `Edit` call, or use `Bash` for multi-edit `.rs` changes.
- **Self-hosted runner workspaces persist**: Files from previous runs may exist at test import time but vanish after `actions/checkout`. Use `self.skipTest()` inside test bodies instead of `@unittest.skipUnless` for file-existence guards.
- **Agent artifact files are gitignored**: `AUTOSHIP_RESULT.md`, `AUTOSHIP_PROMPT.md`, `BEACON_RESULT.md`, `BEACON_PROMPT.md`, `.autoship/` — never commit these. They are runtime outputs from the agent pipeline.
- **README metrics badge format**: `update_readme_metrics.py` finds `[![Rust LOC]` and `[![Tests]` string markers for in-place replacement. HTML `<img>` badge format breaks it — keep these two badges as markdown even if other badges are HTML.
- **Stale remote branch refs**: `git branch -r` can show 100+ phantom branches that no longer exist on GitHub. Run `git remote prune origin` to clear stale local tracking refs before any branch audit or bulk-delete operation.
- **Pre-checkout bootstrap paradox**: The workspace prep bash block in `ci.yml`/`claude-agent.yml` must run before `actions/checkout`, so it cannot source a script from the repo. Keep it inlined in both workflows; `tests/test_ci_runner_workspace_prep.py` validates both blocks stay identical.
- **Adding `ActiveScreen` variants**: touch 4 files — `app.rs` (enum + `label()` + `ALL` array + `layout_presets: [LayoutPreset; N]` + 3 match fns) and `tui/ui/mod.rs` (screen dispatch + 2 `header_tab_label` matches + help match).
- **LSP diagnostics lag after edits**: `<new-diagnostics>` blocks reflect pre-edit state. Trust `cargo build` output, not the inline diagnostics, to confirm fixes landed.
- **`GameState` test initializers**: always include `actual_version: None` — the field exists on the common-crate struct but is easy to miss in manual struct literals.
- **OpenWolf system**: `.wolf/` directory holds anatomy.md (file map), cerebrum.md (preferences/do-not-repeat), memory.md (session log), and buglog.json (known fixes). Rules are loaded via `.claude/rules/openwolf.md`. Check `.wolf/buglog.json` before fixing any bug; update it after every fix.
- **Stale worktree CLAUDE.md files**: `.claude/worktrees/agent-*/CLAUDE.md` are copies created by agent worktrees and are never current. Ignore them — only the repo root `CLAUDE.md` is authoritative.
