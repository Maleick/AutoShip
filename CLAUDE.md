# OpenWolf

@.wolf/OPENWOLF.md

This project uses OpenWolf for context management. Read and follow .wolf/OPENWOLF.md every session. Check .wolf/cerebrum.md before generating code. Check .wolf/anatomy.md before reading files.

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**TextQuest** is a Rust-based EverQuest multibox controller (targeting a 36-box setup on a TLP server). Four workspace crates: an external orchestrator (`textquest`) that reads game state via `ReadProcessMemory` and displays a TUI dashboard, an injected DLL (`textquest-dll`, cdylib) that hooks internal EQ functions for direct control, shared types (`textquest-common`), and a web dashboard (`textquest-web`, axum + React SPA).

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

~2,500+ tests across 4 workspace crates (auto-counted by `scripts/update_readme_metrics.py`). Platform-independent tests run on macOS; Windows-only tests are behind `#[cfg(windows)]`. Rust edition 2024. Nightly toolchain required on Windows because the hook stack depends on `retour`.

**CI gate**: Required check is `PR gate (fmt + clippy + test + python)`. Runs on self-hosted Windows runner for same-repo PRs, GitHub-hosted Windows for forks. Dev preflight: `python3 scripts/dev-preflight.py`.

**Dev preflight** runs the same fmt → clippy → test → Python test sequence as CI. Run it before pushing to catch failures locally.

## Autonomous Agent Pipeline

Claude is an optional issue worker. Full protocol is in [`AGENTS.md`](AGENTS.md) — follow it for any GitHub-routed work. Key rule: Claude never merges PRs; the shared Codex PR manager owns merge decisions.

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

**`textquest/` — Orchestrator (external process)**

| Module            | Purpose                                                                                                                                                     |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `process/`        | OS-level process interaction — open, read memory, find processes/windows                                                                                    |
| `eq/`             | EverQuest data layer — spawn structs, spawn linked list traversal                                                                                           |
| tui/              | Terminal UI — app state, event handling, theme, sprites; ui/ subdir has per-panel renderers (dashboard, groups, hex dump, map, navigation, spawns, widgets) |
| `config.rs`       | TOML config loading (`config/frostreaver.toml`)                                                                                                             |
| `inject/`         | DLL injection and staging                                                                                                                                   |
| `ipc/`            | Named pipe server + shared memory setup                                                                                                                     |
| `client/`         | Multi-client management — sessions, self-healing monitor, CPU affinity                                                                                      |
| `nav/`            | Waypoint recording (RDP simplification), camp management, zone routing                                                                                      |
| `combat/`         | Assist target broadcasting, CC assignment, spell database                                                                                                   |
| `camp/`           | Camp loop state machine — buffs, CC, class config, hunt mode, loot, positioning, progression, puller, recovery, vendor                                      |
| `orchestrator.rs` | Wires camp loop state machine to IPC command delivery                                                                                                       |
| `launcher/`       | Login automation — per-client login FSM, staggered launch, process spawner, post-login sequencer                                                            |
| `credentials/`    | Encrypted credential store — Argon2id + AES-256-GCM, SQLite backend                                                                                         |
| `soul/`           | Soul Engine — LLM-driven character personalities, persistent memory, idle behavior, social dynamics                                                         |
| `discord/`        | Discord integration — webhook alerts, command bridge, embedded serenity bot (DZ lockouts, contested mob alerts, slash commands)                              |
| `loot/`           | EQ item database (SQLite), TLP loot tables, per-character wishlists, loot history                                                                           |
| `metrics/`        | Fleet metrics — SQLite-backed events, DPS, loot, lockout, and plat tracking for session monitor                                                             |

**`textquest-dll/` — Injected DLL (cdylib)**

