# Frostreaver (DMFT)

External process memory reader, DLL injector, and multibox controller for EverQuest, built in Rust.

Frostreaver reads live game state from EQ client memory, injects a DLL for direct control via internal function calls (InterpretCmd), and orchestrates up to 36 characters across a TLP multibox setup.

## Status

**DLL injection + command execution confirmed working on live eqgame.exe** (March 2026 build). Login automation (Phases 1-3) working end-to-end: credential entry, server select, character select, Enter World. Two characters successfully grouped, following, sitting/standing via remote commands.

## Features

### Core

- **DLL Injection** — Rust `cdylib` injected via CreateRemoteThread + LoadLibraryW, staged with randomized names
- **InterpretCmd** — Calls EQ's internal `CEverQuest::InterpretCmd` to execute any slash command invisibly
- **Game State Publishing** — DLL reads HP/mana/target/nearby spawns every tick, publishes via shared memory
- **IPC Pipeline** — Named pipes (commands) + shared memory (game state) with current-user DACL security
- **Render Strobing** — Hooks `CDisplay::RealRender_World`, skips 3D rendering for background clients (~97% GPU savings)

### TUI Dashboard (6 screens, 3 themes)

| Screen     | Key | Description                                                                        |
| ---------- | --- | ---------------------------------------------------------------------------------- |
| Dashboard  | `1` | All characters overview, group health bars, session stats (XP/hr, plat/hr)         |
| Spawns     | `2` | Full spawn list with live search (`/`), type filter (`f`: All/PC/NPC/Named)        |
| Character  | `3` | Selected character detail with pixel art class emblem sprites                      |
| Map        | `4` | Zone geometry (Brewall maps), spawn overlay, named mob tracker with respawn timers |
| Groups     | `5` | 6-group dashboard (2x3 grid) with member status                                    |
| Navigation | `6` | Per-character nav status, operating mode, command reference                         |

**Themes:** Dark Modern (default), Dracula, Classic — cycle with `t`

### TUI Controls

| Key       | Action                                                |
| --------- | ----------------------------------------------------- |
| `1-5`     | Switch screens                                        |
| `[` / `]` | Cycle between EQ clients                              |
| `/`       | Search/filter spawns (live typing)                    |
| `f`       | Cycle spawn type filter                               |
| `p`       | Privacy mode (redacts names + server for screenshots) |
| `:`       | Command mode (Tab completion, command history)        |
| `?`       | Help overlay                                          |
| `t`       | Cycle theme (Dark / Dracula / Classic)                |
| `q`       | Quit                                                  |

### Command Bar (`:` mode)

```
:<pid> /sit              Send slash command to a PID
:all /sit                Broadcast to all clients
:camp start <name>       Start camp loop from config
:camp stop               Stop camp loop
:camp status             Show camp state
:login all               Launch all configured accounts
:login G1                Launch group 1 accounts
:help                    Show all commands
```

### Camp Loop Automation

- **5-phase state machine**: Idle → Pull → Fight → Loot → Med
- **Smart decisions** from real game state (HP/mana-driven, not timers)
- **16 class ability configs** (TOML) with cooldowns, priorities, conditions
- **CC system**: Charm/mez tracking, Tash→Malo debuff chain, charm break emergency response
- **Rogue backstab positioning**: Calculates behind-target position using EQ heading math
- **Intelligent pull target selection**: Filters by distance/type, prefers HVT watchlist targets
- **Buff maintenance**: Tracks durations, auto-rebuffs during idle/med
- **Death recovery**: Detects deaths, cleric rez commands, rebuff sequence
- **Sell/bank cycle**: Navigate to vendor, sell, return to camp

### Anti-Detection

- **CSPRNG session tokens** (not PID-derived)
- **Randomized DLL staging names** (CSPRNG filename, not static)
- **Restrictive pipe DACL** (current user only)
- **Note:** IPC pipe names currently use a static `dmft_` prefix (randomized session-GUID names are planned)
- **Human-like command jitter** (triangle distribution + hesitation spikes)
- **Per-character personality profiles** (reaction speed, aggression, discipline variation)
- **GM flag detection** (alerts on GM spawns)
- **Render strobing** (background clients at ~5fps, not zero)

### Named Spawn Tracker

- Detects named mobs (filters generic "a goblin" names)
- HVT watchlist (`config/hvt_watchlist.toml`) with Discord alert support
- Tracks up/down status with respawn timer estimation
- Map shows `!` for live named, `X` for dead with countdown

### EQ Log Parser

- Parses loot, kills, money, XP, deaths, zone changes from EQ log files
- Live session stats on TUI dashboard (XP/hr, plat/hr, top items)
- Feeds into LootDatabase for economy analysis

## Architecture

```
DMFT Workspace (3 crates)
├── dmft/           — Orchestrator: TUI, camp loop, process reading, injection
├── dmft-dll/       — Injected DLL: hooks, game state reader, IPC, render strobing
└── dmft-common/    — Shared types: IPC, offsets, combat/nav/soul types
```

### Command Pipeline

```
TUI :command  →  Orchestrator  →  Named Pipe  →  DLL  →  InterpretCmd  →  EQ
     or                                                    (invisible to game)
Discord msg
```

### Camp Loop

