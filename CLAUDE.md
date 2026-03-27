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

No tests exist yet. The project uses Rust edition 2024.

## Architecture

### Two runtime modes

- **TUI mode** (default): ratatui-based live dashboard with spawn list, player/target panels, hex dump viewer
- **Dump mode** (`--dump`): one-shot CLI output of player, target, and spawn data

### Cross-platform strategy

All Windows process APIs are behind `#[cfg(windows)]` with macOS/Linux stubs. The TUI runs on macOS with demo data (`src/tui/run.rs:load_demo_data`), making UI development possible without a live EQ client.

### Module structure

- **`src/process/`** — OS-level process interaction
  - `memory.rs`: `ProcessHandle` (open, read, read_ptr, chase_ptr, read_string), `find_processes_by_name`
  - `window.rs`: `find_windows_by_title` — window enumeration for client discovery
- **`src/eq/`** — EverQuest-specific data layer
  - `offsets.rs`: Memory addresses and struct field offsets from MQ2/eqlib headers. All addresses are preferred-base (`0x140000000`) and must be rebased at runtime via `rebase()`
  - `structs.rs`: `SpawnInfo`, `EqClass`, `SpawnType` — high-level data types (not repr(C); built by reading individual fields)
  - `spawn.rs`: `read_spawn`, `read_local_player`, `read_target`, `read_all_spawns` — spawn linked list traversal
- **`src/tui/`** — Terminal UI (ratatui + crossterm)
  - `app.rs`: `App` state struct, filtering, navigation
  - `ui.rs`: Panel rendering (header, player, target, hex dump, spawn list table)
  - `run.rs`: Event loop, terminal setup/teardown, data refresh, demo data
  - `event.rs`: Keyboard input handling
- **`src/config.rs`** — TOML config loading (`config/frostreaver.toml`): process name, max spawns, group/toon definitions

### Key patterns

- **Offset rebasing**: All EQ pointers in `offsets.rs` are absolute preferred-base addresses. Use `offsets::rebase(preferred_addr, actual_base)` to convert to runtime addresses.
- **Spawn linked list**: Spawns are a `TList<PlayerClient*>` accessed via SpawnManager. `read_all_spawns` walks `NEXT` pointers with a max-count safety limit.
- **Field-by-field reads**: `SpawnInfo` is populated by individual `proc.read::<T>(addr + OFFSET)` calls, not by reading a C struct wholesale. This is intentional — field offsets come from MQ2 headers and may not be contiguous.

### Reference material

`mq2-reference/` contains a MacroQuest2 source clone used for extracting struct offsets and pointer addresses. It is gitignored and not part of the build. The offsets in `src/eq/offsets.rs` are derived from `mq2-reference/src/eqlib/include/eqlib/offsets/eqgame.h` and `PlayerClient.h`.

### Milestone context

The project follows a milestone-based plan:

- **M1** (complete): External memory reading + TUI dashboard
- **M2** (complete): DLL injection into eqgame.exe + internal function hooking (Rust cdylib), IPC (shared memory + named pipes), self-healing monitor, multi-client session manager
- **M2.5** (complete): Login automation — credential store (SQLite + AES-GCM + Argon2), process spawner, login state machine, launch coordinator with stagger, post-login sequencer, CPU affinity manager, hot-updatable offset database
- **M3** (complete): Navigation — waypoint-based pathfinding, Navigator FSM, movement humanization, stuck detection with escalating recovery, waypoint recorder, camp positioning, zone router
- **M4** (complete): Combat automation — ClassStrategy trait with per-class implementations (warrior/cleric/enchanter/generic DPS), HolyShit conditional ability system, GCD tracker, mana governor, puller FSM, aggro detection, combat coordinator
- **M5** (next): Soul Engine — LLM-driven character personalities, persistent memory, idle behavior, social dynamics
- **M6**: LLM Character AI — API integration (Gemini/Claude), in-game chat responses
- **M7**: Learning/RL — behavioral cloning, RL fine-tuning, auto-research loops
- **M8**: Economy — vendor automation, EC tunnel trading, Bazaar, price tracking

The control approach uses DLL injection (like MacroQuest) rather than PostMessage — this enables calling internal EQ functions directly, accessing the navigation mesh for pathfinding, and writing to game memory. The MQ2 reference source (`mq2-reference/`) is used both for struct offsets and as architectural reference for hooking patterns.

### New module structure (M2.5-M4)

- **`dmft-common/src/`** — Shared types across all crates
  - `nav.rs`: Waypoint, NavStatus, CampSpot, IndexedQueue<T>, Xorshift32 PRNG, KNUTH_HASH
  - `combat.rs`: CombatStatus, CombatRole, ClassStrategy types, HolyShit conditions, SpellEntry
  - `login.rs`: LoginPhase, LoginError, AccountInfo
  - `offset_db.rs`: Hot-updatable offset database (JSON load/save)
  - `ipc.rs`: Command/Response enums for all IPC (nav + combat + login)
- **`dmft-dll/src/nav/`** — DLL-side navigation engine
  - `state.rs`: Navigator FSM, `mod.rs`: global singleton + tick integration
  - `stuck.rs`: StuckDetector, `humanize.rs`: MovementPersonality, `waypoint.rs`: WaypointQueue
- **`dmft-dll/src/combat/`** — DLL-side combat engine
  - `state.rs`: Combatant FSM, `strategy.rs`: ClassStrategy trait + CombatContext
  - `classes/`: warrior, cleric, enchanter, generic_dps implementations
  - `holyshit.rs`: conditional ability evaluator, `gcd.rs`: GCD tracker, `mana.rs`: ManaGovernor
  - `puller.rs`: pull cycle FSM, `aggro.rs`: heading-based aggro detection
- **`dmft/src/nav/`** — Orchestrator-side navigation
  - `recorder.rs`: WaypointRecorder + RDP simplification, `camp.rs`: CampManager, `router.rs`: zone routing
- **`dmft/src/combat/`** — Orchestrator-side combat coordination
  - `coordinator.rs`: assist target broadcasting, CC assignment, `spell_db.rs`: static spell data
- **`dmft/src/launcher/`** — Login automation
  - `login_sm.rs`: per-client login FSM, `coordinator.rs`: staggered launch orchestration
  - `spawner.rs`: CreateProcessW wrapper, `post_login.rs`: group→buff→camp sequencer
- **`dmft/src/credentials/`** — Encrypted credential store
  - `crypto.rs`: Argon2id + AES-256-GCM, `store.rs`: SQLite backend, `prompt.rs`: master password
