# Session Handoff — 2026-03-29 ~09:45 UTC

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context. Initialize Serena.

**Prompt to start next session:**
```
Please initialize Serena. Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off. Use autoresearch loops for implementation. Run peer review after each major feature.
```

## Session Stats (Cumulative)
- ~43,000+ lines added (~4,000+ this session)
- ~105 commits (20 this session)
- 610 tests passing across 3 crates
- Auto-login: Phases 1-2 CONFIRMED WORKING (3 successful logins to server select)
- Phase 3 (Enter World): NOT YET WORKING — needs MQ2 EnterWorld() approach
- Camp↔Combat: FULLY WIRED (CombatEngage/Disengage IPC end-to-end)
- TUI commands: :ma, :mt, :engage, :disengage now send real IPC commands

## What's Done (This Session)

### Login Chain — Phases 1-2 CONFIRMED WORKING ✅
- `find_window_by_name` implemented (was a STUB returning None)
- `find_window_by_text_contains` + `find_child_button_by_text` for fuzzy matching
- EULA + 5 pre-login screen handlers from MQ2AutoLogin
- Key discovery: game loop hook doesn't fire during eqmain.dll login screen
- Restored inline IPC handler for credential entry (runs on IPC thread)
- **Confirmed working approach:** Direct CXStr write to InputText (+0x278) with HeapAlloc CStrRep cloning for null password field
- **Confirmed working:** Login button vtable click (WndNotification XWM_LCLICK)
- **Confirmed working:** PLAY EVERQUEST button vtable click
- **NOT working:** SetWindowText vtable at 0x280 — wrong function in eqmain.dll's vtable
- **NOT working:** PostMessageW (WM_CHAR/VK_RETURN) — EQ uses DirectInput
- **NOT working:** EQLogin char array write — UI doesn't read from backend
- Null InputText fix: clones CStrRep from WindowText donor when InputText is null

### Login Chain — Phase 3 (Enter World) NOT YET WORKING
- `/enterworld` slash command — executes but doesn't work at character select
- PostMessageW Enter key — EQ ignores it (DirectInput)
- eqgame.exe CXWndManager button search — "Enter World" text not found in 630 windows
- Window dump shows character CREATE windows but not character SELECT buttons
- **MQ2's approach:** `pCharacterListWnd->EnterWorld()` — direct function call
  - `CCharacterListWnd::EnterWorld` at `0x1400D4B20` (preferred base)
  - `pinstCXWndManager` at `0x140F37B28` (eqgame.exe, NOT eqmain.dll)
  - Need to find `pCharacterListWnd` — MQ2 uses `FindMQ2Window("CharacterListWnd")`

### Camp↔Combat Integration ✅
- `CampAction` enum (Slash, CombatEngage, CombatDisengage)
- `transition_to_fighting()` sends CombatEngage to all group members
- `transition_to_looting()` sends CombatDisengage to all members
- DLL `dispatch_command()` handles CombatEngage/Disengage/SetAssistTarget
- Orchestrator `dispatch_action()` + `send_ipc_command()` for structured IPC
- `CampSnapshot` includes `target_spawn_id`

### TUI Commands ✅
- `:ma <name>` — sets MA, sends /assist to all focused clients
- `:mt <name>` — sets Main Tank
- `:engage [target_id]` — sends CombatEngage IPC
- `:disengage` — sends CombatDisengage IPC
- `send_ipc_command()` helper for structured IPC from TUI

### Scripts & Desktop ✅
- Desktop shortcuts → repo scripts (auto-update on git pull)
- `test_autologin.bat` — fully automatic, 10s initial wait
- Watchdog uses `/login:` flag to bypass EULA

## Priority TODO — Next Session

### 1. Fix Phase 3: Enter World (USE TEAMCREATE FOR PARALLEL WORK)

**Agent 1 — MQ2 Research:**
- Deep-dive `StateMachine.cpp` CharacterSelect state handling
- Find how MQ2 locates `pCharacterListWnd` at runtime
- Check if it uses SIDL window lookup or a global pointer
- Check eqgame.exe offsets for `pinstCCharacterListWnd` or similar
- Research `CCharacterListWnd::EnterWorld()` function signature and calling convention

