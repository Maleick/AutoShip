# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Frostreaver** is a Rust-based external memory reader for EverQuest (targeting a 36-box multibox setup on the Frostreaver TLP server). It reads EQ client process memory via Windows APIs to extract live game state (spawns, player HP/mana/position, target info) and displays it in a terminal UI.

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
  - `window.rs`: `find_windows_by_title` — window enumeration for future input dispatch (M2)
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

The project follows a milestone-based plan. M1 (memory reading + TUI) is largely complete. M2 (input dispatch / window sending) is next. Group definitions in the config are scaffolding for later milestones.
