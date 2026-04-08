# TUI Map Rendering & Polish Research

> Research date: 2026-04-03
> Scope: Brewall map format, ratatui rendering strategy, TUI polish recommendations

---

## 1. Brewall/ShowEQ Map Format Specification

### File Structure

Brewall maps use the **ShowEQ/MQ2 map format** — plain text files with one record per line. Each zone has up to 4 layer files:

| File         | Layer        | Purpose                                                        |
| ------------ | ------------ | -------------------------------------------------------------- |
| `zone.txt`   | 0 (base)     | Zone geometry — walls, terrain, water boundaries               |
| `zone_1.txt` | 1 (labels)   | POI labels — zone connections, merchants, quest NPCs           |
| `zone_2.txt` | 2 (custom)   | Custom additions — compass markers, grid overlays, annotations |
| `zone_3.txt` | 3 (extended) | Additional geometry (rare, large zones only)                   |

TextQuest currently loads all 4 layers via `textquest/src/eq/map_parser.rs:load_zone_map`.

### Line Types

**L (Line segment):**

```
L x1, y1, z1, x2, y2, z2, r, g, b
```

- 9 comma-separated fields after the `L` prefix
- Coordinates are EQ world-space floats (Y-up in EQ, but map files use X,Y,Z order)
- RGB color values 0-255
- `(0,0,0)` = default/black — TextQuest maps these to `theme.map_lines` (dim gray)
- Colored lines typically represent: zone boundaries (red 255,0,0), water (blue), elevation changes (gray 150,150,150), buildings (varied)

**P (Point/label):**

```
P x, y, z, r, g, b, size, label_text
```

- 8 fields (splitn by comma, last field preserves internal commas)
- `size` = display priority/marker size (1-3 typically; 3 = zone connections, 2 = POIs, 1 = minor)
- Labels use underscores for spaces (parser converts: `Druid_Ring` → `Druid Ring`)
- Color conventions: red (255,0,0) = zone exits, green (0,128,0) = merchants/NPCs, white (255,255,255) = landmarks

### Coordinate System

**Critical: EQ's coordinate system is non-standard.**

- EQ world: Y increases North, X increases East, Z is vertical (up)
- Map files store coordinates as-is from EQ
- TextQuest's map rendering negates both axes: `to_grid(-spawn.y, -spawn.x)` — this flips the map so North=up, East=right on screen
- The `MapBounds` bounding box is computed from raw map coordinates, and the `to_grid` transform handles the axis mapping

### Data Scale

From actual `config/maps/` analysis:

- **1,707 zone map files** (569 zones × 3 layer files each)
- Typical zone: 900-4,500 line segments (e.g., commons.txt = 1,299 lines, freporte.txt = 4,438)
- Labels: 5-40 POIs per zone
- Coordinate ranges: zones span roughly 2,000-12,000 units across

### What's Already Working

The current implementation in `map_parser.rs` and `map.rs` is **fully functional**:

- Parses all L and P lines correctly
- Handles commas in labels, malformed lines (with warnings)
- Sorts lines by Z-depth (back-to-front rendering)
- Sorts points by Z-depth and size (priority ordering)
- Computes bounding box for all geometry
- Layer 2 (`_2.txt`) contains compass/grid overlays — these render as red crosshair-like shapes

---

## 2. Current Map Rendering Architecture

### How It Works Now

The map renderer (`textquest/src/tui/ui/map.rs`, ~1,500+ lines) uses a **character grid** approach:

1. **Grid allocation**: `Vec<Vec<(char, Color)>>` — one cell per terminal character
2. **Transform pipeline**: `MapTransform` converts world coords → grid coords via center/scale
3. **Layer rendering** (back to front):
   - Zone geometry lines (Bresenham line drawing with Unicode line chars: `─`, `│`, `╱`, `╲`, `·`)
   - Map labels (first char as marker + text overflow)
   - Navmesh overlay (outer + inner lines)
   - Spawn dots (with clustering for zoomed-out views)
   - Named tracker dead markers (`✕`)
   - Nav path overlay (waypoint lines + `★` destination)
   - Target line (`✚` crosshair)
   - Player marker (`◆`) with FOV cone