```
Orchestrator ticks camp loop → reads game state from shared memory →
generates (pid, slash_command) pairs per role → sends via IPC pipe →
DLL executes InterpretCmd with human-like jitter delay
```

## Quick Start

### Development (any platform — demo mode)

```bash
cargo build              # Debug build
cargo run                # TUI with demo data (auto-detected on non-Windows)
cargo test               # Run the full workspace test suite
cargo clippy --all-targets --all-features -- -D warnings
```

**Demo mode** activates automatically when no live EQ process is found (always on macOS/Linux, on Windows when EQ isn't running). It populates the TUI with 18 simulated characters across 3 groups covering all 16 EQ classes:

| Group | Zone | Classes |
|-------|------|---------|
| G1 | Permafrost | WAR, CLR, ENC, BRD, RNG, WIZ |
| G2 | Eastern Wastes | SK, SHM, DRU, ROG, NEC, MAG |
| G3 | Great Divide | PAL, MNK, BST, BER, CLR, WIZ |

Each zone has NPC spawns (including named bosses like Lady Vox, Wuoshi, Garudon), corpses, and realistic HP/mana values. This lets you develop and test all TUI screens without a live EQ client.

### Production (Windows — live EQ)

```powershell
# Build
cargo build --release

# Launch EQ clients
scripts\launch_eq.bat

# Inject DLL into all running EQ clients
target\release\dmft.exe --inject

# Send a slash command to a specific client
target\release\dmft.exe --cmd <pid> "/sit"

# Broadcast to all clients
scripts\cmd_all.bat "/sit"

# Run TUI dashboard
target\release\dmft.exe
```

### Log Files

- **Orchestrator:** `./logs/dmft.log` (daily rolling)
- **DLL:** `%TEMP%/dmft/dmft-dll.log` (daily rolling)

## Configuration

### Accounts (`config/accounts.toml`)

```toml
[[accounts]]
name = "frostreaver01"
server = "Firiona Vie"
character = "Camrene"
class = "WAR"
group = 1
```

### Camp Configs (`config/camps/*.toml`)

```toml
name = "crushbone_entrance"
zone = "crushbone"
camp_center = [500.0, -200.0, 3.0]
pull_point = [550.0, -180.0, 3.0]
pull_radius = 150.0
camp_radius = 30.0
rest_mana_pct = 20
pull_mana_pct = 60
```

### Class Ability Configs (`config/classes/*.toml`)

16 classes: WAR, CLR, PAL, RNG, SK, DRU, MNK, BRD, ROG, SHM, NEC, WIZ, MAG, ENC, BST, BER

### HVT Watchlist (`config/hvt_watchlist.toml`)

```toml
[[targets]]
name = "Emperor Crush"
zone = "crushbone"
priority = "high"
alert_discord = true
```

### EQ Client Optimization

Run `scripts\optimize_ini.ps1` to apply minimal settings:

- StickFigures=1, Shadows=0, MaxBGFPS=10, AllLuclinPcModelsOff=1
- Expected: ~500MB RAM per client (down from ~2GB)

## Roadmap

### Completed

- [x] **M1** — External memory reading + TUI dashboard
- [x] **M2** — DLL injection + function hooking + IPC
- [x] **M2.5** — Login automation + credential store
- [x] **M3** — Navigation — waypoint pathfinding, movement humanization
- [x] **M4** — Combat automation — class strategies, puller FSM
- [x] **M5** — Soul Engine — personality traits, persistent memory

### Active Development (Phase 1-6)

- [x] Phase 1: Command foundation (DLL injection, InterpretCmd, grouping)
- [x] Phase 2: Camp loop (state machine, smart HP/mana decisions)
- [x] Phase 3: Class configs + CC system + positioning
- [x] Phase 4: Autonomy (sell/bank, death recovery, buff maintenance)
- [x] Phase 5: Navigation (navmesh pathfinding via Detour, 888-zone BFS routing)
- [ ] Phase 6: Anti-detection hardening (reflective injection, string obfuscation)
- [ ] Phase 7: TLP launch readiness

### Future

- [ ] **M6** — LLM-driven character optimization
- [ ] **M7** — Reinforcement learning from camp data
- [ ] **M8** — Economy automation (Krono farming loop)

## Research Docs

- `docs/orchestration-design.md` — 7-phase plan, group model, camp loop design
- `docs/anti-detection.md` — Warden research, mitigation strategies
- `docs/redguides-automation-research.md` — KissAssist, CWTN, camp loop patterns
- `docs/mq2-deep-dive.md` — MQ2Nav, combat, stick/follow analysis
- `docs/eq-maps-research.md` — Brewall format, coordinate transform
- `docs/eq-ini-optimization.md` — 4-tier INI settings, memory budgets
- `docs/dll-injection-plan.md` — Injection sequence, integration loop
- `docs/wineq-research.md` — Render strobing, window management
- `docs/roadmap-review.md` — Milestone priorities, risk assessment
- `docs/code-review-session3.md` — Code audit findings

## Requirements

- **Rust** (edition 2024; nightly MSVC toolchain currently required on Windows because `retour` uses unstable features)
- **Windows** for live EQ interaction (macOS/Linux for development only)
- **EverQuest** client (March 2026 build confirmed)

## License

Private project.
