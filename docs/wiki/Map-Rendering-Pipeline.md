# Map Rendering Pipeline

## Overview

The TextQuest tactical map rendering system converts 3D EverQuest world coordinates into a 2D character-based grid displayed in the TUI. The pipeline combines zone geometry, spawn positions, navigation paths, and player/target overlays into a single coherent view.

**Key entry point:** `draw_map_view()` in `textquest/src/tui/ui/map.rs`

The rendering process:

1. **Compute view bounds** — Merge zone map bounds, navmesh bounds, and spawn extents
2. **Calculate transform** — Convert 3D world coordinates to screen grid positions with zoom/pan
3. **Cull & render layers** — Draw geometry, labels, spawns, paths, and overlays in order
4. **Paint to grid** — Accumulate characters and colors into a 2D grid
5. **Finalize & display** — Convert grid rows to ratatui `Span` objects for rendering

---

## Coordinate Systems

### EQ World Coordinates

EverQuest uses a **right-handed coordinate system**:
- **Y-axis** — North is positive; South is negative
- **X-axis** — East is positive; West is negative  
- **Z-axis** — Up is positive; Down is negative (elevation)
- **Heading** — 0=North, 128=West, 256=South, 384=East; 512 units = full circle (clockwise in world space)

### Map Coordinates

Internal transformation inverts both axes via `(-player.y, -player.x)`:
- A 180° rotation that flips the coordinate system
- Preserves angular direction (clockwise remains clockwise)
- Centers the player at the origin for local view calculations

### Screen Coordinates

The final grid uses standard screen coordinates:
- **Column (X)** — Increases to the right
- **Row (Y)** — Increases downward (inverted from typical math)
- Each cell = 1 character in the TUI grid

### Transformation Sequence

```
EQ World (x, y, z)
    ↓ [Apply axis swap + negate: (-y, -x)]
Map Coords (mx, my)
    ↓ [Apply scale & center translate]
Grid Position (col, row)
    ↓ [Bounds check & clamp]
Screen (char, color)
```

---

## Transformation Pipeline

### Step 1: Compute View Bounds (`combined_bounds()`)

Merges all visible data sources into a bounding box:
- Zone map geometry bounds (if map data loaded)
- Navigation mesh overlay bounds (if visible)
- Spawn position extents (fallback if no map data)

Returns a `ViewBounds` struct with `(min_x, max_x, min_y, max_y)` in map coordinates.

### Step 2: Calculate Transform (`map_transform()`)

Computes a `MapTransform` that maps map coordinates to grid positions:

- **Viewport mode selection**
  - `Auto` — Uses local view if player is in bounds and map is large (> 1200 units)
  - `Local` — Centers on player (if player position available)
  - `Global` — Shows full map bounds

- **For local view** — Camera follows player
  - Half-height = 180 units (260 if map maximized)
  - Aspect ratio clamped to [1.0, 2.6]
  - Scale = pixels per unit

- **For global view** — Fits entire bounds
  - Scale = (grid_width - 2) / bound.width OR (grid_height - 2) / bound.height (whichever is smaller)

- **Apply zoom & pan**
  - `scale = base_scale * zoom_factor`
  - `center_x += pan_x; center_y += pan_y`
  - Clamp center to prevent viewing area from escaping bounds

Returns `MapTransform` with `(center_x, center_y, scale_x, scale_y, using_local_view)`.

### Step 3: Grid Projection (`to_grid` closure)

Inline closure that transforms map coordinates to grid indices:

```rust
let to_grid = |mx: f32, my: f32| -> (i32, i32) {
    let col = ((mx - transform.center_x) * transform.scale_x + w as f32 / 2.0) as i32;
    let row = ((my - transform.center_y) * transform.scale_y + h as f32 / 2.0) as i32;
    (col, row)
};
```

- Subtracts transform center (camera position)
- Multiplies by scale (pixels per unit)
- Adds grid center offset (w/2, h/2) to center the view
- Returns grid cell indices (may be out of bounds)

---

## Rendering Layers

Layers are drawn in order (back-to-front):

1. **Geometry** (if `show_geometry`)
   - Zone map lines with Z-clipping
   - Uses `clip_project_draw_line()` to handle height variations
   - Color-mapped by RGB values from zone data

2. **Labels** (if `show_labels` and zoom ≥ 0.8)
   - Point markers from zone map
   - First character of label placed at point location

3. **Navmesh** (if `show_navmesh`)
   - Outer boundary lines (always drawn)
   - Inner detail lines (only if local view or zoom ≥ 1.35)

4. **Spawns** (if `show_spawns`)
   - Cached spawn positions with glyph selection
   - Glyphs: selected (◍) > group (⊕) > named (◆) > PC (class letter) > NPC (○) > corpse (†)
   - Spawn count clustering at zoom < 0.95

5. **Navigation Paths** (if `show_nav_paths`)
   - Waypoint lines drawn with bresenham
   - Directional arrows at segment midpoints
   - Numbered waypoints (1–9, then +)
   - Star (★) at final destination

6. **Target Line** (if target selected)
   - Bresenham line from player to target
   - Crosshair (✚) marker at target

7. **Player Marker & FOV**
   - Player position (◆) in map_you color
   - FOV cone: 60° wedge ahead of player
   - Heading arrow in adjacent cell

8. **Overlays**
   - Radius circles (aggro, vision, etc.)
   - Loc marker (⊗) with label
   - Named NPC persistent markers
   - Camp locations
   - Spawn highlights with pulse animation

---

## Z-Clipping (`clip_line_z()`)

