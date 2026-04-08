# EQ Maps Research — TUI Integration

Research into integrating EverQuest zone maps into the Frostreaver TUI dashboard.

## 1. Map Sources

### Brewall's EQ Maps (Recommended)

- **Website:** https://www.eqmaps.info/eq-map-files/
- **Download:** https://www.eqmaps.info/wp-content/uploads/2024/01/brewall-20240109.zip
- **Format:** ZIP of `.txt` map files, one set per zone
- **Quality:** Most comprehensive and well-maintained. Cleaned up, corrected, with hunter info, collectible spawns, named mob locations, and merchant annotations.
- **Color standards:** Documented at https://www.eqmaps.info/eq-map-files/mapping-standards/

### Good's EQ Maps

- **GitHub:** https://github.com/RedGuides/goodurden-maps
- **RedGuides:** https://www.redguides.com/community/resources/goods-everquest-map-pack.303/
- **Format:** Same `.txt` format as Brewall's
- **Quality:** Extremely detailed, broad community support

### EQ Atlas (Legacy)

- Historical reference maps, mostly image-based (not parseable)
- Not useful for programmatic integration

**Recommendation:** Use Brewall's maps as primary source. They're the community standard, well-documented color conventions, and actively maintained.

## 2. Map File Format Specification

### File Naming Convention

Each zone has up to 4 layer files:

```
<zoneshortname>.txt      — Layer 0: Zone geometry (walls, terrain)
<zoneshortname>_1.txt    — Layer 1: Labels/points (NPC names, locations)
<zoneshortname>_2.txt    — Layer 2: Grid/coordinates (red X at origin)
<zoneshortname>_3.txt    — Layer 3: Custom annotations (blank by default)
```

Example: `ecommons.txt`, `ecommons_1.txt`, `ecommons_2.txt`, `ecommons_3.txt`

### L Lines — Line Segments

```
L x1, y1, z1, x2, y2, z2, r, g, b
```

| Field      | Type        | Description             |
| ---------- | ----------- | ----------------------- |
| x1, y1, z1 | float       | Start point coordinates |
| x2, y2, z2 | float       | End point coordinates   |
| r, g, b    | int (0-255) | Line color              |

Example:

```
L 2881.0, -2022.0, -295.0, 2885.0, -2027.0, -295.0, 128, 255, 0
```

### P Lines — Points / Labels

```
P x, y, z, r, g, b, size, label_text
```

| Field      | Type        | Description                                           |
| ---------- | ----------- | ----------------------------------------------------- |
| x, y, z    | float       | Point coordinates                                     |
| r, g, b    | int (0-255) | Label color                                           |
| size       | int (1-3)   | Font size                                             |
| label_text | string      | Text label (may contain commas, underscores = spaces) |

The label field is parsed with `splitn(8, ',')` so commas in the label text are preserved.

Example:

```
P 5531.2642, -168.7061, -299.5485, 128, 255, 0, 2, Gargoyle_Island
P -3710.0198, -1594.5485, -192.5240, 128, 255, 0, 2, Gull_Skytalon_(Named,Roam)
```

### Coordinate System

**Critical detail:** Map file coordinates use negated axes relative to EQ's `/loc` output.

```
Map file:  (-locY, -locX, locZ)
EQ /loc:   (locY, locX, locZ)
```

To convert from EQ `/loc` `(Y, X, Z)` to map coordinates: negate both Y and X.
To convert from map coordinates `(mx, my, mz)` to EQ world: `locY = -mx`, `locX = -my`, `locZ = mz`.

This is inherited from the ShowEQ/LoY cartography format and is a well-known source of confusion in the EQ development community.

### Layer System

- Layers 0-3 map to the 4 files (base, `_1`, `_2`, `_3`)
- In the EQ client, layers can be toggled visible/invisible
- Z-filtering is handled at render time, not encoded in the layer structure
- For our TUI, we primarily want Layer 0 (geometry) and Layer 1 (labels)

### Brewall's Color Conventions

