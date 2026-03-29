# Session Handoff — 2026-03-29 Deep Night Sprint

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

**Prompt to start next session:**
```
Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off.
```

## What Was Done This Session (MASSIVE)

### DLL Injection — WORKING LIVE
- First successful DLL injection into live eqgame.exe (2026-03-29 00:24 UTC)
- InterpretCmd confirmed: /sit, /stand, /invite, /target, /follow all executed
- Two characters grouped and following each other via remote commands
- Command pipeline: `dmft.exe --cmd <pid> "/slash_command"` → pipe → DLL → InterpretCmd
- `dmft.exe --inject` finds processes, stages DLL with random name, injects
- Window renaming: DLL renames to "EQ - CharName (ZoneName)"

### TUI — 5 Screens + Command Bar
- **Dashboard (1)**: Character grid with HP/mana/state/zone
- **Spawns (2)**: Full spawn list with search (`/`) and filter cycling (`f`)
- **Character (3)**: Detail view with pixel art class emblem sprites
- **Map (4)**: Brewall zone geometry with Bresenham line rasterization, spawn overlay, named mob panel, legend
- **Groups (5)**: 2x3 grid of 6 groups (Alpha-Foxtrot) with member status
- **Command bar (`:`)**: Send commands to PIDs, broadcast `all /cmd`, camp control
- **Privacy mode (`p`)**: Redacts character names + server

### Camp Loop + Orchestrator
- 5-phase state machine: Idle → Pull → Fight → Loot → Med
- Orchestrator ticks camp loop, sends slash commands via IPC
- TOML camp configs (camp center, pull point, radius, mana thresholds)
- TUI commands: `:camp start <name>`, `:camp stop`, `:camp status`
- 16 class ability configs (all EQ classes) with cooldowns, priorities, conditions
- Bard uses /melody (TLP native twist)

### Game State Publishing
- DLL reads local player + target every tick (HP, mana, position, class, level)
- Nearby spawns read every 30 ticks (500 unit radius, 100 cap, linked list walk)
- Published to shared memory via IPC for orchestrator consumption
- Command jitter: 1-10 tick random delay via Xorshift32 PRNG

### Named Spawn Tracker + HVT
- Named mob detection (filters "a "/"an "/"the " prefix names)
- HVT watchlist: config/hvt_watchlist.toml with 10 classic named mobs
- Up/down alerts with respawn timer estimation
- Map shows ! for named, X for dead named spawn locations

### Supporting Systems
- EQ log parser: loot/kill/money/XP/death/zone events, LootDatabase stats
- GM flag reading (player_zone::GM offset 0x03ec)
- Anti-detection: command jitter, Warden research doc, string audit
- Offset calibration: CHAR_CLASS→0x0FDC, group member offsets, zone info offsets
- Pixel art sprites: half-block renderer with 16 class emblems
- 1707 Brewall map files installed on frostreaver
- Navmeshes downloaded: Classic, Kunark, Velious, Luclin, PoP

### Research Docs
- docs/orchestration-design.md — 7-phase plan, group model, camp loop
- docs/anti-detection.md — Warden research, mitigation strategies
- docs/redguides-automation-research.md — CWTN, KissAssist, camp loops
- docs/mq2-deep-dive.md — nav, combat, stick/follow gap analysis
- docs/eq-maps-research.md — Brewall format, coordinate transform
- docs/wineq-research.md — render strobing, window management
- docs/eq-ini-optimization.md — 4-tier settings, memory budgets
- docs/roadmap-review.md — milestone reorder, anti-detection priorities
- docs/code-review-session3.md — 3 critical (2 fixed), 5 important, 6 minor
- docs/dll-injection-plan.md — injection sequence, integration loop

### Scripts
- scripts/launch_eq.bat — launches 6 EQ clients with stagger
- scripts/inject_test.bat — one-click injection
- scripts/verify_injection.bat — 4-point verification
- scripts/inject_and_group.bat — inject + group helper
- scripts/cmd_all.bat — broadcast command to all clients
- scripts/optimize_ini.ps1 — apply minimal INI settings

### Stats
- 340 tests passing across 3 crates
- ~15,000 lines added this session
- 16 commits

## Known Issues

### STANDSTATE offset still wrong
- 0x0574 reads wrong value (shows FD when sitting)
- Needs hex dump scan on live client

### StickFigures=1 not working
- INI setting didn't take effect — may need different key or section

### Pipe response not wired
- Orchestrator uses fire-and-forget — no confirmation from DLL

### Game state → orchestrator not wired
- DLL publishes to shared memory, but orchestrator doesn't read it yet
- Camp loop still uses timers, not real HP/mana values
- Need SharedStateReader on orchestrator side

## Immediate Next Steps (Testing Priority)

1. **Test window renaming**: Relaunch + inject → verify windows show "EQ - CharName (zone)"
2. **Test map rendering**: Press 4 in TUI → verify Freeport/PoK zone geometry shows
3. **Test command bar**: Press `:` → type `<pid> /sit` → verify execution
4. **Test camp loop**: `:camp start test_camp` (need to create a test camp config first)
5. **Wire SharedStateReader**: Orchestrator reads game state from shared memory for smart camp decisions
6. **Render strobing**: Hook CDisplay::RealRender_World for background clients

## Architecture

- **DLL injection**: `target\release\dmft_dll.dll` staged with random name → CreateRemoteThread + LoadLibraryW
- **Command pipeline**: TUI `:` command or `dmft.exe --cmd <pid> "/cmd"` → pipe → DLL → InterpretCmd
- **Game state**: DLL reads EQ memory → publishes GameState to shared memory → orchestrator reads (TODO)
- **Camp loop**: Orchestrator ticks state machine → generates (pid, slash_cmd) pairs → sends via IPC
- **Maps**: Brewall .txt files in config/maps/ → parsed by map_parser.rs → rasterized with Bresenham
- **Logs**: DLL → %TEMP%\dmft\dmft-dll.log, Orchestrator → logs/dmft.log

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log`
- Launch: `C:\Users\xmale\Desktop\launch_eq.bat`
- Local IP: 192.168.1.130 (Tailscale: 100.121.123.85)

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`
