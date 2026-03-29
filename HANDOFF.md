# Session Handoff — 2026-03-29 Late Night

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

**Prompt to start next session:**
```
Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off. Priority #1: Test WM_CHAR auto-login — the build is deployed on frostreaver, just needs one more test cycle (close EQ, run test_autologin.bat, send --login command).
```

## Session Stats (Cumulative)
- ~33,000+ lines added (8,000+ this session)
- ~45 commits (15+ this session)
- 552 tests passing across 3 crates (was 479)
- 8 agent team sprints this session
- 4 peer reviews (login, camp, TUI, infra)
- Auto-login pointer chain validated on live EQ

## What Works (Proven Live)

### DLL Injection + Command Execution
- `dmft.exe --inject` finds eqgame.exe, stages DLL with random name, injects
- `dmft.exe --cmd <pid> "/slash_command"` sends commands via IPC pipe
- `dmft.exe --calibrate` sends CalibrateLogin to all injected clients
- `dmft.exe --login <account> <password> [server] [character]` sends StartLogin
- InterpretCmd calls EQ's internal function — /sit, /stand, /invite, /target, /follow confirmed
- Window renaming: DLL sets title to "[DMFT] EQ - CharName (ZoneName)"
- Game state publishing: DLL reads HP/mana/target/spawns every tick to shared memory
- IPC pipe DACL fixed — Authenticated Users (was CREATOR_OWNER, blocked orchestrator)
- IPC commands handled on listener thread (works at login screen before game loop runs)

### Auto-Login (calibrated, WM_CHAR approach deployed but untested)
- All EQLogin pointer chain validated on live March 10 EQ build
- eqmain.dll discovery working (GetModuleHandleW)
- LoginClient, EQLogin, HWND, LoginServerAPI, CSidlManager all resolve correctly
- Direct memory write to EQLogin works but doesn't update UI widgets
- WM_CHAR typing approach deployed: sends keystrokes char-by-char via PostMessageW
- Accounts/passwords in accounts.csv (36 accounts, 7 created)
- **NEEDS TESTING**: Close EQ, run test_autologin.bat, then `dmft.exe --login frostreaver01 <password>`

### TUI (5 screens + enhancements)
- Dashboard (1): character grid, session stats (XP/hr, plat/hr)
- Spawns (2): full list with search (/), filter cycling (f), scroll
- Character (3): detail view with pixel art class emblem sprites
- Map (4): Brewall zone geometry, spawn overlay, named mob tracker with respawn timers
- Groups (5): 2x3 grid with per-group zone/camp info, color-coded status
- Command bar (:): Multi-level tab completion (camp add/list/remove, track, mode, G1-G6)
- :track/:untrack — spawn tracking with Up/Down/Unknown status
- :camp add/list/remove/start/stop/status — camp management
- :mode camp/hunt — operating mode switching
- Multi-group focus: Shift+1-6 to focus groups, Shift+0 for aggregate
- :G1-G6 command prefix for group-targeted commands
- Help overlay (?), Privacy mode (p)
- 45 zone-to-filename mappings for Brewall maps

### Camp Loop
- 5-phase state machine: Idle → Pull → Fight → Loot → Med
- Loot automation: LootCycle FSM (target → approach → open → loot → close → next corpse)
- Vendor interaction: VendorStep sub-FSM with /notify commands
- Camp progression: 11 camps (levels 1-55), auto-advance on outlevel
- Named mob database: 8 zones with respawn tracking, priority pull override
- Hunt mode: HuntLoop FSM with role-based soft-follow (melee 20u, casters 70u, healers 50u)
- CC system: charm/mez tracking, Tash→Malo debuff chain
- Death recovery, buff maintenance, per-character personality profiles
- All 5 peer review bugs fixed (#3-#7)

### Security + Anti-Detection
- CSPRNG random session tokens
- Randomized IPC pipe/shared memory names
- Pipe DACL (Authenticated Users + session token handshake)
- Password zeroization (credentials borrowed, not cloned)
- GM flag detection, render strobing, command jitter

### Infrastructure
- 800MB working set limits (SetProcessWorkingSetSize, configurable)
- CPU affinity management (-1 = Windows auto-distribute)
- Optimized eqclient_multibox.ini template
- CharClass offset fixed (0x0420 direct field)
- Zone long name in window titles

## Known Issues

### Must Fix
- **Auto-login WM_CHAR untested** — Build deployed, needs one test cycle
- **Zone name still showing "Unknown"** — Zone long name read may need offset validation
- **STANDSTATE offset (0x0574)** — Reads wrong value, needs hex dump calibration
- **StickFigures=1 not working** — INI setting has no effect in-game
- **Working set limit is soft** — SetProcessWorkingSetSize, not Ex version (advisory only)

### Deferred (from peer reviews)
- TUI: Demo mode groups empty (demo names lack account numbers)
- TUI: Group focus doesn't sync selected_client
- TUI: :mode missing from help overlay
- TUI: :G1 <Tab> with empty rest shows nothing
- Camp: charm_break_response takes immutable members (can't update cooldown)
- Camp: LootConfig.target_delay is dead config
- Camp: Empty members causes stuck Looting state
- Login: Error dialogs all classified as WrongPassword
- Session token is PID-derived (deterministic, needs CSPRNG)

## Testing Checklist

### Auto-Login Test (PRIORITY #1)
1. Close all EQ clients
2. Double-click `test_autologin.bat` on desktop
3. Wait for password screen, press any key
4. From Command Prompt: `cd C:\Users\xmale\Projects\DMFT`
5. `target\release\dmft.exe --login frostreaver01 dr698iDBBa1IpTS`
6. Watch EQ window — should see username/password being typed, then login submit

### Quick Test (6 characters)
1. `launch_eq.bat` on desktop
2. Log in all 6 manually (until auto-login works)
3. Press any key → auto-inject + TUI
4. Test: screens 1-5, tab completion, :track, :camp, :mode, Shift+1-6

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log.YYYY-MM-DD`
- Launch: `C:\Users\xmale\Desktop\launch_eq.bat`
- Calibrate: `C:\Users\xmale\Desktop\test_autologin.bat`
- Local IP: 192.168.1.130

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`

## Next Priorities
1. Test WM_CHAR auto-login (one more cycle)
2. Build auto-login batch for all 6 accounts
3. Live 6-character group test with TUI
4. Navmesh loader (biggest remaining gap for autonomous operation)
5. Cross-zone travel
6. Anti-detection hardening
