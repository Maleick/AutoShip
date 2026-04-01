# DMFT (Dave Mike Fun Times)

[![CI](https://github.com/Maleick/DMFT/actions/workflows/ci.yml/badge.svg)](https://github.com/Maleick/DMFT/actions/workflows/ci.yml)
[![Release](https://github.com/Maleick/DMFT/actions/workflows/release.yml/badge.svg)](https://github.com/Maleick/DMFT/actions/workflows/release.yml)
[![Rust](https://img.shields.io/badge/rust-edition%202024-orange?style=flat-square)](https://www.rust-lang.org/)
[![Rust LOC](https://img.shields.io/badge/Rust%20LOC-74%2C122-blue?style=flat-square)](#testing)
[![Tests](https://img.shields.io/badge/Tests-1%2C766%20exact-brightgreen?style=flat-square)](#testing)
[![Status](https://img.shields.io/badge/status-Active-green?style=flat-square)](#roadmap)
[![License](https://img.shields.io/badge/license-Private-red?style=flat-square)](#license)

External process memory reader, DLL injector, and multibox controller for EverQuest, built in Rust.

DMFT reads live game state from EQ client memory, injects a DLL for direct control via internal function calls (InterpretCmd), and orchestrates up to 36 characters across a TLP multibox setup.

If you plan to do offset, struct, or MacroQuest reference work, clone with submodules:

```bash
git clone --recurse-submodules https://github.com/Maleick/DMFT.git
cd DMFT

# Existing clone
git submodule update --init --recursive
```

Routine `cargo build` / `cargo test` work does not require the reference trees, but `third_party/eqlib` and `third_party/macroquest` are the canonical local sources for reference work. See `third_party/README.md` for the layout.

## Status

**DLL injection + command execution confirmed working on live eqgame.exe** (March 2026 build). Login automation (Phases 1-3) working end-to-end: credential entry, server select, character select, Enter World. Two characters successfully grouped, following, sitting/standing via remote commands.

## Features

### Core

- **DLL Injection** — Rust `cdylib` injected via CreateRemoteThread + LoadLibraryW, staged with randomized names
- **InterpretCmd** — Calls EQ's internal `CEverQuest::InterpretCmd` to execute any slash command invisibly
- **Game State Publishing** — DLL reads HP/mana/target/nearby spawns every tick, publishes via shared memory
- **IPC Pipeline** — Named pipes (commands) + shared memory (game state) with current-user DACL security
- **Render Strobing** — Hooks `CDisplay::RealRender_World`, skips 3D rendering for background clients (~97% GPU savings)

### TUI Dashboard (4 screens, 3 themes)

| Screen     | Key | Description                                                                                         |
| ---------- | --- | --------------------------------------------------------------------------------------------------- |
| Characters | `1` | Operator roster, selected character detail with class emblem sprites, toggleable group/scope panels |
| Map        | `2` | Zone geometry (Brewall maps), spawn overlay, named mob tracker with respawn timers, Z-slice control |
| Navigation | `3` | Per-character nav status, operating mode, waypoint queue                                            |
| Debug      | `4` | Full spawn list with live search, type filter (All/PC/NPC/Named), hex dump, target detail           |

**Themes:** Dark Modern (default), Dracula, Classic — cycle with `T`

**Widget library:** Sparklines, gauge bars, scrollable lists, tooltips, badges, notification area, inline hints, multi-option selectors, and scrollbar indicators. Context-sensitive help overlay with per-screen keybinding hints, did-you-mean suggestions for commands, and a comprehensive scrollable reference.

### TUI Controls

| Key         | Action                                                |
| ----------- | ----------------------------------------------------- |
| `1-4`       | Switch screens                                        |
| `Shift+1-6` | Focus group G1-G6                                     |
| `Shift+0`   | All groups (clear group focus)                        |
| `Tab`       | Cycle focused pane                                    |
| `[` / `]`   | Cycle between EQ clients                              |
| `/`         | Search spawns (live typing)                           |
| `f`         | Cycle spawn type filter                               |
| `g`         | Toggle group section (Characters screen)              |
| `v`         | Toggle scope section (Characters screen)              |
| `z`         | Collapse focused section                              |
| `+` / `-`   | Adjust Tactical Z slice                               |
| `m`         | Maximize Tactical map                                 |
| `p`         | Privacy mode (redacts names + server for screenshots) |
| `T`         | Cycle theme                                           |
| `:`         | Command mode                                          |
| `?`         | Help overlay (scrollable, context-sensitive)          |
| `q`         | Quit                                                  |

**Status Glyphs:** `⚔`/`✚`/`✦` Fight/Heal/Cast · `➜`/`✓`/`!` Navigate/Arrived/Stuck · `☾`/`⇣`/`⌕` Sit/Feign/Loot

### Command Bar (`:` mode)

```
:<name> /sit             Send slash command to character
:G1-G6 /cmd             Send to group
:all /sit                Broadcast to all clients
:camp start|stop|list    Camp loop control
:camp add|rm             Add/remove camp config
:nav <dest>              Navigate to camp, coords, or slash fallback
:track <name>            Track a spawn
:ma <name>               Set Main Assist
:mt <name>               Set Main Tank
:engage / :disengage     Start/stop combat
:invite <name>           Group invite
:accept                  Accept group invite
:mode camp|hunt          Set operating mode
:ch start <pids> <int>   Start CH chain
:ch stop|add|rm          CH chain management
:ch adaptive on|off      Adaptive CH timing
:help                    Show all commands
```

### Camp Loop Automation

- **5-phase state machine**: Idle -> Pull -> Fight -> Loot -> Med
- **Smart decisions** from real game state (HP/mana-driven, not timers)
- **16 class ability configs** (TOML) with cooldowns, priorities, conditions
- **CC system**: Charm/mez tracking, Tash->Malo debuff chain, charm break emergency response
- **Rogue backstab positioning**: Calculates behind-target position using EQ heading math
- **Intelligent pull target selection**: Filters by distance/type, prefers HVT watchlist targets
- **Buff maintenance**: Tracks durations, auto-rebuffs during idle/med
- **Death recovery**: Detects deaths, cleric rez commands, rebuff sequence
- **Sell/bank cycle**: Navigate to vendor, sell, return to camp

### Soul Engine

- **Personality system** — Per-character mood, traits, and behavioral profiles with deterministic personality engine
- **Persistent memory** — SQLite-backed memory database for long-term character state
- **Social dynamics** — Social graph tracking relationships between characters
- **Idle behavior** — Personality-driven actions during downtime
- **LLM integration** — Async request queue for Claude/Gemini-driven character responses (M6 — active development)

### Login Automation

- **Credential store** — Argon2id + AES-256-GCM encrypted credentials in SQLite
- **Login FSM** — Automated login state machine: credential entry, server select, character select, Enter World
- **Launch coordinator** — Staggered multi-client launch with post-login sequencing
- **Process spawner** — Spawns and manages EQ client processes

### Navigation

- **Navmesh pathfinding** — Detour-based pathfinding via C++ FFI shim
- **888-zone BFS routing** — Zone-to-zone route planning across the full EQ world
- **Navigator FSM** — Waypoint following with stuck detection and recovery
- **Movement humanization** — Natural-looking movement patterns
- **Waypoint recording** — RDP simplification for path recording

### Combat

- **17 class strategies** — ClassStrategy trait with per-class implementations including generic DPS fallback
- **Puller FSM** — Automated pull cycle with target selection and aggro management
- **HolyShit system** — Emergency response conditions (low HP, charm break, adds)
- **GCD tracker + mana governor** — Intelligent ability timing and resource management
- **CH chain** — Coordinated Complete Heal rotation with adaptive timing
- **Skill cooldown tracking** — Per-ability cooldown management across all classes

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
DMFT Workspace (3 crates, ~57K lines of Rust)
├── dmft/           — Orchestrator: TUI, camp loop, process reading, injection, soul engine
├── dmft-dll/       — Injected DLL: hooks, game state reader, IPC, render strobing, combat
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

| Group | Zone           | Classes                      |
| ----- | -------------- | ---------------------------- |
| G1    | Permafrost     | WAR, CLR, ENC, BRD, RNG, WIZ |
| G2    | Eastern Wastes | SK, SHM, DRU, ROG, NEC, MAG  |
| G3    | Great Divide   | PAL, MNK, BST, BER, CLR, WIZ |

Each zone has NPC spawns (including named bosses like Lady Vox, Wuoshi, Garudon), corpses, and realistic HP/mana values. This lets you develop and test all TUI screens without a live EQ client.

### Production (Windows — live EQ)

```powershell
# Build
cargo build --release

# Inject DLL into all running EQ clients
target\release\dmft.exe --inject

# Send a slash command to a specific client
target\release\dmft.exe --cmd <pid> "/sit"

# Run TUI dashboard
target\release\dmft.exe
```

### Log Files

- **Orchestrator:** `./logs/dmft.log` (daily rolling)
- **DLL:** `%TEMP%/dmft/dmft-dll.log` (daily rolling)

## Testing

Current workspace totals: 74,122 Rust lines and 1,766 exact tests. This line and the badges above are auto-refreshed by `scripts/update_readme_metrics.py`. CI runs on every push to master:

| Platform | Jobs                      |
| -------- | ------------------------- |
| macOS    | fmt + clippy + test       |
| Windows  | build (nightly toolchain) |

Tag-triggered releases (`v*`) build Windows binaries and create GitHub Releases automatically.

## Configuration

### Accounts (`config/accounts.toml`)

```toml
[[accounts]]
name = "dmft01"
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
- [x] **M2** — DLL injection + function hooking + IPC + self-healing monitor
- [x] **M2.5** — Login automation + encrypted credential store + launch coordinator
- [x] **M3** — Navigation — navmesh pathfinding (Detour), 888-zone BFS routing, movement humanization
- [x] **M4** — Combat automation — 17 class strategies, puller FSM, HolyShit system, CH chain
- [x] **M5** — Soul Engine — personality traits, persistent memory, social dynamics, idle behavior

### Next

- [ ] **M6** — LLM Character AI — API integration (Gemini/Claude), in-game chat responses

### Future

- [ ] **M7** — Learning/RL — behavioral cloning, RL fine-tuning
- [ ] **M8** — Economy automation (vendor, EC tunnel trading, Bazaar, Krono farming)

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

## Wiki

The long-lived operator and developer wiki is source-controlled in `docs/wiki/` and published to
the GitHub wiki with `scripts/sync_wiki.py`.

```bash
python3 scripts/sync_wiki.py --check
python3 scripts/sync_wiki.py --dry-run
python3 scripts/sync_wiki.py --push
```

Update the repo-side source files in `docs/wiki/` in the same PRs that change behavior, then
publish the wiki snapshot after review.

## Requirements

- **Rust** (edition 2024; nightly MSVC toolchain currently required on Windows because `retour` uses unstable features)
- **Windows** for live EQ interaction (macOS/Linux for development only)
- **EverQuest** client (March 2026 build confirmed)

## License

Private project.
