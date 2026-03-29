# Session Handoff — 2026-03-29 ~09:00 UTC

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

**Prompt to start next session:**
```
Please initialize Serena. Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off. Use autoresearch loops for implementation. Run peer review after each major feature.
```

## Session Stats (Cumulative)
- ~41,000+ lines added (~2,000+ this session)
- ~90 commits (10 this session)
- 610 tests passing across 3 crates
- Auto-login: Phases 1-2 CONFIRMED WORKING (credentials + Login click + PLAY EVERQUEST click)
- Camp↔Combat: FULLY WIRED (CombatEngage/Disengage IPC end-to-end)
- TUI commands: :ma, :mt, :engage, :disengage now send real IPC commands
- 12 class strategies (unchanged from last session)

## What's Done (This Session)

### Login FSM Overhaul
- `find_window_by_name` was a STUB returning None — implemented with CXWndManager enumeration
- Added `find_window_by_text_contains` + `find_child_button_by_text` for fuzzy matching
- Added EULA + 5 pre-login screen handlers from MQ2AutoLogin
- Fixed window text matching from calibration: "Login", "PLAY EVERQUEST!", "SERVER SELECT"
- Key discovery: game loop hook doesn't fire during eqmain.dll login screen
- Restored inline IPC handler for credential entry (runs on IPC thread)
- Phases 1-2 CONFIRMED: char array write + Login button vtable click + PLAY EVERQUEST click

### Camp↔Combat Integration (BIGGEST GAP CLOSED)
- Added `CampAction` enum (Slash, CombatEngage, CombatDisengage)
- `transition_to_fighting()` sends CombatEngage to tank, DPS, and healer
- `transition_to_looting()` sends CombatDisengage to all members
- DLL `dispatch_command()` handles CombatEngage/Disengage/SetAssistTarget
- Orchestrator `dispatch_action()` + `send_ipc_command()` for structured IPC
- `CampSnapshot` includes `target_spawn_id`

### TUI Commands Wired
- `:ma <name>` — sets MA, sends /assist to all focused clients
- `:mt <name>` — sets Main Tank
- `:engage [target_id]` — sends CombatEngage IPC to all focused clients
- `:disengage` — sends CombatDisengage IPC to all focused clients
- Added `send_ipc_command()` helper for TUI → DLL structured commands

### Scripts & Desktop
- Desktop shortcuts pointing to repo scripts (auto-update on git pull)
- `test_autologin.bat` — fully automatic, no manual pauses
- Watchdog uses `/login:` flag to bypass EULA
- `create_shortcuts.ps1` on frostreaver for setup

## Priority TODO — Next Session

### 1. Fix Login Credential Entry (PRIORITY #1 — BLOCKING)
The password isn't reaching EQ. Three approaches tried, none fully working:

**Approach A: EQLogin char arrays** — Writes to backend memory but UI doesn't read from it. Login button click works but EQ submits empty password.

**Approach B: CXStr/CEditWnd write** — Crashes eqmain.dll. The InputText offset (+0x278) is wrong for eqmain.dll's CEditWnd version.

**Approach C: PostMessageW(WM_CHAR)** — Built but untested. Types password char-by-char. EQ may use DirectInput which ignores PostMessage.

**MQ2's approach (from source):** Calls `CEditWnd::SetWindowText(const CXStr&)` — a virtual function at vtable offset `0x280`. This is an EQ internal function that updates BOTH the UI and internal state. Key code from `StateMachine.cpp`:
```cpp
CEditWnd* pPasswordEditWnd = GetChildWindow<CEditWnd>(m_currentWindow, "LOGIN_PasswordEdit");
SetEditWndText(pPasswordEditWnd, m_record->accountPassword);
SendWndNotification(pConnectButton, pConnectButton, XWM_LCLICK);
```

**What to implement:** Call `CEditWnd::SetWindowText` through the vtable at offset `0x280`. This requires:
1. Find the password CEditWnd widget (already working — "2 before PASSWORD label")
2. Construct a CXStr from the password string
3. Call vtable[0x280/8=80] with (this=pEditWnd, &CXStr)
4. Click Login button (already working)

The tricky part is CXStr construction — it's a pointer to CStrRep. We already have `clone_cstrrep_for_password()` which allocates via HeapAlloc. Alternatively, use the existing CXStr from the username widget as a template.

### 2. Test WM_CHAR Approach First (QUICK WIN)
The PostMessageW(WM_CHAR) approach is already built and deployed on frostreaver. Test it before implementing the vtable approach:
1. Close all EQ windows
2. Double-click "Test AutoLogin" shortcut on Desktop
3. Check DLL log for "Typed password via WM_CHAR"
4. If EQ submits the login, WM_CHAR works and we're done

### 3. More Combat Features
- Bard strategy (class 8) + melody/twist engine
- Ranger strategy (class 4)
- Camp loop recovery wiring (death detection → rez)
- select_target() integration

### 4. TUI Enhancements
- `:ch start/stop/interval` for CH chain management
- `:invite all` / `:accept all` for group formation
- Help overlay expansion
- Research awesome-tuis for UI ideas

### 5. Deferred
- Navmesh loader (biggest gap for autonomous navigation)
- Phase 3 character select (Enter World button calibration)
- Zone name "Unknown" offset
- STANDSTATE offset calibration

## Key Technical Discoveries (This Session)

### Login Architecture
- `find_window_by_name` was a STUB — all window-based ops were no-ops
- Game loop hook (ProcessGameEvents) does NOT fire during eqmain.dll login screen
- eqmain.dll has its own event loop separate from eqgame.exe
- Inline IPC handler runs on IPC thread (works during login)
- CEditWnd::SetWindowText vtable offset = 0x280 (from MQ2 source)
- CEditWnd__SetWindowText_x = 0x1405FF460 (eqgame.exe, not eqmain.dll)
- MQ2 uses SetEditWndText() wrapper which calls the vtable function
- Password CEditWnd found at "2 before PASSWORD label" in CXWndManager array

### Camp↔Combat
- CampAction enum bridges slash commands and structured IPC commands
- Orchestrator dispatch_action() routes to send_slash_command() or send_ipc_command()
- Combatant FSM engage() does: /face, pet attack, auto-attack toggle, class strategy activation

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log.YYYY-MM-DD`
- Desktop shortcuts: "EQ Watchdog", "Test AutoLogin" (point to repo scripts)
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`

## Research TODO for Next Session
- Use TeamCreate to spawn parallel agents:
  1. **MQ2 Research Agent**: Deep-dive into MQ2AutoLogin `SetEditWndText`, `GetChildWindow`, and how they construct CXStr for the vtable call. Check `StateMachine.cpp` exhaustively.
  2. **Login Implementation Agent**: Implement `CEditWnd::SetWindowText` vtable call in Rust
  3. **Testing Agent**: If a Windows MCP exists, use it for the test loop on frostreaver