**Geometry lines (Layer 0):**
| Color | RGB | Meaning |
|-------|-----|---------|
| Black | (0,0,0) | Main level walls/terrain |
| Dark→Light Purple | varying | Lower levels (depth gradient) |
| Green→Blue | varying | Upper levels (height gradient) |
| Violet Red | (199,21,133) | Zone lines |
| Grey | (150,150,150) | Steps/terrain transitions |
| Light Brown | (150,100,0) | Paths/trails |
| Light Blue | (70,130,180) | Water |
| Blue | (0,0,255) | Water (deep) |
| Red | (255,0,0) | Lava |
| Peru | (205,133,63) | Doors |
| Orange | (255,165,0) | Ladders |

**Labels (Layer 1):**
| Color | RGB | Meaning |
|-------|-----|---------|
| Red (large) | (255,0,0) | Zone connections |
| Green | (0,127,0) | Merchants |
| Gold | (255,210,0) | Bankers |
| Brown | varies | Named mobs (suffix: `(Named)`) |
| — | — | Ground spawns (prefix: `GS:`) |

## 3. MQ2Map Plugin Analysis

The MQ2Map plugin in a local MacroQuest checkout (5,108 lines total) is a **real-time spawn overlay**, not a zone geometry loader. Key insight:

- `MapGenerate()` iterates the live spawn list from memory
- Creates `MapObject` instances for each spawn (PC, NPC, pet, corpse, ground item)
- `MapUpdate()` syncs positions each frame
- 37 filter types (`MapFilter` enum) control visibility

**What MQ2Map does NOT do:** It does not parse zone map `.txt` files. Zone geometry rendering is handled by the EQ client's built-in `MapViewMap` window, which MQ2Map hooks into via `PostDraw`.

**In-memory structs** (from `eqlib/game/UI.h`):

```cpp
struct MapViewLine {          // sizeof 0x30
    MapViewLine*  pNext;
    MapViewLine*  pPrev;
    CVector3      Start;      // x, y, z
    CVector3      End;        // x, y, z
    ARGBCOLOR     Color;
    int           Layer;      // 0-3
};

struct MapViewLabel {         // sizeof 0x4c
    uint32_t      LabelId;
    MapViewLabel* pNext;
    MapViewLabel* pPrev;
    CVector3      Location;   // x, y, z
    ARGBCOLOR     Color;
    int           Size;       // 1-3
    const char*   Label;
    int           Layer;      // 0-3
    int           Width;
    int           Height;
    int           OffsetX;
    int           OffsetY;
};
```

## 4. Rust Parser Design

### Structs

```rust
/// A line segment from an EQ map file (L line)
pub struct MapLine {
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
    pub x2: f32,
    pub y2: f32,
    pub z2: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A labeled point from an EQ map file (P line)
pub struct MapPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub size: u8,
    pub label: String,
}

/// All data for a single zone map
pub struct ZoneMap {
    pub zone_short_name: String,
    pub lines: Vec<MapLine>,       // from layer 0
    pub points: Vec<MapPoint>,     // from layer 1
    pub grid_lines: Vec<MapLine>,  // from layer 2 (optional)
}
```

### Parser

