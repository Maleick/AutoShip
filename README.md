# Frostreaver (DMFT)

External process memory reader, DLL injector, and multibox controller for EverQuest, built in Rust.

Frostreaver reads live game state from EQ client memory, injects a DLL for direct control (movement, casting, targeting), and orchestrates up to 36 characters across a TLP multibox setup — complete with AI-driven character personalities.

## Features

- **Live memory reading** — attaches to `eqgame.exe` via `ReadProcessMemory` to extract player, target, and spawn data in real-time
- **Terminal UI dashboard** — ratatui-based interface with spawn list, player/target panels, hex viewer, and filtering
- **DLL injection** — Rust `cdylib` injected into each EQ client for internal function hooking and memory writes
- **IPC** — shared memory (game state) + named pipes (commands) between orchestrator and injected DLLs
- **Navigation** — waypoint-based pathfinding with humanized movement, stuck detection, and camp positioning
- **Combat automation** — class-specific strategies (warrior/cleric/enchanter/DPS), HolyShit emergency system, GCD tracking, mana governance
- **Soul Engine** — AI personality system with Big Five + EQ-themed traits, persistent memory, social graph, idle behaviors
- **Login automation** — encrypted credential store (AES-256-GCM + Argon2id), staggered launch, post-login sequencing
- **Cross-platform dev** — builds on macOS with demo data stubs; full functionality on Windows

## Architecture

```
DMFT Workspace (3 crates)
├── dmft/           — Orchestrator: TUI, process reading, launch coordination, combat/nav coordination
├── dmft-dll/       — Injected DLL: hooks, combat FSM, navigator FSM, IPC listener
└── dmft-common/    — Shared types: IPC messages, offsets, combat/nav/soul types
```

### How It Works

```
┌─────────────────────────────────────────────────┐
│  dmft.exe (Orchestrator)                        │
│  ├── TUI Dashboard (ratatui)                    │
│  ├── Process Discovery (find eqgame.exe)        │
│  ├── Memory Reading (ReadProcessMemory)         │
│  ├── DLL Injection (CreateRemoteThread)         │
│  ├── Combat Coordinator (assist targets, CC)    │
│  ├── Nav Router (zone routing, camp management) │
│  ├── Soul Coordinator (personalities, memory)   │
│  └── Launch Coordinator (login, credentials)    │
│            │ IPC (shared mem + pipes)            │
│            ▼                                     │
│  ┌─────────────────────────────────┐ x36 clients│
│  │  dmft_dll.dll (per EQ client)   │            │
│  │  ├── Game Loop Hook             │            │
│  │  ├── Movement Controller        │            │
│  │  ├── Targeting Controller       │            │
│  │  ├── Casting Controller         │            │
│  │  ├── Combatant FSM              │            │
│  │  ├── Navigator FSM              │            │
│  │  └── IPC Publisher/Listener     │            │
│  └─────────────────────────────────┘            │
└─────────────────────────────────────────────────┘
```

## Quick Start

### Development (macOS — demo mode)

```bash
cargo build              # Debug build
cargo run                # TUI with demo data
cargo run -- --dump      # One-shot CLI dump
cargo clippy             # Lint
cargo fmt --check        # Check formatting
```

### Windows Setup (for testing with EQ)

```powershell
# Open PowerShell as Admin
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
git clone https://github.com/Maleick/DMFT.git
cd DMFT
.\scripts\setup-windows.ps1    # Installs Rust, VS Build Tools, builds everything
.\scripts\test-windows.ps1     # Runs automated tests
```

### Log Files

- **Orchestrator:** `./logs/dmft.log` (daily rolling)
- **DLL:** `%TEMP%/dmft/dmft-dll.log` (daily rolling)
- Set `RUST_LOG=debug` for verbose output

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

### EQ Client Optimization

For multiboxing, apply `config/eqclient_multibox.ini` to each background client's `eqclient.ini`. Key savings:

| Setting                     | Impact                                  |
| --------------------------- | --------------------------------------- |
| `Sound=FALSE`               | ~30-50MB RAM per client                 |
| `AllLuclinPcModelsOff=TRUE` | ~100-200MB RAM per client               |
| `MaxBGFPS=10`               | ~60-80% GPU reduction when backgrounded |
| All particles = 0           | ~5-15% CPU in group/raid content        |
| `640x480` windowed          | ~50-80MB VRAM per client                |

**Set eqclient.ini to read-only** after configuring to prevent EQ from reverting settings.

## Roadmap

- [x] **M1** — External memory reading + TUI dashboard
- [x] **M2** — DLL injection + internal function hooking + IPC + self-healing monitor
- [x] **M2.5** — Login automation + encrypted credential store + launch coordinator
- [x] **M3** — Navigation — waypoint pathfinding, humanized movement, stuck detection, camp positioning
- [x] **M4** — Combat automation — class strategies, HolyShit system, puller FSM, combat coordinator
- [x] **M5** — Soul Engine — personality traits, persistent memory, social graph, idle behavior (Phase 1)
- [ ] **M6** — LLM Character AI — scheduled Claude optimization of character behaviors
- [ ] **M7** — Learning — Claude-as-optimizer with telemetry feedback loops
- [ ] **M8** — Economy — vendor automation, EC tunnel trading, Bazaar

## Offsets

Memory addresses are derived from [MacroQuest eqlib](https://github.com/macroquest/eqlib) (`live` branch). Current client date: **March 10, 2026**.

Offsets change with every EQ patch. The hot-updatable offset database (`dmft-common/src/offset_db.rs`) supports JSON-based offset loading without recompilation.

## Requirements

- **Rust** (edition 2024, stable MSVC toolchain on Windows)
- **Windows** for live EQ interaction (macOS/Linux for development only)
- **Visual Studio Build Tools** (C++ workload) for MSVC linker

## License

Private project.
