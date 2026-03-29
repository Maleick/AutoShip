# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Frostreaver** is a Rust-based EverQuest multibox controller (targeting a 36-box setup on a TLP server). It has two components: an external process that reads game state via `ReadProcessMemory` and displays it in a TUI dashboard, and an injected DLL (`cdylib`) that hooks internal EQ functions for direct control (movement, casting, navigation).

## Build Commands

```bash
cargo build              # Debug build (works on macOS — stubs out Windows APIs)
cargo build --release    # Release build
cargo run                # Run TUI mode (demo mode on macOS, live on Windows)
cargo run -- --dump      # One-shot CLI dump mode (original M1 behavior)
cargo clippy             # Lint
cargo fmt --check        # Check formatting
```

The project has 621 tests across all three crates. Run `cargo test` to execute them. The project uses Rust edition 2024.

## Architecture

### Two runtime modes

- **TUI mode** (default): ratatui-based live dashboard with spawn list, player/target panels, hex dump viewer
- **Dump mode** (`--dump`): one-shot CLI output of player, target, and spawn data

### Cross-platform strategy

All Windows process APIs are behind `#[cfg(windows)]` with macOS/Linux stubs. The TUI runs on macOS with demo data (`dmft/src/tui/run.rs:load_demo_data`), making UI development possible without a live EQ client.

### Module structure (dmft orchestrator — M1)

- **`dmft/src/process/`** — OS-level process interaction
  - `memory.rs`: `ProcessHandle` (open, read, read_ptr, chase_ptr, read_string), `find_processes_by_name`
  - `window.rs`: `find_windows_by_title` — window enumeration for client discovery
- **`dmft/src/eq/`** — EverQuest-specific data layer
  - `structs.rs`: `SpawnInfo`, `EqClass`, `SpawnType` — high-level data types (not repr(C); built by reading individual fields)
  - `spawn.rs`: `read_spawn`, `read_local_player`, `read_target`, `read_all_spawns` — spawn linked list traversal
- **`dmft/src/tui/`** — Terminal UI (ratatui + crossterm)
  - `app.rs`: `App` state struct, filtering, navigation
  - `ui.rs`: Panel rendering (header, player, target, hex dump, spawn list table)
  - `run.rs`: Event loop, terminal setup/teardown, data refresh, demo data
  - `event.rs`: Keyboard input handling
- **`dmft/src/config.rs`** — TOML config loading (`config/frostreaver.toml`): process name, max spawns, group/toon definitions

### Key patterns

- **Offset rebasing**: All EQ pointers in `dmft-common/src/offsets.rs` are absolute preferred-base addresses. Use `offsets::rebase(preferred_addr, actual_base)` to convert to runtime addresses. This file also contains EQ internal function addresses (CastSpell, DoAttack, ExecuteCmd, etc.) used for direct function calls from the injected DLL.
- **Spawn linked list**: Spawns are a `TList<PlayerClient*>` accessed via SpawnManager. `read_all_spawns` walks `NEXT` pointers with a max-count safety limit.
- **Field-by-field reads**: `SpawnInfo` is populated by individual `proc.read::<T>(addr + OFFSET)` calls, not by reading a C struct wholesale. This is intentional — field offsets come from MQ2 headers and may not be contiguous.
- **File-based tracing**: Both the orchestrator (`dmft`) and DLL (`dmft-dll`) use `tracing` + `tracing-appender` for file-based structured logging.

### Reference material

`mq2-reference/` contains a MacroQuest2 source clone used for extracting struct offsets and pointer addresses. It is gitignored and not part of the build. The offsets in `dmft-common/src/offsets.rs` are derived from `mq2-reference/src/eqlib/include/eqlib/offsets/eqgame.h` and `PlayerClient.h`.

### Milestone context

The project follows a milestone-based plan:

- **M1** (complete): External memory reading + TUI dashboard
- **M2** (complete): DLL injection into eqgame.exe + internal function hooking (Rust cdylib), IPC (shared memory + named pipes), self-healing monitor, multi-client session manager
- **M2.5** (complete): Login automation — credential store (SQLite + AES-GCM + Argon2), process spawner, login state machine, launch coordinator with stagger, post-login sequencer, CPU affinity manager, hot-updatable offset database
- **M3** (complete): Navigation — waypoint-based pathfinding, Navigator FSM, movement humanization, stuck detection with escalating recovery, waypoint recorder, camp positioning, zone router
- **M4** (complete): Combat automation — ClassStrategy trait with per-class implementations (warrior/cleric/enchanter/generic DPS), HolyShit conditional ability system, GCD tracker, mana governor, puller FSM, aggro detection, combat coordinator
- **M5** (complete): Soul Engine — LLM-driven character personalities, persistent memory, idle behavior, social dynamics
- **M6** (next): LLM Character AI — API integration (Gemini/Claude), in-game chat responses
- **M7**: Learning/RL — behavioral cloning, RL fine-tuning, auto-research loops
- **M8**: Economy — vendor automation, EC tunnel trading, Bazaar, price tracking