```rust
pub fn load_zone_map(map_dir: &Path, zone: &str) -> Result<ZoneMap> {
    let mut map = ZoneMap {
        zone_short_name: zone.to_string(),
        lines: Vec::new(),
        points: Vec::new(),
        grid_lines: Vec::new(),
    };

    // Load layer 0 (geometry)
    let base_path = map_dir.join(format!("{zone}.txt"));
    if base_path.exists() {
        parse_map_file(&base_path, &mut map.lines, &mut map.points)?;
    }

    // Load layer 1 (labels)
    let labels_path = map_dir.join(format!("{zone}_1.txt"));
    if labels_path.exists() {
        parse_map_file(&labels_path, &mut map.lines, &mut map.points)?;
    }

    Ok(map)
}

fn parse_map_file(
    path: &Path,
    lines: &mut Vec<MapLine>,
    points: &mut Vec<MapPoint>,
) -> Result<()> {
    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?.trim().to_string();
        if line.starts_with('L') {
            lines.push(parse_l_line(&line)?);
        } else if line.starts_with('P') {
            points.push(parse_p_line(&line)?);
        }
        // Skip blank lines and comments
    }
    Ok(())
}

fn parse_l_line(line: &str) -> Result<MapLine> {
    // Strip 'L' prefix, split on ',', trim each part, parse 9 fields
    let parts: Vec<&str> = line[1..].splitn(9, ',').map(|s| s.trim()).collect();
    // Parse x1,y1,z1, x2,y2,z2, r,g,b
    ...
}

fn parse_p_line(line: &str) -> Result<MapPoint> {
    // Strip 'P' prefix, splitn(8, ',') to preserve commas in label
    let parts: Vec<&str> = line[1..].splitn(8, ',').map(|s| s.trim()).collect();
    // Parse x,y,z, r,g,b, size, label
    ...
}
```

### Existing Rust Reference

- **eqformat_map** (MIT licensed): https://github.com/martinlindhe/eqformat_map
  - Clean Rust parser for this exact format
  - Can use as direct reference or even as a dependency

### Module Placement

Suggested location: `textquest-common/src/map.rs` (shared types) + `textquest/src/tui/map.rs` (TUI rendering)

## 5. TUI Rendering Plan

### Architecture

The map view should be a dedicated full-screen TUI mode (toggled from main dashboard), not the 10x10 mini-map. It renders zone geometry as ASCII art with spawn overlay.

### Coordinate Transformation

```
Terminal coordinates:  (col, row) where col=0..width, row=0..height
Map coordinates:       (mx, my, mz) — floats, potentially thousands of units

Transform pipeline:
1. Center on player position (px, py)
2. Apply zoom: scale = terminal_chars_per_eq_unit
3. Project to terminal grid:
   col = (mx - px) * scale + width/2
   row = (my - py) * scale + height/2
4. Clip to terminal bounds
```

Map file X → terminal column (horizontal), Map file Y → terminal row (vertical).

Since map coords are `(-locY, -locX)`, and our TUI reads locY/locX from memory:

```rust
let map_x = -loc_y;  // player's map X
let map_y = -loc_x;  // player's map Y
```

### Line Rasterization

Use Bresenham's line algorithm to rasterize `MapLine` segments into terminal cells:

```rust
fn rasterize_line(
    x1: f32, y1: f32, x2: f32, y2: f32,
    scale: f32, center_x: f32, center_y: f32,
    width: u16, height: u16,
    buf: &mut Vec<(u16, u16, Color)>,
) {
    // Transform to terminal coords
    let c1 = ((x1 - center_x) * scale + width as f32 / 2.0) as i32;
    let r1 = ((y1 - center_y) * scale + height as f32 / 2.0) as i32;
    let c2 = ((x2 - center_x) * scale + width as f32 / 2.0) as i32;
    let r2 = ((y2 - center_y) * scale + height as f32 / 2.0) as i32;

    // Bresenham's from (c1,r1) to (c2,r2), clipping to bounds
    // Push visible pixels to buf with mapped RGB color
}
```

### Zoom Levels

| Level      | Scale           | Coverage (~120 col terminal) | Use Case               |
| ---------- | --------------- | ---------------------------- | ---------------------- |
| 1 (far)    | 0.05 chars/unit | ~2400 EQ units               | Full zone overview     |
| 2          | 0.1             | ~1200 units                  | Area overview          |
| 3          | 0.2             | ~600 units                   | Neighborhood           |
| 4 (close)  | 0.5             | ~240 units                   | Immediate surroundings |
| 5 (detail) | 1.0             | ~120 units                   | Fine detail            |

Zoom controlled by `+`/`-` keys. Pan with arrow keys (or auto-follow player).

### Rendering Characters

