# Session Handoff — 2026-03-29 Late Night

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

**Prompt to start next session:**
```
Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off.
```

## What Was Done This Session

### Major Milestone: DLL Injection + Command Execution WORKING
- **First successful DLL injection** into live eqgame.exe (2026-03-29 00:24 UTC)
- **InterpretCmd working**: `/sit`, `/stand` executed on both clients via IPC pipe
- **Command pipeline**: `dmft.exe --cmd <pid> "/command"` → Named Pipe → DLL → InterpretCmd → game action
- EQ base: 0x7ff78f000000, hook at ProcessGameEvents (0x7ff78f28e0f0)
- DLL staged with randomized name (e.g. `msvc_svc6946.dll`)
- Logs: `%TEMP%\dmft\dmft-dll.log`

### TUI Multi-Screen Dashboard
- **4 screens**: Dashboard (1), Spawns (2), Character (3), Map (4) — switch with number keys
- **Tab bar** in header shows active screen
- **Search mode**: `/` key types live filter text, `Esc` cancels
- **Filter cycling**: `f` key cycles All → PC → NPC → Named
- **Privacy mode**: `p` key redacts character names + server for screenshots
- **Spawn list scroll**: Fixed — selection stays visible past 20 rows
- **Pixel art sprites**: Half-block renderer (▀▄) with EQ class emblems (warrior swords, cleric ankh, SK skull, etc.) + state animations
- **Map parser**: Brewall .txt format, Bresenham line rasterization, spawn overlay

### Offset Calibration
- **CHAR_CLASS**: 0x0420 → 0x0FDC (ActorClient)
- **STANDSTATE**: 0x0134 → 0x0574 (PlayerZoneClient) — **still reads wrong** (shows FD when sitting, needs hex dump scan)
- **Mana**: correct, but only valid for local player

### DLL Infrastructure
- **MAIN_LOOP_OFFSET**: fixed from 0x0 → 0x28E0F0 (was silently skipping hook)
- **Pipe bug (C3)**: fixed connect→auth→read→disconnect cycle
- **Render skipping**: foreground detection added (GetForegroundWindow every 30 ticks)
- **Shutdown**: split into minimal `shutdown()` (loader lock safe) + `graceful_shutdown()`
- **`dmft.exe --inject`**: finds EQ processes, stages DLL, injects, reports status
- **`dmft.exe --cmd <pid> <command>`**: sends slash commands to injected clients
- **launch_eq.bat**: on desktop, launches both clients with stagger

### Research Completed
- **EQ INI optimization** (`docs/eq-ini-optimization.md`): ~500MB/client with full optimization, PlayNice = render skipping, Job Object memory caps
- **WinEQ 2022** (`docs/wineq-research.md`): render strobing (97.8% GPU reduction), 30 profile limit
- **EQ Maps** (`docs/eq-maps-research.md`): Brewall format, coordinate transform, Rust parser
- **MQ2 Deep Dive** (`docs/mq2-deep-dive.md`): nav, combat, stick/follow gap analysis, 36-box group comp
- **Code Review** (`docs/code-review-session3.md`): 3 critical (2 fixed), 5 important, 6 minor
- **Roadmap Review** (`docs/roadmap-review.md`): reorder Economy before Soul Engine, anti-detection is #1 risk

## Known Issues

### STANDSTATE offset still wrong
- 0x0574 reads 110 (FD) when character is sitting — needs hex dump scan on frostreaver
- Old offset 0x0134 always read 0 (Standing) — neither is correct

### Pipe response not wired
- Orchestrator uses fire-and-forget (send_async) — works but no response/confirmation
- DLL doesn't send Response after executing commands yet

### Session token is predictable
- PID-derived token — any local process can reconstruct it
- Needs random injection-time token for production

### Double injection
- Running `--inject` twice loads DLL twice into same process
- Need guard to prevent re-injection (check if already loaded)

## Immediate Next Steps (Priority Order)

1. **Window renaming**: DLL renames eqgame.exe window to character name (enables PID→name mapping)
2. **Characters meet**: Send `/target` + `/follow` to get two characters together
3. **Group invite**: `/invite` via InterpretCmd
4. **TUI command bar**: `:` mode for typing commands in the TUI
5. **STANDSTATE calibration**: Hex dump scan to find correct offset
6. **INI optimization**: Deploy optimized eqclient.ini to reduce ~2GB → ~500MB per client
7. **Render strobing**: Hook CDisplay::RealRender_World to skip rendering for background clients
8. **Anti-detection**: Research Warden, implement string stripping, innocent mode
9. **Map files**: Download Brewall's maps for Classic through PoP zones

## Architecture Reminders

- **Use TeamCreate** for parallel agent work (visible in tmux splits)
- **DLL injection path**: `target\release\dmft_dll.dll` → staged with randomized name → CreateRemoteThread + LoadLibraryW
- **Command pipeline**: `dmft.exe --cmd <pid> "/slash_command"` → pipe → DLL → InterpretCmd
- **EQ base**: 0x7ff78f000000 (March 10, 2026 build)
- **Hook**: ProcessGameEvents at base + 0x28E0F0
- **InterpretCmd**: at base + 0x283FB0 — `void InterpretCmd(PlayerClient*, const char*)`
- **IPC pipes**: `\\.\pipe\dmft_cmd_{pid}` — one per injected client
- **Logs**: DLL → `%TEMP%\dmft\dmft-dll.log`, Orchestrator → `logs/dmft.log`

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log`
- Launch script: `C:\Users\xmale\Desktop\launch_eq.bat`
- Inject: `target\release\dmft.exe --inject`
- Send cmd: `target\release\dmft.exe --cmd <pid> "/command"`

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`