The control approach uses DLL injection (like MacroQuest) rather than PostMessage — this enables calling internal EQ functions directly, accessing the navigation mesh for pathfinding, and writing to game memory. The MQ2 reference source (`mq2-reference/`) is used both for struct offsets and as architectural reference for hooking patterns.

### Module structure (dmft-common — shared types)

- **`dmft-common/src/`** — Shared types across all crates
  - `offsets.rs`: EQ memory addresses, struct field offsets, and internal function addresses (CastSpell, DoAttack, ExecuteCmd, etc.). All preferred-base (`0x140000000`), rebased at runtime via `rebase()`
  - `offset_db.rs`: Hot-updatable offset database (JSON load/save)
  - `nav.rs`: Waypoint, NavStatus, CampSpot, IndexedQueue<T>, Xorshift32 PRNG, KNUTH_HASH
  - `combat.rs`: CombatStatus, CombatRole, ClassStrategy types, HolyShit conditions, SpellEntry
  - `login.rs`: LoginPhase, LoginError, AccountInfo
  - `soul.rs`: Soul Engine shared types
  - `ipc.rs`: Command/Response enums for all IPC (nav + combat + login)
  - `protocol.rs`: Wire protocol types
  - `types.rs`: Common shared type definitions

### Module structure (dmft-dll — injected DLL, M2-M4)

- **`dmft-dll/src/hooks/`** — Game loop hooks
  - `game_loop.rs`: Main game loop hook (ProcessGameEvents), `movement.rs`: Movement hooks
  - `casting.rs`: Spell casting hooks, `targeting.rs`: Target selection hooks
- **`dmft-dll/src/eq/`** — EQ function bindings for direct calls from DLL
  - `widgets.rs`: Shared UI widget primitives (CXWndManager scan, CXStr read/write, button click via vtable, window find)
- **`dmft-dll/src/ipc/`** — DLL-side IPC (shared memory + named pipes)
  - `shared.rs`: Shared memory access, `pipe.rs`: Named pipe client
- **`dmft-dll/src/nav/`** — DLL-side navigation engine
  - `state.rs`: Navigator FSM, `mod.rs`: global singleton + tick integration
  - `stuck.rs`: StuckDetector, `humanize.rs`: MovementPersonality, `waypoint.rs`: WaypointQueue
- **`dmft-dll/src/combat/`** — DLL-side combat engine
  - `state.rs`: Combatant FSM, `strategy.rs`: ClassStrategy trait + CombatContext
  - `classes/`: warrior, cleric, enchanter, generic_dps implementations
  - `holyshit.rs`: conditional ability evaluator, `gcd.rs`: GCD tracker, `mana.rs`: ManaGovernor
  - `puller.rs`: pull cycle FSM, `aggro.rs`: heading-based aggro detection
  - `positioning.rs`: Combat positioning, `humanize.rs`: Combat action humanization
- **`dmft-dll/src/login/`** — Login automation (DLL-side)
  - `mod.rs`: Login state machine integration, `eqmain.rs`: eqmain.dll discovery and pointer resolution
  - `widgets.rs`: Login-specific UI widget helpers (credential entry, splash dismiss, SIDL window names)

### Module structure (dmft orchestrator — M2-M4)

- **`dmft/src/inject/`** — DLL injection
  - `loader.rs`: DLL injector, `dll_prep.rs`: DLL preparation/staging
- **`dmft/src/ipc/`** — Orchestrator-side IPC
  - `pipe.rs`: Named pipe server, `shared.rs`: Shared memory setup
- **`dmft/src/client/`** — Multi-client management
  - `manager.rs`: Client manager, `session.rs`: Session tracking
  - `healing.rs`: Self-healing monitor, `affinity.rs`: CPU affinity manager
- **`dmft/src/nav/`** — Orchestrator-side navigation
  - `recorder.rs`: WaypointRecorder + RDP simplification, `camp.rs`: CampManager, `router.rs`: zone routing
- **`dmft/src/combat/`** — Orchestrator-side combat coordination
  - `coordinator.rs`: assist target broadcasting, CC assignment, `spell_db.rs`: static spell data
- **`dmft/src/launcher/`** — Login automation (M2.5)
  - `login_sm.rs`: per-client login FSM, `coordinator.rs`: staggered launch orchestration
  - `spawner.rs`: CreateProcessW wrapper, `post_login.rs`: group→buff→camp sequencer
- **`dmft/src/credentials/`** — Encrypted credential store (M2.5)
  - `crypto.rs`: Argon2id + AES-256-GCM, `store.rs`: SQLite backend, `prompt.rs`: master password

### Module structure (dmft orchestrator — M5 Soul Engine)

- **`dmft/src/soul/`** — LLM-driven character AI
  - `personality.rs`: Character personality definitions, `memory.rs`: Persistent character memory
  - `coordinator.rs`: Soul Engine coordinator, `social.rs`: Social dynamics between characters
  - `idle.rs`: Idle behavior generation, `config.rs`: Soul Engine configuration
  - `llm/`: LLM integration — `priority_queue.rs`: Request prioritization, `fallback.rs`: Fallback behavior
- **`dmft-common/src/soul.rs`** — Shared Soul Engine types
