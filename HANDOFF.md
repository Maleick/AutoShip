# Session Handoff — 2026-03-30 ~08:30 UTC

## Start Here

Read this file + check memories (`MEMORY.md`) for full project context.

## Session Stats (Cumulative)
- ~57,000+ lines across 3 crates
- ~175+ commits (~20 this session)
- 645+ tests passing, 0 failures
- Navigation: **NAVMESH PATHFINDING WORKING** (Detour integration via mqmesh.com)
- Zone graph: **888-zone adjacency graph** readable from EQ memory
- Login chain: credentials → server → char select WORKING, Enter World fix applied (UNTESTED)
- 6 accounts configured, scripts ready

## What's Done (This Session — Overnight 2026-03-30)

### Navigation System (TOP PRIORITY) ✅
- **Navmesh pathfinding** — downloads .navmesh from mqmesh.com, parses binary format (header + zlib + protobuf), loads Detour tiles via C++ FFI shim
- **`--navpath <zone> <x1> <y1> <z1> <x2> <y2> <z2>`** — offline path query (verified: freeportwest 486 tiles, 5-waypoint path)
- **`--nav <PID> <x> <y> <z>`** — mesh-aware navigation (reads zone from shared memory, queries mesh, sends full waypoints)
- **`--navall <x> <y> <z>`** — navigate ALL clients to a point with mesh paths
- **Navigator calls ExecuteCmd(CMD_FORWARD)** — wired but UNTESTED on live client
- **Zone info in shared state** — zone_short_name + zone_long_name from zoneHeader

### Zone-to-Zone Navigation ✅
- **ZoneGuideManagerClient reader** — DLL reads 888-zone graph from EQ memory singleton
- **BFS pathfinding** — `ZoneGraph::find_path(from, to)` with transfer type awareness
- **`--zones <PID>`** — dump full zone graph from live client
- **IPC: QueryZoneGraph** command + Response::ZoneGraph

### Enter World Fix ✅ (UNTESTED)
- Root cause: `find_visible_window_by_sidl_name` used eqmain dShow offset in eqgame context
- Fix: replaced with `rescan_char_list_wnd()` (SidlText scan only)
- Fallback: `send_enter_to_eq()` instead of `/enterworld` slash command

### New CLI Commands ✅
- `--inject-pid <PID>` — inject DLL into specific process
- `--login-pid <PID>` — send login to specific process
- `--status <PID>` — player state + zone info
- `--statusall` — table view of all EQ clients
- `--nav <PID> <x> <y> <z>` — mesh-aware navigation
- `--navall <x> <y> <z>` — navigate all clients
- `--navpath <zone> <x1> <y1> <z1> <x2> <y2> <z2>` — offline path query
- `--zones <PID>` — dump zone graph

### Auto-Accept Dialogs ✅
- Scans for group invite, raid invite, trade, task, resurrect, expedition dialogs
- Auto-clicks accept button via SIDL name matching
- Toggle via `SetAutoAccept` IPC command

### Research Reports ✅
- GAP_ANALYSIS.md — comprehensive MQ2 vs Frostreaver feature comparison
- MQ2NAV_RESEARCH.md — navmesh format, Detour integration
- eqlib deep dive — Enter World, zone routing, movement, struct validation
- AutoAccept patterns, RedGuides/OpenVanilla, goodurden maps, nav-mesh-updater

## MORNING TEST PLAN

### 1. Enter World (press Enter on 6 clients)
- Characters are at char select with new DLL
- Enter World fix should auto-enter after pressing Enter once
- Verify with `--statusall` — should show zone names and real positions

### 2. Test Navigation
```bash
# Check all clients
./target/release/dmft.exe --statusall

# Navigate one client with mesh pathfinding
./target/release/dmft.exe --nav <PID> <x> <y> <z>

# Navigate all clients to a point
./target/release/dmft.exe --navall <x> <y> <z>
```

### 3. Test Zone Graph
```bash
./target/release/dmft.exe --zones <PID>
```

## Build Requirements

```bash
export PATH="/c/Program Files/CMake/bin:/c/Program Files/LLVM/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
export CMAKE_POLICY_VERSION_MINIMUM=3.5
cargo build --release
```

## Key Offsets (NEVER CHANGE)

| Offset | Value | Context |
|--------|-------|---------|
| CEDITBASEWND_INPUT_TEXT | 0x278 | eqmain |
| CXWND_VTABLE_WND_NOTIFICATION | 0x110 | eqmain |
| CXWND_VTABLE_WND_NOTIFICATION | 0x120 | eqgame |
| EXECUTE_CMD | 0x1402235B0 | eqgame |
| ZONE_GUIDE_MANAGER | 0x1403571F0 | eqgame |

## Key References
- MQ2 Login: https://github.com/macroquest/macroquest/tree/master/src/login
- MQ2 Routing: https://github.com/macroquest/macroquest/tree/master/src/routing
- MQ2Nav: https://github.com/brainiac/MQ2Nav
- eqlib: https://github.com/macroquest/eqlib
- mqmesh.com — navmesh downloads + updater.json manifest
