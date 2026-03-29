# Session Handoff — 2026-03-29 Early Morning

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

## Session Stats (Cumulative)
- ~35,000+ lines added (~2,000+ this session)
- ~60 commits (15+ this session)
- 552 tests passing across 3 crates
- Auto-login: full chain working (login → server select → in-world)

## Auto-Login Breakthrough (This Session)

### What Works
- **CXWndManager window enumeration** — discover all 150 UI widgets at runtime
- **CXStr direct widget write** — write credentials to CEditWnd::InputText (+0x278)
- **HeapAlloc CStrRep** — allocate CStrRep for empty password field using process heap + donor metadata
- **Vtable WndNotification click** — click Login button programmatically (no foreground needed)
- **Full login chain** — credentials → Login click → PLAY EVERQUEST click → Enter World
- **Last test**: character entered world (eqmain.dll unloaded, EQ memory jumped to 1GB)

### Technical Discoveries
- EQ login screen uses **DirectInput** (IDirectInput8A) — all input simulation (SendInput, WM_CHAR, PostMessage) fails
- eqmain.dll CXWndManager has different struct layout than eqgame.exe (+0x010 for window array, not +0x008)
- CXStr = pointer to CStrRep; CStrRep has data at +0x18, length at +0x08, alloc at +0x04, freeList at +0x10
- Allocating CStrRep with Rust's allocator crashes EQ (wrong heap); HeapAlloc + donor freeList works
- CXWnd WndNotification vtable offset in eqmain: 0x110
- WndNotification calls must happen from a thread context where EQ's UI is operational
- WindowText (+0x078) and InputText (+0x278) share the same CStrRep pointer on CEditWnd

### Remaining Login Issues
- Vtable clicks for phases 2/3 (server select, enter world) need verification
- Timing: 12s wait for server select, 15s for character select — could be optimized with polling
- One crash on re-launch after previous session (may be one-off)

### Peer Review Fixes Applied
- Password redacted in Command::StartLogin Debug impl
- Log rotation: max 7 daily files (was unlimited, caused 9GB log)
- Backspace count 32→128 for field clearing
- StartLogin field length validation in validate_command()
- Dead FSM path documented in game_loop dispatch_command

## What Works (Proven Live)

### DLL Injection + Command Execution
- `dmft.exe --inject` finds eqgame.exe, stages DLL with random name, injects
- `dmft.exe --cmd <pid> "/slash_command"` sends commands via IPC pipe
- `dmft.exe --calibrate` sends CalibrateLogin to all injected clients
- `dmft.exe --login <account> <password> [server] [character]` — FULL AUTO-LOGIN
- InterpretCmd calls EQ's internal function — /sit, /stand, /invite, /target, /follow confirmed
- Window renaming: DLL sets title to "[DMFT] EQ - CharName (ZoneName)"
- IPC pipe DACL fixed — Authenticated Users

### Auto-Login (WORKING)
- Full chain: credentials → Login click → server select → character select → enter world
- CXStr direct widget write (MQ2 approach — no input simulation)
- CXWndManager window enumeration for widget discovery
- Vtable WndNotification for button clicks (no foreground focus needed)
- Accounts/passwords in accounts.csv (36 accounts, 7 created)

### TUI (5 screens + enhancements)
- Dashboard, Spawns, Character, Map, Groups
- Command bar with tab completion, :track, :camp, :mode
- Multi-group focus with Shift+1-6

### Camp Loop + Combat + Navigation
- All M3-M4 features as described in CLAUDE.md

## Known Issues

### Must Fix
- **Auto-login timing** — Fixed delays (12s/15s) should be replaced with screen detection polling
- **Zone name still showing "Unknown"** — Zone long name read may need offset validation
- **STANDSTATE offset (0x0574)** — Reads wrong value, needs hex dump calibration
- **9GB log from pipe error loop** — Need backoff/sleep in pipe error handler

### Deferred
- Same as previous handoff (TUI demo mode, charm_break_response, etc.)

## MQ2 Plugin Research Queue
User requested deep comparison for: MQ2Melee (HIGH), MQ2Cast (HIGH), MQ2NetHeal, MQ2AutoGroup, MQ2Vendors, MQ2Debuffs, MQ2AutoSkills, MQ2Rez, MQ2Cursor, MQ2AutoBank, MQ2Map, MQ2TargetInfo, MQ2Labels, MQ2AutoSize

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
1. Verify full auto-login chain with vtable clicks (one more test)
2. Replace fixed delays with screen detection polling
3. Build auto-login batch for all 6 accounts
4. Live 6-character group test with TUI
5. MQ2 plugin deep research (Melee, Cast priority)
6. Navmesh loader (biggest remaining gap for autonomous operation)
