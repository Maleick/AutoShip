# Session Handoff — 2026-03-30 ~02:30 UTC

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

## Session Stats (Cumulative)
- ~54,000+ lines across 3 crates
- ~155+ commits (~15 this session)
- 474 tests passing (2 pre-existing affinity test failures on Windows, need #[cfg(not(windows))])
- 0 clippy errors
- Login chain: **FULLY WORKING** — login → server select → character select → enter world
- 36 account credentials stored in config/accounts.csv (gitignored)
- 6-client Group 1 launch script ready (scripts/launch_group1.bat)

## Unmerged Branches
- `claude/nostalgic-cray` — audit fixes (shared memory DACL, DLL injection improvements). Has merge conflicts with login chain rewrite. Cherry-pick in next session.
- `claude/vibrant-ishizaka` — already merged
- Other `claude/*` branches — empty/stale, locked by worktrees from dispatch sessions

## What's Done (This Session — 2026-03-30)

### Login Chain — FULLY WORKING ✅
Root causes found and fixed:
1. **eqmain vtable WndNotification at 0x110** (not 0x120 like eqgame) — eqmain::CXWnd has different vtable layout than eqgame CXWnd. This was why PLAY EVERQUEST clicks didn't work.
2. **Password empty bug** — `mem::take` moved password before credential write. Fixed ordering.
3. **CXWndManager offsets swapped** — eqgame count at +0x008, array at +0x010 (ArrayClass layout).
4. **Window scan crash** — bad pointer at end of array. Fixed with early exit.
5. **IPC pipe error loop** — missing DisconnectNamedPipe + no backoff → 8.5GB log. Fixed.
6. **PostMessage Enter** — simulate_enter_key used SendInput (foreground only). Fixed to PostMessage.
7. **Game loop Enter key** — sends VK_RETURN every 3s when at character select to click Enter World.

### Proven Working Flow:
1. Launch EQ: `eqgame.exe patchme /login:frostreaver01`
2. Wait 12s for login screen
3. Inject DLL: `dmft.exe --inject`
4. Wait 2s
5. Send login: `dmft.exe --login frostreaver01 <password> "Firiona Vie"`
6. DLL writes credentials via CStrRep + clicks Login via WndNotification(0x110)
7. DLL also types password via WM_CHAR as backup
8. Phase 2: finds PLAY EVERQUEST, vtable clicks it
9. Phase 3: eqmain.dll unloads → character select
10. Game loop sends Enter → enters world (memory jumps to 1GB)

### Research Done ✅
- Cloned macroquest, eqlib, mq-definitions repos to mq2-reference/, mq2-eqlib/, mq2-definitions/
- Deep analysis of MQ2 AutoLogin StateMachine.cpp — complete window name map, state flow, dialog handling
- eqmain::CXWnd vtable layout discovered in LoginFrontend.h (different from eqgame CXWnd.h)
- CListWnd inherits CXWnd (not CSidlScreenWnd) — confirmed
- Navmesh: MQ2Nav uses Recast/Detour with protobuf-wrapped .navmesh files
- /stick: uses ExecuteCmd for movement (keyboard simulation), not CPhysicsInfo writes
- Casting: CastSpell by gem slot, interrupt detection via chat message parsing
- IPC: MQ2 uses TCP (EQBC), our shared memory approach is better for single-machine

### Code Quality ✅
- IPC pipe backoff (exponential 10ms→5s)
- read_cxstr pointer validation (rep_ptr < 0x10000)
- Window scan early exit to prevent crashes
- Tests gated with #[cfg(not(windows))] for null-pointer stub tests

## Key Offsets (NEVER CHANGE WITHOUT LIVE TEST)

| Offset | Value | Context | Notes |
|--------|-------|---------|-------|
| CEDITBASEWND_INPUT_TEXT | 0x278 | eqmain | NOT 0x280 (eqlib says 0x280 but that's wrong for this client) |
| CXWND_WINDOW_TEXT | 0x078 | both | Confirmed |
| CSIDL_SCREEN_WND_SIDL_TEXT | 0x270 | both | CSidlScreenWnd only, not CListWnd |
| CXWND_VTABLE_WND_NOTIFICATION | 0x110 | eqmain | Different from eqgame! |
| CXWND_VTABLE_WND_NOTIFICATION | 0x120 | eqgame | Different from eqmain! |
| CXWndManager COUNT | 0x008 | eqgame | ArrayClass: m_length first |
| CXWndManager ARRAY | 0x010 | eqgame | ArrayClass: m_array second |
| CXWndManager ARRAY | 0x010 | eqmain | Different layout |
| CXWndManager COUNT | 0x018 | eqmain | Different layout |

## IMMEDIATE TODO — Next Session

### 1. Multi-Client Testing
- Test `scripts/launch_and_login.bat` with single client
- Create multi-account batch for 6 clients with staggered launch
- Need additional account credentials in config

### 2. Character Select Improvements
- SelectCharacter by name (currently Enter selects first/default character)
- Use CCharacterListWnd::SelectCharacter(index) + EnterWorld() via game loop
- Stop sending Enter once in-world

### 3. Dialog Handling
- Implement proper "character already logged in" Yes/No dialog detection
- Use eqmain child window names: YESNO_YesButton, YESNO_NoButton
- Need eqmain vtable offset (0x110) for these button clicks

### 4. Continue Audit + Cleanup
- Run simplify on login chain code
- Fix remaining clippy warnings
- Add tests for new widget primitives

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log.YYYY-MM-DD`
- Test: `C:\Users\xmale\Projects\DMFT\scripts\launch_and_login.bat`
- MQ2 ref: `C:\Users\xmale\Projects\DMFT\mq2-reference/`
- eqlib ref: `C:\Users\xmale\Projects\DMFT\mq2-eqlib/`

### Session Notes
- EQ requires Console session with GPU (not RDP)
- Use `tscon` or disconnect RDP to activate console
- Character stuck in-world takes 5-10 min to timeout
- `/login:` flag required to skip EULA
