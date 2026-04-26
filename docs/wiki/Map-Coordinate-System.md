# EverQuest Map Coordinate System and Zone Boundaries

This document provides a comprehensive reference for EverQuest coordinate conventions, zone boundary definitions, and map file formats used in TextQuest and related tools.

## Coordinate System Basics

### Axes and Direction Conventions

EverQuest uses a **right-handed coordinate system** with three orthogonal axes:

| Axis | Direction | Range | Notes |
|------|-----------|-------|-------|
| **Y** | North (+) / South (-) | Typically ±10,000+ | Primary horizontal axis in EQ |
| **X** | East (+) / West (-) | Typically ±10,000+ | Secondary horizontal axis in EQ |
| **Z** | Up (+) / Down (-) | Elevation/altitude | Positive = higher altitude |

**Important:** The Y-X axis ordering is unintuitive by modern convention, but this is consistent with EQ's internal packet structure and `/loc` command output.

### Coordinate Units

- **Scale:** EQ coordinates are in arbitrary game units (1 unit ≈ 0.01 to 0.05 meters, exact scaling not documented)
- **Precision:** Floating-point (f32 / f64 depending on context)
- **Valid Range:** Coordinates beyond ±10,000 are generally considered out-of-bounds for safe landing positions (see `SAFE_COORD_MAX` in code)
- **Epsilon:** Position changes < 0.1 units are treated as noise (see `POSITION_EPSILON`)

### Heading / Direction

Heading is stored as a `u16` (0-512 range, wrapping):

| Heading | Direction | Numeric Value |
|---------|-----------|---------------|
| North | 0° | 0 |
| West | 90° | 128 |
| South | 180° | 256 |
| East | 270° | 384 |
| Full Circle | 360° | 512 (wraps to 0) |

Heading increases **clockwise** when viewed from above (standard world-space convention).

## Coordinate Display Formats

### EQ `/loc` Command Output

The in-game `/loc` command displays location as:
```
Location: Y, X, Z
```

Example: `Location: 100.5, -250.3, 45.2` means Y=100.5, X=-250.3, Z=45.2.

### Map File Coordinate Transform

Map files (ShowEQ/LoY format, used by TextQuest) use **negated X and Y** relative to `/loc`:

```
Map coordinates: (-locY, -locX, locZ)
EQ /loc format: (locY, locX, locZ)
```

**Conversions:**
- `/loc` output → map file: negate both Y and X, keep Z
- Map file → `/loc` output: Y = -map_x, X = -map_y, Z = map_z

**Example:**
```
/loc output:      Y=100.5, X=-250.3, Z=45.2
Map file coords:  X=-100.5, Y=250.3, Z=45.2
```

### TextQuest Internal Representation

TextQuest internally uses **EQ world coordinates** directly (matching `/loc` output). When rendering maps:
- Player position: apply transform `(-player.y, -player.x)` to center view on map
- Map geometry: stored with negated X/Y to match map file format
- Navigation targets: stored in EQ coordinates, transformed at render time

## Zone Boundaries and Detection

### Zone Lines (Virtual Boundaries)

A **zone line** is a virtual boundary between two zones. When a character's position crosses the zone line, the EQ client triggers a zone transition.

**Characteristics:**
- Defined by coordinates (often a plane, line, or bounding box)
- Stored in EQ's internal zone configuration data
- Exposed via MQ2 through `ZoneGuideManagerClient` memory structure
- Not visible in the game world but physically enforced

### Zone Boundary Ranges

Each zone has a nominal bounding box:

```
BoundingBox = [min_x, max_x, min_y, max_y, min_z, max_z]
```

**Standard Camp Zones:**

| Zone | Min X | Max X | Min Y | Max Y | Min Z | Max Z | Zone ID | Notes |
|------|-------|-------|-------|-------|-------|-------|---------|-------|
| **Crescent Reach** | -1000 | 1000 | -1000 | 1000 | -50 | 200 | 394 | Tutorial/starter zone |
| **Crushbone Castle** (crushbone) | -2000 | 500 | -2500 | 1000 | -100 | 200 | 10 | Orc dungeon |
| **Guk (Lower)** (lguk) | -2000 | 500 | -1500 | 1000 | -200 | 100 | 2 | Troll dungeon |
| **Lost City of Sebilis** (sebilis) | -2000 | 500 | -1500 | 1000 | -200 | 100 | 17 | Iksar dungeon |
| **Mistmoore Castle** (mistmoore) | -3000 | 1000 | -2000 | 1500 | -100 | 300 | 33 | Vampire castle |
| **Unrest (Estate of Unrest)** (unrest) | -2000 | 500 | -1500 | 1000 | -100 | 200 | 38 | Haunted mansion |

