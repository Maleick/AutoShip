# OpenWolf

@.wolf/OPENWOLF.md

This project uses OpenWolf for context management. Read and follow .wolf/OPENWOLF.md every session. Check .wolf/cerebrum.md before generating code. Check .wolf/anatomy.md before reading files.

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**TextQuest** is a Rust-based EverQuest multibox controller (targeting a 36-box setup on a TLP server). It has two components: an external process that reads game state via `ReadProcessMemory` and displays it in a TUI dashboard, and an injected DLL (`cdylib`) that hooks internal EQ functions for direct control (movement, casting, navigation).

## Build Commands

```bash
# CMAKE_POLICY_VERSION_MINIMUM is set automatically via .cargo/config.toml

cargo build              # Debug build (works on macOS — stubs out Windows APIs)
cargo build --release    # Release build
cargo run                # Run TUI mode (demo mode on macOS, live on Windows)
cargo run -- --dump      # One-shot CLI dump mode (original M1 behavior)
cargo clippy             # Lint
cargo fmt --check        # Check formatting
cargo test               # Run tests (macOS runs platform-independent subset)
```

~1250 platform-independent tests across 3 crates (625 textquest + 431 textquest-common + 172 textquest-dll). Additional Windows-only tests are behind `#[cfg(windows)]`. Rust edition 2024.

## Autonomous Agent Pipeline

Claude is an optional issue worker in TextQuest. Follow [`AGENTS.md`](AGENTS.md) and mirror these rules whenever you act through GitHub or a local Claude Code session:

- Only execute an issue when it is in the `TextQuest Roadmap` GitHub Project with `Agent Status = Ready for Agent` and has label `agent:ready`.
- Claude is opt-in. Treat `worker:claude` or an explicit `@claude` mention as the routing signal. Otherwise Codex is the default worker.
- Claim exactly one issue per run by moving `Agent Status` to `Agent Working`, replacing `agent:ready` with `agent:working`, and posting a short claim comment.
- Branch from `master` as `claude/issue-<number>-<slug>`.
- Default to implementation mode for concrete issues. Do not wait for a second instruction to start coding.
- If an issue has `mode:research`, use the same docs-first workflow as the Codex autoresearch loop: update roadmap or research docs first, run the verifier and guard commands, and do not commit transient automation state.
- If the task spans multiple independent surfaces, use parallel workers or subagents automatically and integrate before final verification.
- Always run the issue's `Verify` commands. If Rust, config, or scripts changed, also run repo gate commands when feasible. If the diff is docs, workflow, or prompt only, run lightweight syntax checks plus `python3 scripts/sync_wiki.py --check`.
- Open a non-draft PR into `master`, link the issue, and stop there. Claude never merges TextQuest pull requests; the shared Codex PR manager owns merge decisions.
- If blocked, move the issue `Agent Status` to `Blocked`, add `agent:blocked`, and leave a concrete unblock comment.

## Architecture

### Runtime modes

- **TUI mode** (default): ratatui-based live dashboard with spawn list, player/target panels, hex dump, map, navigation, group views
- **Dump mode** (`--dump`): one-shot CLI output of player, target, and spawn data

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

**`textquest-common/` — Shared types**

| Module                                       | Purpose                                                                                                                                         |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `offsets.rs`                                 | EQ memory addresses + struct field offsets + internal function addresses. All preferred-base (`0x140000000`), rebased at runtime via `rebase()` |
| `offset_db.rs`                               | Hot-updatable offset database (JSON)                                                                                                            |
| `ipc.rs`                                     | Command/Response enums for all IPC channels                                                                                                     |
| `nav.rs`, `combat.rs`, `login.rs`, `soul.rs` | Domain-specific shared types                                                                                                                    |
| `protocol.rs`, `types.rs`                    | Wire protocol and common type definitions                                                                                                       |

### Milestones

- **M1** (complete): External memory reading + TUI dashboard
- **M2** (complete): DLL injection, internal function hooking, IPC, self-healing monitor
- **M2.5** (complete): Login automation — credential store, process spawner, login FSM, launch coordinator
- **M3** (complete): Navigation — waypoint pathfinding, Navigator FSM, humanization, stuck detection, zone router
- **M4** (complete): Combat automation — ClassStrategy trait, 17 classes, HolyShit system, puller FSM, combat coordinator
- **M5** (~70% — 4 closed, 9 open, 5 PRs in review): Anti-Cheat — reflective injection, HWBP hooks, sleep obfuscation, indirect syscalls, ETW blinding, page encryption, stack spoofing, fingerprint spoofing
- **M6** (~55% — 11 closed, 9 open): Web Dashboard — axum + React/Vite/Tailwind SPA for credentials, group/camp config, session monitoring; TUI enhancements (EQ Internals, packet sniffer, map rework, Neriak theme)
- **M7**: Zoning/Movement — zone transitions, movement validation, travel recovery
- **M8**: Orchestrator — multibox coordination, group/session control, relay surfaces
- **M9**: Learning/RL — behavioral cloning, RL fine-tuning
- **M10**: Economy — Krono farm, vendor automation, loot distribution, banking
- **M11**: Soul Engine + LLM — local AI (Gemma 4/ollama), personalities, in-game chat (no external API)

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
- **MacroQuest references are local submodules**: `third_party/eqlib` and `third_party/macroquest` are part of the repo as git submodules and are used for offset and struct-reference work. Routine `cargo build` / `cargo test` work does not require them, but offset/struct work does. Run `git submodule update --init --recursive` after checkout. Derived offsets still live in `textquest-common/src/offsets.rs`.
- **Field reads, not struct casts**: If you see individual field reads where a struct read seems obvious, that's by design. MQ2 struct layouts have gaps.