| Element         | Character   | Notes                          |
| --------------- | ----------- | ------------------------------ |
| Horizontal wall | `─`         | Box-drawing                    |
| Vertical wall   | `│`         | Box-drawing                    |
| Diagonal        | `/` `\`     | Based on slope                 |
| Generic wall    | `·`         | Fallback for any line pixel    |
| Player          | `@`         | Bright green, always on top    |
| Group member    | `*`         | Cyan                           |
| NPC             | `+`         | Yellow                         |
| Named NPC       | `!`         | Red, bold                      |
| Corpse          | `%`         | Dark grey                      |
| Label text      | actual text | Rendered at map point location |

### Z-Level Filtering

Use the player's current Z (elevation) to filter which lines to render:

```rust
const Z_TOLERANCE: f32 = 50.0; // tunable
let visible = line.z1.abs() - player_z.abs() < Z_TOLERANCE
           || line.z2.abs() - player_z.abs() < Z_TOLERANCE;
```

This prevents rendering floors above/below the player, which is critical for multi-level dungeons.

### Color Mapping

Map RGB colors to terminal colors (ratatui `Color::Rgb` for true-color terminals):

```rust
fn map_color(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)  // True color — most modern terminals support this
}
```

Fallback for 256-color terminals: quantize to nearest ANSI color.

### Spawn Overlay

Overlay live spawn data on top of the static map:

1. Read spawn positions from existing `SpawnInfo` data (already available in TUI)
2. Transform spawn coordinates to terminal coords using same pipeline
3. Render spawn glyphs on top of map geometry
4. Use spawn type to determine glyph and color

### Performance Considerations

- **Pre-parse:** Load and parse map files once at zone entry, store in `ZoneMap`
- **Spatial index:** For large zones (10k+ lines), build a simple grid-based spatial index to only rasterize lines within the viewport
- **Frame budget:** Only re-rasterize when zoom/pan/position changes, cache the rendered buffer
- **Map file sizes:** Typical zone map is 50-500 KB of text, parses in <10ms

### UI Layout

```
┌─ Map: East Commonlands ──────────────────────────────────┐
│                                                          │
│    ───────                                               │
│   │       │        Merchant_Area                         │
│   │   @   │     ·····                                    │
│   │       │    ·     ·                                   │
│    ───────    ·   +   ·    ! Frenzy_(Named)              │
│               ·     ·                                    │
│                ·····                                     │
│                                                          │
│                              * Groupmember1              │
│                              * Groupmember2              │
│                                                          │
├──────────────────────────────────────────────────────────┤
│ Zoom: 3 (0.2x) │ Center: @Player │ Z-filter: -295±50    │
│ [+/-] Zoom  [Arrows] Pan  [F] Follow  [Z] Z-filter  [Q] │
└──────────────────────────────────────────────────────────┘
```

### Implementation Phases

1. **Phase 1 — Parser:** `textquest-common/src/map.rs` with `MapLine`, `MapPoint`, `ZoneMap`, `load_zone_map()`
2. **Phase 2 — Static render:** New TUI screen that loads a zone map and renders geometry with Bresenham's
3. **Phase 3 — Player tracking:** Center on player position, auto-follow mode
4. **Phase 4 — Spawn overlay:** Render live spawns on top of map
5. **Phase 5 — Polish:** Z-filtering, zoom/pan controls, label rendering, color mapping

## 6. References

- Brewall's Maps: https://www.eqmaps.info/eq-map-files/
- Brewall's Color Standards: https://www.eqmaps.info/eq-map-files/mapping-standards/
- Good's Maps (GitHub): https://github.com/RedGuides/goodurden-maps
- eqformat_map Rust parser (MIT): https://github.com/martinlindhe/eqformat_map
- nox-maps Python parser: https://github.com/devin-hart/nox-maps
- ZlizEQMap C# parser: https://github.com/hada79/ZlizEQMap
- MQ2Map plugin docs: https://docs.macroquest.org/plugins/core-plugins/mq2map/
- MQ2Map source: local MacroQuest `plugins/map/` sources (5,108 lines)
- MapViewLine/MapViewLabel structs: MacroQuest eqlib `UI.h`
