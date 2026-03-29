# Session Handoff — 2026-03-29 Final

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

**Prompt to start next session:**
```
Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off.
```

## Session Stats
- ~25,000+ lines added
- ~30 commits
- 459 tests passing across 3 crates
- 15+ agent team sprints
- 2 peer reviews (GPT + Gemini)
- DLL injection proven live on eqgame.exe

## What Works (Proven Live)

### DLL Injection + Command Execution
- `dmft.exe --inject` finds eqgame.exe, stages DLL with random name, injects
- `dmft.exe --cmd <pid> "/slash_command"` sends commands via IPC pipe
- InterpretCmd calls EQ's internal function — /sit, /stand, /invite, /target, /follow confirmed
- Two characters grouped and following each other
- Window renaming: DLL sets title to "EQ - CharName (zone)"
- Game state publishing: DLL reads HP/mana/target/spawns every tick to shared memory

### TUI (5 screens)
- Dashboard (1): character grid, session stats (XP/hr, plat/hr)
- Spawns (2): full list with search (/), filter cycling (f), scroll
- Character (3): detail view with pixel art class emblem sprites (16 classes)
- Map (4): Brewall zone geometry, spawn overlay, named mob tracker, legend
- Groups (5): 2x3 grid of 6 groups with member status
- Command bar (:): Tab completion, history, camp control
- Help overlay (?): all keybinds and commands
- Privacy mode (p): redacts names + server

### Camp Loop
- 5-phase state machine: Idle → Pull → Fight → Loot → Med
- Smart transitions from real game state (HP/mana-driven)
- Orchestrator sends slash commands via IPC each tick
- 16 class ability configs (TOML) with cooldowns + priorities
- CC system: charm/mez tracking, Tash→Malo debuff chain, charm break response
- Rogue backstab positioning (EQ heading math)
- Intelligent pull target selection (distance, HVT priority)
- Buff maintenance (duration tracking, auto-rebuff during idle/med)
- Death recovery (detect death, cleric rez, rebuff sequence)
- Sell/bank cycle (vendor state machine)
- Per-character personality profiles (anti-synchronicity)
- Human-like command jitter (triangle distribution + hesitation)

### Security + Anti-Detection
- CSPRNG random session tokens
- Randomized IPC pipe/shared memory names (session GUID)
- Restrictive pipe DACL (current user SID only)
- GM flag detection
- Render strobing (skip 3D for background clients, strobe every 5s)
- Command jitter with human-like timing distribution
- DLL staged with randomized system-looking filename

### Data + Maps
- 1707 Brewall map files installed (all EQ zones)
- Navmeshes downloaded: Classic, Kunark, Velious, Luclin, PoP
- EQ log parser: loot/kill/money/XP/death events
- Log watcher tails files in real-time for TUI stats
- HVT watchlist with 10 classic named mobs
- Named spawn tracker with respawn timers

## Known Issues

### Must Fix Before Live XP Testing
- **STANDSTATE offset wrong** (0x0574 reads FD when sitting) — needs hex dump scan
- **StickFigures=1 not working** — INI setting correct but no effect in-game
- **Auto-login password entry** — needs UI widget manipulation (CEditWnd), manual for now
- **CC cooldown not tracked** — assign_cc never updates last_cast_tick (GPT peer review #3)
- **Re-mez spams every tick** — needs_remez doesn't extend cc_expiry_tick (#4)
- **Rez spams every tick** — death_commands has no cast-in-progress guard (#5)
- **Spawn ID partial match** — CcExpiring filter matches partial IDs (#6)
- **Vendor sell cycle has no travel time** — completes in 3 ticks (#7 medium)

### Deferred (from peer reviews)
- Reflective DLL injection (avoid LoadLibrary detection)
- Seqlock retry on read contention
- Lock-free command queue (replace Mutex on game thread)
- Ring buffer logging instead of file logging from DLL
- String obfuscation in DLL binary

## Testing Checklist

### Quick Test (2 characters)
1. Launch EQ clients manually or via `launch_eq.bat`
2. Log in frostreaver01 + frostreaver02
3. `target\release\dmft.exe --inject`
4. `target\release\dmft.exe` (TUI)
5. Test: `1-5` screen switching, `?` help, `:` command bar
6. Test: `:<pid> /sit` and `:<pid> /stand`
7. Test: `4` map screen — should show zone geometry
8. Test: `p` privacy mode

### 6-Character Test
1. Double-click `launch_eq.bat` on desktop
2. Enter passwords on all 6 clients (01, 02, 03, 04, 06, 07)
3. Create characters on accounts that don't have them yet
4. Press any key when all in-game → auto-inject + TUI
5. Test: `:all /sit` broadcasts to all
6. Test: Group formation via `:` commands

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log`
- Launch: `C:\Users\xmale\Desktop\launch_eq.bat`
- Local IP: 192.168.1.130

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`

## Next Priorities
1. Fix peer review bugs (#3-#7 from GPT review)
2. Live 2-character combat test in a newbie zone
3. Auto-login (UI widget password entry)
4. Create characters on accounts 03, 04, 06, 07
5. 6-character group XP test
6. Navmesh loader for pathfinding
7. Anti-detection hardening