**Note:** These bounds are approximate and may vary based on actual EQ data. Verify against live zone guide data via `ZoneGuideManagerClient`.

### Zone Safety and Landing Validation

After zoning, a player spawns at a **safe coordinate**—a predefined spawn point inside the destination zone:

**Safe Coordinates Validation:**
- Must be within zone bounds ±500 units
- Z coordinate must be above floor level (checked against collision geometry)
- Y/X must not be in collision/wall geometry
- Distance to zone-in point < threshold (typically 100-200 units)

**Recovery Modes:**
If a landing position is invalid:
1. **Fallback to zone-in point** — use the official zone entry spawn
2. **Fallback to safety waypoint** — use a manually configured safe location from navmesh
3. **Fallback to previous known good** — if available from recent movement history

See `textquest-common/src/safe_coords.rs` for `SafeCoordRequest` and validation logic.

## Zone Transition Detection

TextQuest detects zone transitions through multiple channels:

### 1. Zone Packet (`OP_Zone`)
- EQ server sends `OP_Zone` when zone transition is approved
- Contains new zone ID and safe spawn coordinates
- Most reliable signal for zone transition completion

### 2. Zone Entry Integrity Check
- DLL monitors for changes in `ZONE_ID` offset during movement
- Triggers when player crosses zone line boundary
- Pre-transition validation (blocks invalid transitions)

### 3. Position Validation Loop
- Continuous position monitoring checks if `(X, Y)` is within current zone bounds
- Detects stuck states (position outside bounds but no `OP_Zone` received)
- Triggers recovery after configurable timeout

### Zone Graph Structure

TextQuest maintains a **zone graph** in memory (`textquest-dll/src/nav/zone_graph.rs`):

```
ZoneGraph {
  nodes: Vec<ZoneNode>,        // One per zone
  edges: Vec<ZoneConnection>,  // Adjacency list
}

ZoneNode {
  zone_id: i32,
  name: String,                // e.g., "crushbone"
  min_level: i32,
  max_level: i32,
  connections: Vec<ZoneConnection>,
}

ZoneConnection {
  dest_zone_id: i32,
  dest_x: f32,
  dest_y: f32,
  dest_z: f32,
}
```

This graph is read from EQ's `ZoneGuideManagerClient` singleton at runtime.

## Map File Format (Brewall/ShowEQ)

### File Structure

Map files are **text-based** with line-oriented format. Common filename patterns:
- `{zonename}.txt` — main zone geometry
- `{zonename}_{layer}.txt` — layer-specific geometry (water, props, etc.)

### Line Syntax

#### Geometry Line (L)
```
L x1, y1, z1, x2, y2, z2, r, g, b
```

**Fields:**
- `x1, y1, z1` — Start point (3D world coordinates)
- `x2, y2, z2` — End point (3D world coordinates)
- `r, g, b` — RGB color (0-255 each)

**Example:**
```
L -3740.8, -985.0, -52.6, -3740.8, -982.9, -52.6, 0, 0, 0
```
Draws a black line from (-3740.8, -985.0, -52.6) to (-3740.8, -982.9, -52.6).

#### Point (P)
```
P x, y, z, r, g, b, size, label_text
```

**Fields:**
- `x, y, z` — Point position
- `r, g, b` — Color
- `size` — Marker size (pixels, relative)
- `label_text` — Annotation label

**Not yet documented in TextQuest config but available in ShowEQ/MQ2 map parsers.**

### Coordinate Conventions in Map Files

Map file coordinates use the **negated axis transform**:
- X in file = -(player Y from `/loc`)
- Y in file = -(player X from `/loc`)
- Z in file = player Z from `/loc`

