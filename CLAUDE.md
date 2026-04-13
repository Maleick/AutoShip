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

## Autonomous Agent Pipeline

Claude is an optional issue worker. Full protocol is in [`AGENTS.md`](AGENTS.md) — follow it for any GitHub-routed work. Key rule: agents never merge PRs; the AutoShip orchestrator manages PR lifecycle and merge decisions.

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

| File                        | Purpose                                                              |
| --------------------------- | -------------------------------------------------------------------- |
| `config/frostreaver.toml`   | Main config — process, launch, polling, group settings               |
| `config/accounts.toml`      | Per-account name, server, character, class, group                    |
| `config/camps/*.toml`       | Camp definitions — zone, pull point/radius, mana thresholds          |
| `config/classes/*.toml`     | 16 class ability configs with cooldowns, priorities, level overrides |
| `config/hvt_watchlist.toml` | High-value target alerts (named mob tracking + Discord)              |

## Log Locations

- **Orchestrator**: `./logs/textquest.log` (daily rolling)
- **DLL**: `%TEMP%/textquest/textquest-dll.log` (daily rolling)
- **Web**: stdout (axum default) — no file logging configured yet

## Patterns & Conventions

- **Offset rebasing**: All EQ pointers in `offsets.rs` are absolute preferred-base addresses. Use `offsets::rebase(preferred_addr, actual_base)` to convert to runtime addresses.
- **Spawn linked list**: `TList<PlayerClient*>` via SpawnManager. `read_all_spawns` walks `NEXT` pointers with a max-count safety limit.
- **Field-by-field reads**: `SpawnInfo` is populated by individual `proc.read::<T>(addr + OFFSET)` calls, not by reading a C struct wholesale. This is intentional — field offsets from MQ2 headers may not be contiguous.
- **Logging**: Use `tracing` + `tracing-appender` for file-based structured logging; do not use the `log` crate. `println!` / `eprintln!` are reserved for user-facing CLI/TUI output only (e.g., `textquest/src/main.rs`), not for structured logs.
- **Platform gates**: All OS APIs behind `#[cfg(windows)]` with macOS/Linux stubs. Never use `#[cfg(target_os)]` directly — use `#[cfg(windows)]` / `#[cfg(not(windows))]`.
- **DLL injection approach**: Custom Rust DLL (like MacroQuest) rather than PostMessage — enables direct EQ function calls, navmesh access, and game memory writes.
- **Tests**: Unit tests in-file (#[cfg(test)]); core logic tests are platform-independent and run on macOS, while Windows-specific tests are gated behind #[cfg(windows)].
- **IPC dual-channel**: Named pipe and shared memory names are derived via `textquest_common::ipc::pipe_name(session_id, client_id)` / `shared_memory_name(session_id, client_id)`. In practice this is `\\.\pipe\{session_id:x}_cmd_{client_id}` for commands (bidirectional, request/response) and `{session_id:x}_state_{client_id}` for state frames (broadcast, write-once per frame). `session_id` is required; do not assume the old predictable `textquest_cmd_` / `textquest_state_` prefixes.

## Gotchas

- **CMAKE env var**: `CMAKE_POLICY_VERSION_MINIMUM=3.5` is already set via `.cargo/config.toml`. Only export it manually if you are troubleshooting outside the normal Cargo flow.
- **macOS stubs**: `#[cfg(not(windows))]` stubs return dummy data. Some code paths are unreachable on macOS — don't chase bugs in stub implementations.
- **Offset addresses are not pointers**: Values in `offsets.rs` are preferred-base hex addresses, not ready-to-use pointers. Always `rebase()` before use.
- **MacroQuest/eqlib references are optional local checkouts**: keep them outside the repo if you use them for offset or struct-reference work. Routine `cargo build` / `cargo test` work does not require them. Derived offsets still live in `textquest-common/src/offsets.rs`.
- **Field reads, not struct casts**: If you see individual field reads where a struct read seems obvious, that's by design. MQ2 struct layouts have gaps.
- **Nightly MSVC toolchain**: Windows builds require nightly Rust because `retour` (function hooking) uses unstable features. macOS builds work on stable.
- **Edit tool + post-write automation**: Some local agent/editor setups run post-write automation that touches additional files and updates mtimes after an edit. This can trip Claude Code's "file modified since read" guard on subsequent edits in the same session. Fix: make all edits to a file in one `Edit` call, or use `Bash` for multi-edit `.rs` changes.
- **Self-hosted runner workspaces persist**: Files from previous runs may exist at test import time but vanish after `actions/checkout`. Use `self.skipTest()` inside test bodies instead of `@unittest.skipUnless` for file-existence guards.
- **Agent artifact files are gitignored**: `AUTOSHIP_RESULT.md`, `AUTOSHIP_PROMPT.md`, `BEACON_RESULT.md`, `BEACON_PROMPT.md`, `.autoship/` — never commit these. They are runtime outputs from the agent pipeline.
- **const fn misuse**: Drop `const` from functions that allocate (`Vec`, `String`, `Box`), take `&mut self`, or call non-const functions — none of these are const-evaluable. `#[cfg]`-gated blocks inside `const fn` are a separate but related rejection trigger.