4. **Color run optimization**: Consecutive same-color cells are batched into single `Span` objects
5. **Minimap**: Corner overlay with full-zone overview at reduced resolution
6. **Visible region culling**: Only processes lines/points within the current viewport

### Rendering Features Already Implemented

- **Zoom**: Scroll-based, 0.1x-10x range
- **Pan**: Arrow-key based, speed scales with zoom level
- **Viewport modes**: Auto (switches local/global), Local (player-centered), Global (zone-centered)
- **Toggle layers**: Geometry (G), Spawns (S), Paths (P), Mesh (M), Labels (L)
- **Z-filter**: Vertical range filter (±10-500 units, adjustable with +/-)
- **Spawn clustering**: At zoom < 0.95, overlapping spawns show count digits
- **Spawn filtering**: Filter by type (PC, NPC, named, corpse, etc.)
- **Map maximize**: `m` key expands map to full screen with dock below
- **Minimap**: Auto-sizes in corner, shows current viewport position
- **Spawn cache**: Invalidated on state change, avoids per-frame rebuild
- **Performance tracing**: Optional `PERF_TRACE_ENV` timing

### What's NOT a Problem

The "bunch of lines" complaint is likely about **visual quality at default zoom**, not a parsing/rendering bug. The map data loads and renders correctly. The issues are:

1. **Character-grid resolution**: At 1x zoom, each terminal cell represents ~20-50 EQ units, so fine detail is lost
2. **Monochrome appearance**: `(0,0,0)` RGB lines all map to dim gray, making zones look flat
3. **No sub-character rendering**: ratatui's Canvas widget with Braille dots could provide 2x4 subpixel resolution per cell
4. **Layer 2 noise**: Compass/grid overlay lines clutter the view

---

## 3. Rendering Strategy Recommendations

### Option A: Enhanced Character Grid (Incremental, Low Risk)

Keep the current approach but improve visual quality:

**Color enhancement:**

- Map `(0,0,0)` lines by context: walls → brighter, terrain boundaries → different shade
- Use the map file's actual RGB colors more aggressively (many lines have non-black colors)
- Add a "color boost" mode that brightens dim map colors for terminal visibility

**Character refinement:**

- Use Braille dots (`⠁⠂⠃...⣿`) for density visualization where many lines overlap
- Use `▀▄█░▒▓` block elements for filled areas
- Better diagonal characters for slopes

**Layer filtering:**

- Toggle layer 2 (compass/grid annotations) separately — it's visual noise at most zoom levels
- Default to hiding layer 2 at zoom < 1.5

**Estimated effort**: 2-4 hours. No architectural change.

### Option B: ratatui Canvas Widget (Higher Quality, Medium Effort)

Switch from character grid to ratatui's `Canvas` widget:

```rust
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Points};
```

**Advantages:**

- Built-in coordinate mapping (`x_bounds`, `y_bounds`)
- Braille marker mode: each terminal cell becomes a 2x4 dot grid (8x resolution)
- Native line drawing with sub-cell precision
- Built-in `Shape` trait for custom geometry

**Implementation sketch:**

```rust
let canvas = Canvas::default()
    .block(panel("Map", border_style, t))
    .x_bounds([transform.min_x as f64, transform.max_x as f64])
    .y_bounds([transform.min_y as f64, transform.max_y as f64])
    .marker(Marker::Braille)  // or Marker::HalfBlock for 2x resolution
    .paint(|ctx| {
        // Zone geometry
        for ml in &map.lines {
            ctx.draw(&CanvasLine {
                x1: ml.x1 as f64, y1: ml.y1 as f64,
                x2: ml.x2 as f64, y2: ml.y2 as f64,
                color: Color::Rgb(ml.r, ml.g, ml.b),
            });
        }
        // Spawns as points
        ctx.draw(&Points {
            coords: &spawn_positions,
            color: Color::Green,
        });
        // Labels
        ctx.print(x, y, Span::styled("Label", style));
    });
```

**Tradeoffs:**

- Canvas paints shapes via callback — harder to layer with priority control
- Current character grid gives precise per-cell control (spawn glyphs, legend row)
- Canvas doesn't support per-character glyph customization (no `◆`, `✕`, `★` markers)
- **Hybrid approach possible**: Use Canvas for geometry, overlay character-based spawns on top

