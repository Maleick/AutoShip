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

## EQ Client Optimization (Multiboxing)

For 30+ clients on a single machine, background clients must be configured for minimal resource usage. Apply these settings in each background client's `eqclient.ini`:

```ini
[Defaults]
StickerFigures=1          # Stick figure models — massive RAM reduction
ClipPlane=0.5             # Minimum draw distance
SpellEffects=0            # No spell particles
ShowNPCNames=FALSE        # Reduce UI overhead

[Display]
Width=640                 # Minimum resolution
Height=480
BackgroundFPS=0.0001      # Freeze rendering when not focused
ClientCore=-1             # Let OS handle CPU scheduling (don't pin to a core)

[Sound]
SoundEnabled=0            # Disable all sound
BGSoundEnabled=0          # No background audio
MusicEnabled=0
```

### Windows System Tweaks

| Setting          | Value                                                                       | Why                                |
| ---------------- | --------------------------------------------------------------------------- | ---------------------------------- |
| Desktop Heap     | `SharedSection=1024,32768,2048` in registry                                 | Prevents crashes above ~20 windows |
| iGPU VRAM (BIOS) | Pre-allocated: 512MB–1GB, Max shared: 8GB                                   | Frees RAM for EQ clients           |
| Pagefile         | 8–16 GB on NVMe (or System Managed)                                         | Required even with 64GB RAM        |
| CPU Affinity     | [Process Lasso](https://bitsum.com/) — reserve cores 0–1 for OS/Frostreaver | Stable scheduling for 30 clients   |

Registry path for desktop heap: `HKLM\System\CurrentControlSet\Control\Session Manager\SubSystems\Windows`

### Expected Resource Usage (30 clients)

| Mode                               | RAM per Client | Total (30) | GPU VRAM       |
| ---------------------------------- | -------------- | ---------- | -------------- |
| Optimized (stick figures, 640x480) | ~300–500 MB    | ~9–15 GB   | ~4–6 GB shared |
| Default settings                   | ~1.0–1.5 GB    | ~30–45 GB  | ~8–12 GB       |

## Roadmap

- [x] **M1** — Memory reading + TUI dashboard
- [ ] **M2** — Input dispatch (PostMessage keystroke sending to EQ windows) + multi-client manager
- [ ] **M3+** — Group coordination, automated assist trains, buff rotations

## License

Private project.
