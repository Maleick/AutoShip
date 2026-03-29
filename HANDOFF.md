# Session Handoff — 2026-03-29 ~10:30 UTC

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context. Initialize Serena.

**Prompt to start next session:**
```
Please initialize Serena. Read HANDOFF.md and check memories for full context. Use TeamCreate (not background sub-agents) for any parallel work — the user wants visible agent teams in tmux splits. Continue from where we left off. Use autoresearch loops for implementation. Run peer review after each major feature.
```

## Session Stats (Cumulative)
- ~49,000+ lines added (~2,000+ this session)
- ~118 commits (12 this session)
- 633 tests passing across 3 crates (453 dmft + 46 dmft-common + 134 dmft-dll)
- Auto-login: Phases 1-3 IMPLEMENTED (awaiting live test)
  - Phase 1: Credential entry via CXStr write ✅ confirmed
  - Phase 2: Server select + PLAY EVERQUEST button click ✅ confirmed
  - Phase 3: Enter World via eqgame.exe SidlText lookup + direct EnterWorld() (NEEDS LIVE TEST)
- All 16 EQ classes have combat strategies
- Camp loop FSM (camp→pull→fight→loot→return cycle)
- Knowledge system with hypothesis→rule promotion

## What's Done (This Session)

### Phase 3 Enter World — FULLY IMPLEMENTED ✅
- **Root cause fixed**: Was using eqmain.dll CXWndManager offsets (+0x010/+0x018) for eqgame.exe which has (+0x008/+0x010)
- **SidlText lookup**: Find CCharacterListWnd by CSidlScreenWnd::SidlText == "CharacterListWnd" at +0x270
- **Thread safety**: EnterWorld() queued to game loop thread via PENDING_ENTER_WORLD atomics
- **SelectCharacter(0)**: 3-stage sequence — SelectCharacter → wait 90 ticks → EnterWorld
- **Condition polling**: Replaced hard sleeps with polling loops (500ms intervals, 30-60s timeouts)

### Combat — All 16 Classes ✅
- Bard melody twist engine (song rotation, twist timing)
- Ranger (ranged/melee hybrid, stance switching by distance)
- Beastlord (pet class + melee DPS)
- Berserker (pure melee DPS)
- Fixed class ID mapping in build_strategy factory (was wrong for Paladin, Ranger, SK, Wizard)

### Camp Loop FSM ✅
- States: Idle, AtCamp, Pulling, Fighting, Looting, Returning, Recovery
- Wipe recovery with 3-wipe auto-stop
- State timeouts for stuck detection
- Pause/Resume support
- 6 tests

### Widget Pattern Extraction ✅
- `dmft-dll/src/eq/widgets.rs` — shared module with:
  - `read_cxstr()` — CXStr reading from any address
  - `click_button_via_vtable()` — WndNotification(XWM_LCLICK)
  - `set_edit_text_via_vtable()` — SetWindowText via vtable
  - `write_cxstr_inplace()` — direct CXStr content write

### TUI Improvements ✅
- Dynamic group window (auto-sizes to connected clients)
- Navigation tab (camp/zone commands)
- Hunt/Camp mode tabs with command input
- Zone name resolution improvements
- Group member population
- Hex dump connection

### Documentation & Infrastructure ✅
- Knowledge system: EQ internals + login automation domains
- Codebase audit: 2 critical, 8 high, 15 medium, 12 low findings
- Updated README, HANDOFF, CLAUDE.md
- Anthropic long-running app patterns documented

## Priority TODO — Next Session

### 1. LIVE TEST Phase 3 Enter World
- Build is on frostreaver, ready to test
- Run test_autologin → verify Phase 3 enters world
- Check DLL log for: "Phase 3: CXWndManager resolved", "Found CCharacterListWnd", "EnterWorld() called"
- If SidlText lookup fails, check log for window dump and calibrate

### 2. Fix Audit Critical Issues
- C1: Add DACL to shared memory (restrict to current user SID)
- C2: Use PAGE_READONLY for shared memory reader
- H1: Zeroize password in IPC handler

### 3. Windows MCP Setup
- Install windows-mcp on frostreaver for automated testing loop
- Enable screenshot capture for login UI verification
- Automate build→test→check-log cycle

### 4. Character Name Matching for SelectCharacter
- Currently selects first character (index 0)
- Need: walk Character_List CListWnd items, match by name
- Requires CListWnd item reading (GetItemText equivalent)

### 5. More Combat Features
- Wire CampLoop into CombatCoordinator
- Puller FSM integration with CampLoop
- Loot automation (auto-loot corpses)

### 6. MQ2 Feature Research (Autoresearch Loop)
- Navmesh integration (MQ2Nav format, pathfinding)
- Stick/follow (MQ2MoveUtils patterns)
- Robust casting framework (MQ2Cast patterns)
- NetBots/NetHeal for cross-client state sharing

## Key File Paths

### frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log.YYYY-MM-DD`
- SSH: `sshpass -p '1118' ssh maleick@frostreaver`

### Mac (dev)
- DMFT: `/Users/maleick/Projects/DMFT`