| Module      | Purpose                                                                                                                                                                                                    |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hooks/`    | Game loop hooks — ProcessGameEvents, movement, casting, targeting                                                                                                                                          |
| `eq/`       | EQ function bindings — UI widget primitives (CXWndManager, CXStr, button click via vtable)                                                                                                                 |
| `ipc/`      | Shared memory + named pipe client                                                                                                                                                                          |
| `nav/`      | Navigator FSM, stuck detection, movement humanization, waypoint queue                                                                                                                                      |
| `combat/`   | Combatant FSM, ClassStrategy trait, class strategy implementations (including a generic DPS strategy), HolyShit conditions, GCD tracker, mana governor, puller FSM, aggro detection, loot, skill cooldowns |
| `login/`    | Login state machine — eqmain.dll pointer resolution, credential entry, splash dismiss                                                                                                                      |
| `dialog.rs` | Auto-accept dialog handling (group invite, trade, task, resurrect)                                                                                                                                         |
| `stealth/`  | Memory stealth — Gargoyle-style sleep obfuscation, page encryption, keeps DLL code encrypted ~97% of the time                                                                                             |
| `syscall/`  | Indirect syscalls (RecycledGate pattern) — NT API calls through ntdll gadgets to pass anti-cheat call stack inspection                                                                                     |

**`textquest-common/` — Shared types**

| Module                                       | Purpose                                                                                                                                         |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `offsets.rs`                                 | EQ memory addresses + struct field offsets + internal function addresses. All preferred-base (`0x140000000`), rebased at runtime via `rebase()` |
| `offset_db.rs`                               | Hot-updatable offset database (JSON)                                                                                                            |
| `ipc.rs`                                     | Command/Response enums for all IPC channels                                                                                                     |
| `nav.rs`, `combat.rs`, `login.rs`, `soul.rs` | Domain-specific shared types                                                                                                                    |
| `protocol.rs`, `types.rs`                    | Wire protocol and common type definitions                                                                                                       |
| `ghidra_db.rs`                               | SQLite DB for Ghidra binary analysis data (~20K functions, globals, call graphs)                                                                |
| `packet.rs`                                  | EQ packet capture types — opcode filtering, direction-aware capture sessions                                                                    |
| `routing.rs`                                 | Routing scope types for cross-client command dispatch (single toon, named group, all sessions)                                                  |
| `scanner.rs`                                 | Byte-pattern signature scanner for resolving EQ function addresses (IDA-style patterns with wildcards)                                          |

**`textquest-web/` — Web dashboard (axum backend + React SPA)**

| Module | Purpose                                                                                    |
| ------ | ------------------------------------------------------------------------------------------ |
| `api/` | REST API for credentials, group config, loot tables                                        |
| `ws/`  | WebSocket endpoint for live session monitoring                                             |

### Milestones

- **M1** (complete): External memory reading + TUI dashboard
- **M2** (complete): DLL injection, internal function hooking, IPC, self-healing monitor
- **M2.5** (complete): Login automation — credential store, process spawner, login FSM, launch coordinator
- **M3** (complete): Navigation — waypoint pathfinding, Navigator FSM, humanization, stuck detection, zone router
- **M4** (complete): Combat automation — ClassStrategy trait, 17 classes, HolyShit system, puller FSM, combat coordinator
- **M5** (complete): Anti-Cheat — reflective injection, HWBP hooks, sleep obfuscation, indirect syscalls, ETW blinding, page encryption, stack spoofing, fingerprint spoofing
- **M6** (complete): Web Dashboard — axum + React/Vite/Tailwind SPA for credentials, group/camp config, session monitoring; TUI enhancements (EQ Internals, packet sniffer, map rework, Neriak theme)
- **M7**: Zoning/Movement — zone transitions, movement validation, travel recovery
- **M8**: Orchestrator — multibox coordination, group/session control, relay surfaces
- **M9**: Learning/RL — behavioral cloning, RL fine-tuning
- **M10**: Economy — Krono farm, vendor automation, loot distribution, banking
- **M11**: Soul Engine + LLM — local AI (Gemma 4/ollama), personalities, in-game chat (no external API)

## Configuration Files

| File                       | Purpose                                                     |
| -------------------------- | ----------------------------------------------------------- |
| `config/frostreaver.toml`  | Main config — process, launch, polling, group settings      |
| `config/accounts.toml`     | Per-account name, server, character, class, group           |
| `config/camps/*.toml`      | Camp definitions — zone, pull point/radius, mana thresholds |
| `config/classes/*.toml`    | 16 class ability configs with cooldowns, priorities, level overrides |
| `config/hvt_watchlist.toml`| High-value target alerts (named mob tracking + Discord)     |

## Log Locations

- **Orchestrator**: `./logs/textquest.log` (daily rolling)
- **DLL**: `%TEMP%/textquest/textquest-dll.log` (daily rolling)

## Patterns & Conventions

- **Offset rebasing**: All EQ pointers in `offsets.rs` are absolute preferred-base addresses. Use `offsets::rebase(preferred_addr, actual_base)` to convert to runtime addresses.
- **Spawn linked list**: `TList<PlayerClient*>` via SpawnManager. `read_all_spawns` walks `NEXT` pointers with a max-count safety limit.
- **Field-by-field reads**: `SpawnInfo` is populated by individual `proc.read::<T>(addr + OFFSET)` calls, not by reading a C struct wholesale. This is intentional — field offsets from MQ2 headers may not be contiguous.
- **Logging**: Use `tracing` + `tracing-appender` for file-based structured logging; do not use the `log` crate. `println!` / `eprintln!` are reserved for user-facing CLI/TUI output only (e.g., `textquest/src/main.rs`), not for structured logs.
- **Platform gates**: All OS APIs behind `#[cfg(windows)]` with macOS/Linux stubs. Never use `#[cfg(target_os)]` directly — use `#[cfg(windows)]` / `#[cfg(not(windows))]`.
- **DLL injection approach**: Custom Rust DLL (like MacroQuest) rather than PostMessage — enables direct EQ function calls, navmesh access, and game memory writes.
- **Tests**: Unit tests in-file (#[cfg(test)]); core logic tests are platform-independent and run on macOS, while Windows-specific tests are gated behind #[cfg(windows)].

## Gotchas

- **CMAKE env var**: `CMAKE_POLICY_VERSION_MINIMUM=3.5` is already set via `.cargo/config.toml`. Only export it manually if you are troubleshooting outside the normal Cargo flow.
- **macOS stubs**: `#[cfg(not(windows))]` stubs return dummy data. Some code paths are unreachable on macOS — don't chase bugs in stub implementations.
- **Offset addresses are not pointers**: Values in `offsets.rs` are preferred-base hex addresses, not ready-to-use pointers. Always `rebase()` before use.
- **MacroQuest references are optional local trees**: `third_party/eqlib` and `third_party/macroquest` are optional local reference paths for offset and struct-reference work when they are present in your workspace. Routine `cargo build` / `cargo test` work does not require them. Derived offsets still live in `textquest-common/src/offsets.rs`.
- **Field reads, not struct casts**: If you see individual field reads where a struct read seems obvious, that's by design. MQ2 struct layouts have gaps.
- **Nightly MSVC toolchain**: Windows builds require nightly Rust because `retour` (function hooking) uses unstable features. macOS builds work on stable.
