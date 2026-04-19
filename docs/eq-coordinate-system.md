# EQ Coordinate System

## EQ World Coordinates

EverQuest uses a **right-handed coordinate system** with three axes:

| Axis | Direction | Notes |
|------|-----------|-------|
| Y    | North (+), South (-) | Primary horizontal axis |
| X    | East (+), West (-)    | Secondary horizontal axis |
| Z    | Up (+), Down (-)     | Elevation/altitude |

### Heading

- **0** = North
- **128** = West
- **256** = South
- **384** = East
- **512** = Full circle (clockwise in world space)

Heading is stored as a `u16` in EQ's internal structures.

## Coordinate Display Formats

### EQ `/loc` Output

The in-game `/loc` command displays coordinates as:
```
Location: Y, X, Z
```

This is `(North-South, East-West, Elevation)` — a source of common confusion.

### Map File Coordinates

The ShowEQ/LoY-derived map format uses **negated axes** relative to `/loc`:

```
Map file:  (-locY, -locX, locZ)
EQ /loc:   (locY, locX, locZ)
```

**Conversions:**
- `/loc` → map: negate Y and X
- Map → `/loc`: Y = -map_x, X = -map_y, Z = map_z

### TextQuest Internal

TextQuest uses EQ world coordinates directly (matching `/loc` output). The map transformation applies `(-player.y, -player.x)` to center the view.

## Zone Boundaries

### Zone Lines

Zone lines are **virtual boundaries** between zones. When a character crosses a zone line coordinate, the EQ client triggers a zone transition.

**Characteristics:**
- Defined by coordinates (often a line or plane)
- Stored in EQ's zone configuration data
- Exposed via MQ2 memory reads (see `MQ2Type/ZoneType.cpp`)
- TextQuest stores zone adjacency in `zone_graph.rs`

### Zone Line Detection

TextQuest detects zone transitions via:
1. **Zone packet** — `OP_Zone` packet received
2. **Zone change event** — `OnZone` callback in DLL
3. **Position validation** — checking if player is within known zone bounds

### Zone Graph

The TextQuest zone graph (`textquest-common/src/nav/zone_graph.rs`) models:
- Zone adjacency (which zones connect)
- Zone line coordinates for each connection
- Zone types (interior, exterior, instanced)

### Safe Coordinates

After zoning, players spawn at **safe coordinates** — predefined positions inside the destination zone. These are stored in EQ's zone data and exposed via:

```
- Player spawn point (first login or death run)
-绑定的 zone-in point (for regular zone lines)
-绑定的 safe location (for recalls, ports, etc.)
```

TextQuest uses `is_safe_coordinate()` in `nav/zone_transition.rs` to validate waypoints.

## Coordinate Offsets

All TextQuest offsets in `textquest-common/src/offsets.rs` use **preferred-base** (`0x140000000`) values. Always call `offsets::rebase(addr, actual_base)` before using.

```rust
// CORRECT — rebase to actual loaded module base
let hp = proc.read::<u64>(offsets::rebase(offsets::PLAYER_HP, eq_base))?;
```

## References

- [Map Rendering Pipeline](wiki/Map-Rendering-Pipeline.md)
- [Research: EQ Maps](wiki/Research-EQ-Maps.md)
- [Zone Transition State Map](zone-transition-state-map.md)
- [Offsets, EQ Internals, and MacroQuest References](wiki/Offsets-EQ-Internals-and-MacroQuest-References.md)