**Estimated effort**: 8-16 hours. Requires rethinking the spawn overlay and legend.

### Option C: Hybrid Canvas + Character Overlay (Recommended)

Best of both worlds:

1. **Canvas layer** for zone geometry (Braille or HalfBlock markers for smooth lines)
2. **Character overlay** for spawns, labels, player markers (keep current glyph system)
3. Render Canvas to a buffer, then stamp character overlays on top

**Implementation approach:**

- Render the Canvas widget to a temporary `Buffer`
- Copy the buffer to the frame
- Then overlay spawn characters directly on frame cells where spawns exist
- This preserves the current spawn glyph system (`◆`, `@`, `·`, `!`, `✕`, `★`, `✚`)

**Estimated effort**: 12-20 hours. Best visual quality with preserved UX.

### Recommendation

**Start with Option A** (2-4 hours) for immediate improvement. The current rendering is architecturally sound — it just needs visual tuning. Then evaluate Option C for M6 web dashboard timeline if the character grid feels insufficient.

Key quick wins for Option A:

1. Brighten `(0,0,0)` map lines — use `Rgb(70, 75, 85)` instead of theme's muted gray
2. Hide layer 2 by default (add toggle)
3. Increase default zoom for indoor zones (smaller bounding box → higher detail)
4. Use map line colors when available (currently many lines are painted with non-zero RGB that works well)

---

## 4. TUI Polish Recommendations

### Current State Assessment

The TUI is already **well above average** for a Rust TUI application:

**Strengths (keep these):**

- Theme system with 3 polished themes (Dark Modern, Classic, Dracula) — semantic colors, not inline literals
- Rounded borders (Unicode box-drawing)
- Responsive layouts with named width breakpoints (`WIDTH_MAP_STACK`, `WIDTH_MAP_NARROW`, etc.)
- Color-coded spawn types, HP bars, con colors
- Tab bar with active/inactive states
- Status bar with keyboard hints
- Help overlay system
- Performance-conscious rendering (spawn cache, color-run batching, visible-region culling)

**Areas for improvement:**

### 4.1 Visual Hierarchy

**Header density**: The map title bar packs too much info into one line:

```
Map: commons (1299 lines, 20 labels) | You y:-50 x:100 z:-54 | Sel none | View: auto/global 1.00x | center:0,0 | layers[GSPM L] | Z: 50 [+/-] | Mesh: n/a | m maximize
```

**Recommendation**: Split into 2 rows — title + coordinates on top, toggle states + controls on bottom. Or move layer/filter indicators into the status bar.

### 4.2 Panel Focus Indicators

Current: Active panel gets `border_active` (cyan), inactive gets `border_dim` (gray). This is standard but subtle.

**Enhancement**: Add a bold title or background tint to the focused panel header. Example:

```rust
// Active panel: bright title with indicator
" ▸ Map: commons "
// Inactive panel: dim title
" Map: commons "
```

### 4.3 Spawn List Polish

The spawn list alongside the map is functional. Improvements:

- **Search/filter bar**: Type to filter spawns by name (useful with 200+ spawns in busy zones)
- **Sticky headers**: Group headers (PC, NPC, Named, Corpse) should stay visible during scroll
- **Distance column**: Show distance from player in the spawn list (already have coords, just need `sqrt(dx²+dy²)`)

### 4.4 Color Palette Consistency

The Dark Modern theme uses good colors but could benefit from:

- **Accent hierarchy**: Primary accent (cyan) is used for both borders and text — consider a secondary accent
- **Dim background differentiation**: Active panels could have a slightly lighter background (`Rgb(20, 24, 30)`) vs inactive (`Rgb(15, 18, 24)`)
- **Con colors should use theme RGB**: Classic theme uses named `Color::Red` etc. which look harsh — the Dracula approach (custom RGB per con color) looks better

### 4.5 Animation and Feedback

- **Toast messages**: Current `status_message` is text-only. Add a brief highlight flash (1-2 frames with accent bg) when toggling layers
- **Smooth zoom**: Currently instant — a 2-3 frame interpolation would feel more polished (but adds complexity)
- **Loading indicator**: When a large zone map is parsing, show a brief spinner or "Loading..." in the map area

### 4.6 Responsive Layout Improvements

Current breakpoints are well-defined. Additions:

- **Ultra-narrow mode** (< 80 cols): Hide minimap, collapse legend to icons only
- **Ultra-wide mode** (> 200 cols): Add a third column for named tracker + nav status alongside the map
- **Terminal resize**: Already handled by ratatui's constraint system — just ensure minimaps and overlays recalculate correctly

### 4.7 Keyboard Shortcut Discoverability

- **Contextual hints**: Show relevant shortcuts in the panel border, e.g., `[z] zoom [v] view [g/s/p/m/l] layers`
- **Vim-style command palette**: `:` command mode already exists — ensure all map operations are accessible through it

### 4.8 Unicode and Typography

Already using:

- Rounded box-drawing (`╭╮╰╯`)
- Special markers (`◆◎✕★✚·`)
- Bold modifiers for emphasis

Could add:

- **Separator lines**: `─` thin horizontal rules between sections
- **Nerd Font icons**: If user's terminal supports it, use `for map,` for player, etc. (but make optional — not all terminals support Nerd Fonts)
- **Consistent em-dash / bullet use**: Standardize on `─` for horizontal rules, `•` for list items

---

## 5. Priority List of Improvements

### Tier 1: Quick Wins (1-2 hours each)

1. **Brighten map geometry colors** — Replace `(0,0,0)` → dim gray with a brighter default; use actual RGB colors from map files
2. **Layer 2 toggle** — Add separate toggle for compass/grid layer (or hide by default)
3. **Map title bar compaction** — Move layer toggles and filter info to status bar or second row
4. **Spawn distance column** — Add distance-from-player to spawn list

### Tier 2: Medium Effort (4-8 hours each)

5. **Spawn search/filter** — Type-to-search in spawn list panel
6. **Canvas-based geometry rendering** — Use `ratatui::widgets::canvas::Canvas` with Braille markers for zone lines (8x resolution)
7. **Panel focus enhancement** — Bold titles, subtle bg tint for active panels
8. **Contextual keyboard hints** — Show relevant shortcuts in panel borders

### Tier 3: Larger Features (8-16 hours each)

9. **Hybrid Canvas + Character overlay** — Canvas for geometry, character overlay for spawns/markers
10. **Zone transition visualization** — Highlight zone exits with connecting zone names
11. **Named mob patrol paths** — Render historical position data as ghost trails
12. **Camp radius visualization** — Circle overlay showing camp pull range

### Tier 4: Future / M6 Web Dashboard

13. **Web-based map** — Canvas/SVG rendering in React for the M6 web dashboard
14. **Multi-zone overview** — Show adjacent zones as dimmed outlines
15. **Real-time heat map** — Spawn density visualization over time

---

## 6. Technical Notes

### ratatui Canvas Widget Details

```rust
// Available marker types for Canvas:
Marker::Dot       // 1:1 - one dot per cell (current equivalent)
Marker::Braille   // 2x4 - 8 dots per cell (best for line rendering)
Marker::HalfBlock // 1x2 - 2 dots per cell (good balance)
Marker::Block     // 1:1 - filled blocks
Marker::Bar       // 1:1 - bar characters
```

Braille gives the highest effective resolution: a 100x50 terminal area becomes 200x200 effective pixels. This is the standard approach for high-quality TUI maps (used by `btm`/bottom, `gpustat`, etc.).

### EQ Coordinate Axis Mapping

```
EQ World:  X=East/West, Y=North/South, Z=Up/Down
Map Files: Same as EQ (X, Y, Z order in L/P lines)
TextQuest Grid: to_grid(-spawn.y, -spawn.x) → (col, row)
           This means: map_X = -EQ_Y, map_Y = -EQ_X
           North = up, East = right on screen ✓
```

### Performance Considerations

Current rendering is already well-optimized:

- **Spawn cache**: Only rebuilds when state changes
- **Visible region culling**: Skips off-screen lines/points
- **Color-run batching**: Reduces span count from thousands to ~30 per row
- **Max steps limit**: Bresenham capped at 10,000 steps per line

For Canvas migration, the `paint()` callback runs every frame — ensure map geometry is cached and only the viewport-relevant subset is drawn. The Canvas widget handles its own clipping, so explicit culling is less necessary but still beneficial for large zones (4,000+ line segments).