Culls 3D line segments based on player height with a vertical range filter.

**Purpose:** Remove geometry and other elements that are too high or too low relative to the player.

**Input:**
- Line endpoints `(x1, y1, z1)` and `(x2, y2, z2)` in world coordinates
- `center_z` — Player's Z height
- `z_range` — Half-height of visible range (e.g., 50 units = 100 unit window)

**Output:**
- `None` if line is fully outside the range (culled)
- `Some((x1, y1, x2, y2))` — clipped XY endpoints, or original if inside

**Algorithm:**
1. Check if both endpoints are inside `[center_z - z_range, center_z + z_range]`
2. If both inside → return unmodified
3. If both outside on same side → return `None` (culled)
4. If one or both outside → interpolate XY at boundary crossing using parameter `t`:
   - `t = (boundary_z - z1) / (z2 - z1)`
   - `clipped_x = x1 + t * (x2 - x1)` (same for Y)

**Note:** Clipping occurs in 3D space; the result is 2D (Z is discarded after determining visibility).

---

## Color Mapping (`map_rgb_to_color()`)

Converts zone map RGB colors to ratatui `Color` values.

**Thresholds:**
- Bright colors (high luminance) → White or Yellow
- Mid-tone colors → mapped to theme secondary color
- Dark colors → mapped to theme dim color

Uses the zone map's original RGB data to preserve visual distinction between different zone regions.

---

## Performance Optimizations

### 1. Spawn Position Caching (`map_spawn_cache`)

Spawns are expensive to re-render every frame:
- Cache key includes transform, zoom, filters, z_range, selection
- Only rebuild if key changes
- Cells stored in flat vector (one render pass)
- Clustering at zoom < 0.95 reduces cell count

### 2. Visible Region Culling

Before rendering geometry or labels:
- Compute `VisibleMapRegion` from transform and grid size
- Test bounding boxes with `contains_line()` / `contains_point()`
- Skip geometry outside the view frustum

### 3. Bresenham Line Drawing with Bounds Checking

- `bresenham_line()` iterates through cells, respecting grid bounds
- `grid_in_bounds()` prevents out-of-bounds writes
- Paint modes control overwriting (blank-only vs. full overwrite)

### 4. Lazy Geometry Rendering

- Geometry rendered only if `show_geometry` is true
- Labels only rendered at zoom ≥ 0.8 (reduce clutter at far zoom)
- Navmesh inner lines only at local view or zoom ≥ 1.35

### 5. PERF_TRACE_ENABLED

Optional performance tracing logs:
- Spawn cache rebuild timing
- Cell count and layer visibility
- Enabled via environment variable `TEXTQUEST_PERF_TRACE`

---

## Key Data Structures

### `ViewBounds`

Represents a 2D bounding box in map coordinates:

```rust
struct ViewBounds {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}
```

Methods: `width()`, `height()`, `center_x()`, `center_y()`, `contains_with_margin()`.

### `MapTransform`

Result of coordinate transformation calculation:

```rust
struct MapTransform {
    center_x: f32,        // Camera center in map coordinates
    center_y: f32,
    scale_x: f32,         // Pixels per unit (typically 1.0)
    scale_y: f32,
    using_local_view: bool,
}
```

### `VisibleMapRegion`

Computed from `MapTransform` for frustum culling:

```rust
struct VisibleMapRegion {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}
```

Methods: `contains_line()`, `contains_point()`.

### `MapSpawnPresentationCell`

Cached spawn cell for fast rendering:

```rust
struct MapSpawnPresentationCell {
    row: u16,
    col: u16,
    ch: char,
    color: Color,
}
```

---

## Common Patterns

### Drawing Overlays

1. Compute world position of overlay element
2. Transform to grid via `to_grid(-pos.y, -pos.x)` (note axis inversion)
3. Bounds check with `grid_in_bounds(col, row, w, h)`
4. Write to grid or call `bresenham_line()` for connected elements

### Z-Clipped Rendering

For 3D geometry that needs height culling:
1. Call `clip_line_z()` with line endpoints and player Z
2. If returns `Some((x1, y1, x2, y2))`, the line is visible
3. Pass clipped coordinates to `clip_project_draw_line()` or `to_grid()`

### Heading & Direction

- EQ heading: 0=North, 128=West, 256=South, 384=East (512 total)
- Formula for screen angle: `(512 - heading_eq) * π / 256` radians
- Direction arrows use `signum()` to map 8-direction compass

---

## Testing & Debugging

### Map Legend & Info Line

The header displays:
- Zone name and coordinate ranges
- View mode (Auto/Local/Global)
- Zoom level and camera center
- Layer flags: [GSPMLA] = Geometry, Spawns, Paths, Mesh, Labels, Annotations
- Z-range filter and spawn type filters

### Common Issues

**"Map transform unavailable"**
- No zone map data loaded or no spawn data
- Check `combined_bounds()` returns `Some(bounds)`

**Geometry not rendering**
- Verify `show_geometry` toggle is on
- Check Z-range doesn't cull all geometry
- Confirm zone map file exists

**Spawns missing**
- Verify `show_spawns` toggle is on
- Check spawn filters aren't hiding all types
- Confirm spawns are within z_range

**Navigation path not visible**
- Verify `show_nav_paths` and `show_target_path` are both on
- Check player has active navigation waypoints
- Zoom level may be too far out

---

## See Also

- `textquest/src/tui/ui/map.rs` — Full implementation
- `textquest/src/eq/map_parser.rs` — Zone map file format
- `textquest_common/src/nav.rs` — Navigation types (Waypoint, heading calculations)