**Agent 2 — Implementation:**
- Implement `CCharacterListWnd::EnterWorld()` direct function call
- Add offset `CHAR_LIST_ENTER_WORLD` = `0x1400D4B20` (already in offsets.rs)
- Find pCharacterListWnd: either via eqgame.exe global pointer or SIDL window lookup
- Alternative: find "Enter World" button using correct eqgame.exe CXWndManager offsets
  (note: eqmain.dll offsets for CXWNDMGR_WINDOWS_ARRAY/COUNT might differ from eqgame.exe)

**Agent 3 — Test Loop (if possible):**
- SSH → close EQ → rebuild → trigger test_autologin.bat → wait → check DLL log
- Parse log for Phase 3 results
- Iterate until Enter World works

### 2. Document Confirmed Working Patterns
Extract into shared library (user requested):
- `find_window_by_text()` — CXWndManager enumeration (WORKS)
- `write_cxstr_inplace()` — direct InputText CXStr write (WORKS for non-null CXStr)
- `clone_cstrrep_for_password()` — HeapAlloc CStrRep cloning (WORKS)
- `click_button_via_vtable()` — WndNotification XWM_LCLICK (WORKS)
- `set_edit_text_via_vtable()` — SetWindowText vtable 0x280 (DOES NOT WORK in eqmain.dll)

### 3. TUI Fixes (from live testing feedback)
- Zone name shows "Unknown" — offset calibration needed
- Map shows points only — needs rasterized line rendering
- Hex dump viewer empty — base address connection issue
- Group tab not populating with online characters

### 4. More Combat Features
- Bard strategy + melody engine
- Ranger strategy
- Camp loop recovery wiring

### 5. Peer Review Fixes (from this session)
- ScreenMode = 3 before credential entry (MQ2 does this)
- eqgame.exe CXWndManager offsets may differ from eqmain.dll
- Thread safety: login credential writes happen on IPC thread, not game loop

## Key Technical Discoveries

### Confirmed Working (eqmain.dll login screen)
| Method | Status | Notes |
|--------|--------|-------|
| `write_cxstr_inplace` to InputText +0x278 | ✅ WORKS | When CXStr is non-null |
| `clone_cstrrep_for_password` HeapAlloc | ✅ WORKS | Clones from donor CStrRep |
| `click_button_via_vtable` WndNotification | ✅ WORKS | vtable offset 0x110 |
| CXWndManager window enumeration | ✅ WORKS | "2 before label" heuristic |
| `/login:account` command-line flag | ✅ WORKS | Bypasses EULA |

### Does NOT Work
| Method | Status | Notes |
|--------|--------|-------|
| SetWindowText vtable 0x280 | ❌ WRONG FUNC | eqmain.dll vtable differs from eqgame.exe |
| PostMessageW WM_CHAR | ❌ IGNORED | EQ uses DirectInput |
| PostMessageW VK_RETURN | ❌ IGNORED | Same — DirectInput |
| EQLogin char array write | ❌ NO UI REFRESH | UI reads from CEditWnd, not backend |
| `/enterworld` slash cmd | ❌ NO EFFECT | Not recognized at character select |

### Architecture Notes
- eqmain.dll has its OWN event loop — game loop hook doesn't fire during login
- eqmain.dll and eqgame.exe have DIFFERENT CXWnd vtable layouts
- eqmain.dll CXWndManager offsets (WINDOWS_ARRAY, WINDOWS_COUNT) confirmed working
- eqgame.exe pinstCXWndManager at 0x140F37B28 resolves but may use different array offsets
- CStrRep must be allocated on Windows process heap (HeapAlloc) for EQ to manage
- `/login:` flag inconsistently populates InputText vs WindowText between launches

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log.YYYY-MM-DD`
- Desktop shortcuts: "EQ Watchdog", "Test AutoLogin" (point to repo scripts)
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`
