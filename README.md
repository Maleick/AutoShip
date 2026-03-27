# Frostreaver

External process memory reader and multibox controller for EverQuest, built in Rust.

Frostreaver reads live game state (spawns, HP/mana/position, targets) directly from EQ client memory and displays it in a terminal-based dashboard. Designed to manage up to 36 characters across a TLP multibox setup.

## Features

- **Live memory reading** — attaches to `eqgame.exe` processes via Windows API (`ReadProcessMemory`) to extract player, target, and spawn data in real-time
- **Terminal UI dashboard** — ratatui-based interface with spawn list, player/target panels, hex memory viewer, and filtering
- **Cross-platform development** — builds and runs on macOS with demo data stubs; full functionality on Windows
- **Spawn linked list traversal** — walks EQ's internal `TList<PlayerClient*>` to enumerate all entities in a zone
- **Configurable groups** — TOML-based group/toon definitions for organizing a multibox roster

## Requirements

- **Rust** (edition 2024)
- **Windows** for live EQ memory reading (macOS/Linux supported for development with demo data)

## Quick Start

```bash
# Build
cargo build

# Run TUI dashboard (demo mode on macOS, live on Windows)
cargo run

# One-shot CLI dump of player/spawn data
cargo run -- --dump
```

## Configuration

Edit `config/frostreaver.toml`:

```toml
process_name = "eqgame.exe"
max_spawns = 2048

[[group]]
id = 1
name = "G1_Tank"

[[group.toon]]
name = "YourCharacter"
class = "WAR"
role = "main_tank"
```

## TUI Keybindings

| Key                | Action                               |
| ------------------ | ------------------------------------ |
| `q` / `Ctrl+C`     | Quit                                 |
| `Tab`              | Switch panel (spawn list / hex dump) |
| `j` / `k` / arrows | Navigate spawn list                  |
| `Enter`            | Inspect selected spawn's raw memory  |
| `/`                | Filter spawns                        |
| `Esc`              | Clear filter                         |
| `PgUp` / `PgDn`    | Page through spawn list              |

## Architecture

```
src/
  main.rs          — Entry point, two modes: TUI (default) and dump (--dump)
  config.rs        — TOML config loading (process name, groups, toons)
  eq/
    offsets.rs     — Memory addresses & struct field offsets from MQ2/eqlib headers
    structs.rs     — SpawnInfo, EqClass, SpawnType data types
    spawn.rs       — Spawn reading: local player, target, linked list traversal
  process/
    memory.rs      — ProcessHandle: open, read, pointer chasing, string reads
    window.rs      — Window enumeration by title (for future input dispatch)
  tui/
    app.rs         — Application state, filtering, navigation
    ui.rs          — Panel rendering (header, player, target, hex dump, spawn list)
    run.rs         — Event loop, terminal setup, data refresh, demo data
    event.rs       — Keyboard input handling
```

### How Memory Reading Works

EQ stores game entities in a linked list managed by `PlayerManagerClient`. Frostreaver:

1. Finds `eqgame.exe` by process name and opens it with `PROCESS_VM_READ`
2. Locates the module base address via `EnumProcessModulesEx`
3. Rebases known pointer addresses from MQ2 headers (preferred base `0x140000000`) to the actual runtime base
4. Reads global pointers (`pinstLocalPlayer`, `pinstTarget`, `pinstSpawnManager`)
5. Walks the spawn linked list, reading individual fields (name, HP, class, position, etc.) at their struct offsets

Offsets are derived from the [MacroQuest](https://github.com/macroquest/macroquest) source headers (`eqgame.h`, `PlayerClient.h`).

## Roadmap

- [x] **M1** — Memory reading + TUI dashboard
- [ ] **M2** — Input dispatch (PostMessage keystroke sending to EQ windows) + multi-client manager
- [ ] **M3+** — Group coordination, automated assist trains, buff rotations

## License

Private project.
