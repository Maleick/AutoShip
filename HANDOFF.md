# Session Handoff — 2026-03-29 ~18:00 UTC

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context. Initialize Serena.

**Prompt to start next session:**
```
Please initialize Serena. Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off. Use autoresearch loops for implementation. Run peer review after each major feature.
```

## Session Stats (Cumulative)
- ~47,000+ lines added
- ~106 commits
- 621 tests passing across 3 crates (447 dmft + 46 dmft-common + 128 dmft-dll)
- Auto-login: Phases 1-3 ALL CONFIRMED WORKING
  - Phase 1: Credential entry via CXStr write
  - Phase 2: Server select + PLAY EVERQUEST button click
  - Phase 3: Enter World via eqgame.exe SidlText lookup + direct EnterWorld()
- Camp↔Combat: FULLY WIRED (CombatEngage/Disengage IPC end-to-end)
- TUI commands: :ma, :mt, :engage, :disengage now send real IPC commands

## What's Done (Latest Session)

### Phase 3: Enter World — FIXED ✅ (commit 3088864)
- Uses eqgame.exe offsets (not eqmain.dll) for CXWndManager
- SidlText-based window lookup (finds "CharacterListWnd" via SIDL XML name)
- Direct `CCharacterListWnd::EnterWorld()` function call at offset `0x1400D4B20`
- Fallback: button click via vtable if EnterWorld() pointer is null

### Widget Pattern Extraction ✅
- `dmft-dll/src/eq/widgets.rs` — shared primitives for all EQ UI interaction
  - `find_window_by_name()`, `find_window_by_text_contains()`, `find_child_button_by_text()`
  - `read_cxstr()`, `write_cxstr_inplace()`, `clone_cstrrep()`
  - `click_button_via_vtable()`
- `dmft-dll/src/login/widgets.rs` — login-specific widget helpers (SIDL names, pre-login prompts)
- `dmft-dll/src/login/eqmain.rs` — eqmain.dll discovery and CSidlManager resolution

### Camp↔Combat Integration ✅
- `CampAction` enum (Slash, CombatEngage, CombatDisengage)
- DLL `dispatch_command()` handles CombatEngage/Disengage/SetAssistTarget
- Orchestrator `dispatch_action()` + `send_ipc_command()` for structured IPC

### TUI Commands ✅
- `:ma <name>` — sets MA, sends /assist to all focused clients
- `:mt <name>` — sets Main Tank
- `:engage [target_id]` — sends CombatEngage IPC
- `:disengage` — sends CombatDisengage IPC

## Priority TODO — Next Session

### 1. TUI Fixes (from live testing feedback)
- Zone name shows "Unknown" — offset calibration needed
- Map shows points only — needs rasterized line rendering
- Hex dump viewer empty — base address connection issue
- Group tab not populating with online characters
- Dynamic group window + character/server loading

### 2. More Combat Features
- Bard strategy + melody engine
- Ranger strategy
- Camp loop recovery wiring

### 3. Navigation Tab
- TUI navigation screen
- Hunt/Camp mode tabs + commands

### 4. Anti-Detection Hardening
- Reflective injection (Phase 6)
- String obfuscation

### 5. Peer Review Fixes (from previous session)
- ScreenMode = 3 before credential entry (MQ2 does this)
- Thread safety: login credential writes happen on IPC thread, not game loop

## Key Technical Discoveries

### Confirmed Working (all login phases)
| Method | Status | Notes |
|--------|--------|-------|
| `write_cxstr_inplace` to InputText +0x278 | ✅ WORKS | When CXStr is non-null |
| `clone_cstrrep` HeapAlloc | ✅ WORKS | Clones from donor CStrRep |
| `click_button_via_vtable` WndNotification | ✅ WORKS | vtable offset 0x110 |
| CXWndManager window enumeration | ✅ WORKS | Both eqmain.dll and eqgame.exe |
| SidlText-based window lookup | ✅ WORKS | For eqgame.exe windows (Phase 3) |
| `/login:account` command-line flag | ✅ WORKS | Bypasses EULA |
| Direct `EnterWorld()` function call | ✅ WORKS | Phase 3 character select → game |

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
- eqgame.exe pinstCXWndManager at 0x140F37B28 — uses SidlText for window identification
- CStrRep must be allocated on Windows process heap (HeapAlloc) for EQ to manage

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log.YYYY-MM-DD`
- Desktop shortcuts: "EQ Watchdog", "Test AutoLogin" (point to repo scripts)
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`
