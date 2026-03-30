# Session Handoff — 2026-03-30 ~07:30 UTC

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

## Session Stats (Cumulative)
- ~55,000+ lines across 3 crates
- ~165+ commits (~10 this session)
- 631 tests passing (474 dmft + 46 dmft-common + 111 dmft-dll), 0 failures
- 0 clippy errors
- Login chain: credentials → server → char select WORKING
- Enter world: BROKEN (EnterWorld() never triggers — #1 priority fix)
- Navigation: framework working but movement broken (writes heading but ExecuteCmd added, untested)
- Zone info: reading from shared memory WORKING ("West Freeport (freeportwest)" confirmed)
- 6 accounts launched, DLL injected, at character select (need manual Enter to enter world)

## What's Done (This Session — 2026-03-30)

### Multi-Client Infrastructure ✅
- `--inject-pid <PID>` — inject DLL into specific process
- `--login-pid <PID>` — send login to specific process
- `--status <PID>` — read shared memory (player, position, zone, nav status)
- `--nav <PID> <x> <y> <z>` — send NavigateTo command
- CSPRNG session token auth (written before injection, DLL reads at init)
- Double-injection guard (ALREADY_INITIALIZED atomic bool)
- "Already logged in" YESNO dialog handler in Phase 3 polling

### Zone Info ✅
- `zone_short_name` and `zone_long_name` added to GameState shared memory
- DLL reads from zoneHeader struct every 30 ticks
- Confirmed working: "West Freeport (freeportwest)"

### Navigation Framework (Partially Working)
- Navigator lazy-inits on first in-world tick
- ExecuteCmd(CMD_FORWARD) wired into MovementController (NEW — untested with live client)
- Nav sends NavigateTo, Navigator receives waypoints, stuck detection works
- BUT: characters don't actually walk yet (needs testing with new DLL)

### Test/Audit Cleanup ✅
- All null-pointer stub tests gated with `#[cfg(not(windows))]`
- Cherry-picked audit fixes from claude/nostalgic-cray
- 10 local + 4 remote stale branches deleted
- All pushed to master

### Research Completed ✅
1. **Navmesh**: mqmesh.com serves `.navmesh` files (protobuf + Detour tiles). `divert` Rust crate for pathfinding.
2. **Zone routing**: ZoneGuideManagerClient at 0x1403571F0 has 888-zone adjacency graph in EQ memory. BFS pathfinding.
3. **eqlib deep dive**: Enter World = `CCharacterListWnd::EnterWorld()`, movement = `ExecuteCmd(CMD_FORWARD)`, doors = `EQSwitch::UseSwitch()`.
4. **All offsets verified correct** against MQ2 eqlib (March 2026 patch 20260310).

## IMMEDIATE TODO — Next Session

### 1. Fix Enter World Automation (CRITICAL)
The login chain reaches character select but EnterWorld() never fires. The code exists in game_loop.rs (stages 1-3) but `ENTER_WORLD_STAGE` is never set to 1.

**Root cause**: The login FSM's `do_select_character_via_game_loop()` should queue the enter world, but something in the chain from `tick_selecting_character()` → `do_select_character_via_game_loop()` → `queue_enter_world()` isn't firing.

**Fix approach**:
- Trace the login FSM from Phase 3 (eqmain unloaded) through character select
- The FSM transitions to `SelectingCharacter` but may not find CCharacterListWnd
- Alternative: call EnterWorld() directly when at char select (simpler than the stage system)
- Reference: MQ2 AutoLogin just calls `CCharacterListWnd::EnterWorld()` directly

### 2. Test Navigation Movement
The new DLL has `ExecuteCmd(CMD_FORWARD)` wired in but hasn't been tested on a live in-world client. Need to:
- Get a character in-world with the latest DLL
- Send `--nav <PID> <x> <y> <z>` to a reachable nearby point
- Verify the character actually walks

### 3. Navmesh Integration
- Add `divert` crate dependency
- Download zone meshes from mqmesh.com
- Parse .navmesh protobuf → Detour tiles → NavMeshQuery
- Replace straight-line nav with mesh-aware pathfinding

### 4. Zone-to-Zone Navigation
- Read ZoneGuideManagerClient from memory (zone graph)
- Implement BFS pathfinder
- Add EQSwitch::UseSwitch binding for doors/books
- Record zone line coordinates for key routes

## Key Offsets (NEVER CHANGE WITHOUT LIVE TEST)

| Offset | Value | Context | Notes |
|--------|-------|---------|-------|
| CEDITBASEWND_INPUT_TEXT | 0x278 | eqmain | NOT 0x280 |
| CXWND_VTABLE_WND_NOTIFICATION | 0x110 | eqmain | Different from eqgame! |
| CXWND_VTABLE_WND_NOTIFICATION | 0x120 | eqgame | Different from eqmain! |
| EXECUTE_CMD | 0x1402235B0 | eqgame | CMD_FORWARD=2, CMD_BACK=3 |
| ENTER_WORLD | 0x14027E1F0 | eqgame | CCharacterListWnd::EnterWorld |
| ZoneGuideManagerClient | 0x1403571F0 | eqgame | 888-zone graph |
| EQSwitch::UseSwitch | 0x14026B060 | eqgame | Door/book interaction |

## Key File Paths

### MQ2 References
- Login automation: https://github.com/macroquest/macroquest/tree/master/src/login
- Zone routing: https://github.com/macroquest/macroquest/tree/master/src/routing
- HUD overlay: https://github.com/macroquest/macroquest/blob/master/src/plugins/hud/MQ2HUD.cpp
- eqlib: https://github.com/macroquest/eqlib
- Local clones: mq2-reference/, mq2-eqlib/, mq2-definitions/

### Frostreaver (Windows)
- DMFT: `C:\Users\xmale\Projects\DMFT`
- EQ: `C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest`
- DLL logs: `C:\Users\xmale\AppData\Local\Temp\dmft\dmft-dll.log.YYYY-MM-DD`

### Session Notes
- EQ requires Console session with GPU (not RDP)
- Characters stuck in-world take 5-10 min to timeout
- `/login:` flag required to skip EULA
- PowerShell `$pid` is reserved — use `$procId` instead
- Git Bash converts `/slash` paths — use `MSYS_NO_PATHCONV=1`
