# TUI Map and Navmesh Audit

## Summary

The TUI is in a better operator state than it was at the start of the redesign: the Characters screen is now roster-first, direct character targeting is supported in the command bar, the Tactical screen has a usable map-first layout, and live navigation waypoints are rendered on top of the rasterized EQ map.

The main remaining gap is that the interactive TUI still does not use or render MQ2Nav meshes directly. The backend navmesh loader and path query pipeline already exist and work in CLI flows, but the TUI currently only renders Brewall map geometry plus active waypoint overlays.

## Verified current state

- Character targeting:
  - `Name /cmd` now targets a connected client by character name.
  - `Name` alone focuses that client in the Characters screen.
  - Group targeting still works through `G1`-`G6` and `G1 /cmd`.
- Tactical map rendering:
  - Loads Brewall map lines and labels from `config/maps`.
  - Draws live spawns, named markers, corpse markers, player marker, FOV cone, and active navigation waypoint paths.
  - Uses a player-centered local view for large zones when appropriate.
- Navmesh backend:
  - MQ2Nav `.navmesh` download, parse, Detour load, and path query logic already exist.
  - Per-zone meshes are cached under `data/meshes`.

## Findings

### 1. TUI navigation does not currently use the working navmesh path query backend

The CLI navigation flows already compute `Command::NavigateTo { waypoints }` using `nav::mesh::load_zone(...)` and `nav::mesh::find_path(...)`. The TUI `nav <zone>` path does not do that yet; it currently sends a slash command string (`/nav to <destination>`) to focused clients.

Impact:

- Operator-facing navigation from the TUI is not guaranteed to benefit from the existing Detour path query path.
- The mesh loader can be fully functional while the TUI still behaves like a thin command shell.

### 2. Tactical map renders EQ map geometry, not navmesh geometry

The current renderer draws:

- Brewall line segments and labels from `zone_map`
- spawn glyphs
- named/tracked markers
- live waypoint overlays

It does not draw navmesh polygons, tile edges, off-mesh links, blocked areas, or nearest-poly/path corridor information.

Impact:

- The map shows where the world art says you can go, not where the Detour mesh says the navigator can actually path.
- Debugging mesh failures from the TUI is still largely guesswork.

### 3. The TUI knows about cached meshes for autocomplete, but not for operator visibility

The app can enumerate cached `.navmesh` files for `nav` autocomplete, but the Tactical and Navigation screens do not currently expose:

- whether the current zone has a cached mesh
- whether mesh download/load succeeded
- mesh tile counts / load health
- whether the current active path was navmesh-derived or a straight-line fallback

Impact:

- Operators cannot tell whether a zone is mesh-backed without dropping to CLI/log inspection.

## MQ mesh source

- Site: [mqmesh.com](https://mqmesh.com/)
- Per-zone mesh pattern: `https://mqmesh.com/resources/meshes/{zone}.navmesh`

Example:

- `https://mqmesh.com/resources/meshes/permafrost.navmesh`

## Recommended implementation order

### Phase 1: make TUI navigation use the existing navmesh backend

1. Add a shared navigation service/helper that:
   - resolves the selected client's current zone and position
   - loads or downloads the zone mesh
   - runs `find_path(...)`
   - returns `Vec<Waypoint>` plus metadata about mesh availability and fallback mode
2. Update the TUI `nav <zone|camp>` command path to send `Command::NavigateTo { waypoints }` when mesh-backed pathing is available.
3. Preserve the slash-command fallback only when mesh query or shared-state acquisition fails.

### Phase 2: expose mesh health in the Tactical and Navigation screens

1. Add per-zone mesh status to app state:
   - mesh present / absent
   - source: cache vs download
   - last load result
   - optional tile count / zone short name
2. Surface a compact mesh badge in the map header:
   - `Mesh: loaded`
   - `Mesh: cached`
   - `Mesh: missing`
   - `Mesh: fallback`
3. Show active path provenance in Navigation:
   - `Path: navmesh`
   - `Path: straight-line fallback`

### Phase 3: add mesh visualization to the Tactical map

1. Build a projection layer that converts Detour/navmesh geometry into the same 2D map coordinate space used by the TUI.
2. Add operator toggles:
   - EQ map only
   - navmesh only
   - combined overlay
3. Render at least:
   - walkable polygon edges
   - off-mesh links / jumps / doors
   - active corridor/path result
   - nearest-poly start and end markers

### Phase 4: operator controls for large-zone map work

1. Add manual local/global viewport toggle instead of auto-only local view.
2. Add pan/zoom controls for Tactical maximize mode.
3. Add a mesh-overlay legend so the operator can distinguish map art from pathable mesh geometry.

## Recommended next coding slice

If the next pass is implementation rather than more cleanup, the highest-leverage slice is:

1. Reuse the existing `nav::mesh` loader/query path from the CLI.
2. Wire TUI `nav` commands to emit `NavigateTo` with real waypoints.
3. Add a simple mesh status badge to Tactical and Navigation.

That gives immediate operator value before the harder mesh-geometry rendering work starts.