This is critical when cross-referencing map geometry with in-game position reports.

### Layer Convention

Map files may be split into layers (files suffixed `_0`, `_1`, `_2`, etc.):

| Layer | Purpose | Typical Content |
|-------|---------|-----------------|
| 0 | Base geometry | Walls, floors, major structures |
| 1 | Labels | Text annotations, POI markers |
| 2 | Annotations | Routes, camps, waypoints |
| 3+ | Extended | Water, special geometry, temporary overlays |

When rendering, layers are typically composited in order (0 → 3+) to create the final map display.

### Standard Map Colors

Common RGB color conventions in EQ map files:

| Color | RGB | Usage |
|-------|-----|-------|
| Black | (0, 0, 0) | General geometry, walls |
| Gray | (200, 200, 200) | Secondary walls, columns |
| Brown | (205, 133, 63) | Wood, doors, furniture |
| Blue | (0, 100, 200) | Water, liquid |
| Red | (255, 0, 0) | Danger zones, fire |
| Green | (0, 200, 0) | Safe zones, grass |
| Yellow | (255, 255, 0) | Highlights, POIs |

**Note:** Color choice is aesthetic and varies by map creator. No strict standard exists, but consistency within a zone/project is helpful.

## Zone Exit Database

### Format

| From Zone | To Zone | Approx X | Approx Y | Approx Z | Notes |
|-----------|---------|----------|----------|----------|-------|
| crushbone | qey2hh1 | 100 | -150 | 5 | Main entrance, north side |
| crushbone | poknowledge | 0 | 0 | 0 | Portal (if available) |
| lguk | eastkarana | -200 | 300 | 50 | Underwater exit, northeast |
| sebilis | jungle | 500 | -100 | 25 | Main jungle entrance |
| mistmoore | commons | 100 | 200 | 10 | North tower exit |
| unrest | eastkarana | 400 | -200 | 45 | Basement to field |

### Zone ID Reference

TextQuest's `ZoneGuideManagerClient` provides authoritative zone IDs. Common camp zones:

```
poknowledge = 202   (Plane of Knowledge)
pok = 202           (Alternate name for PoK)
bazaar = 203        (Bazaar)
eastkarana = 3      (East Karana)
northkarana = 2     (North Karana)
gfaydark = 154      (Greater Faydark)
crushbone = 10      (Crushbone Castle)
lguk = 2            (Guk, Lower Level)
sebilis = 17        (Lost City of Sebilis)
mistmoore = 33      (Mistmoore Castle)
unrest = 38         (Estate of Unrest)
velks = 119         (Velketor's Labyrinth)
ntov = 202          (Najena's Tower of Veeshans, if available)
```

## Coordinate Validation and Bounds Checking

### Safe Coordinate Boundaries

```rust
/// Maximum coordinate magnitude for valid landing position.
pub const SAFE_COORD_MAX: f32 = 10_000.0;

/// Minimum position change to register as "moved".
pub const POSITION_EPSILON: f32 = 0.1;
```

### Out-of-Bounds Detection

A coordinate is considered **out-of-bounds** if:
1. X or Y magnitude exceeds `SAFE_COORD_MAX`
2. Z is below floor level (collision check required)
3. Position is inside solid geometry (collision check required)
4. Spawn distance from zone-in point exceeds threshold

Out-of-bounds detection triggers recovery logic in the orchestrator.

## References

- [EQ Coordinate System (docs/eq-coordinate-system.md)](../eq-coordinate-system.md) — Original coordinate system deep dive
- [Zone Transition State Map](../zone-transition-state-map.md) — FSM for zone transition handling
- [Map Rendering Pipeline](./Map-Rendering-Pipeline.md) — TUI map display architecture
- [Safe Coordinates (Code)](../../textquest-common/src/safe_coords.rs) — `SafeCoordRequest` / `SafeCoordResponse`
- [Zone Transition (Code)](../../textquest-common/src/zone_transition.rs) — Failure states and retry logic
- [Zone Graph Reader (Code)](../../textquest-dll/src/nav/zone_graph.rs) — Zone guide memory reading
- [Offsets Reference](./Offsets-EQ-Internals-and-MacroQuest-References.md) — Memory offsets and EQ internals
