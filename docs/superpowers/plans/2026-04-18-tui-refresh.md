# TUI Visual Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the ratatui TUI to match the HTML mock in `design_handoff_tui_refresh/`, updating composition, density, and chrome across all 7 screens + 8 overlays.

**Architecture:** Conservative refresh — modify existing render functions to match the mock's layout, column order, field choices, and box-drawing chrome. No new modules, no new theme, no new widget primitives. State structs gain fields where the mock surfaces data not yet tracked.

**Tech Stack:** Rust, ratatui 0.29+, crossterm. Existing `Theme::neriak()` palette, existing `widgets.rs` helpers.

**Design spec:** `docs/superpowers/specs/2026-04-18-tui-refresh-design.md`
**HTML mock:** `design_handoff_tui_refresh/mock/TextQuest TUI.html`
**Mock source:** `design_handoff_tui_refresh/mock/src/` (per-screen JS files)

---

## File Map

| Task | File(s) Modified                                                                                                                                        | Mock Reference                    |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| T1   | `tui/ui/mod.rs` (lines 359-398, 704-776)                                                                                                                | `mock/src/shell.js`               |
| T2   | `tui/ui/dashboard.rs` (lines 32-619)                                                                                                                    | `mock/src/screen_overview.js`     |
| T3   | `tui/ui/map.rs` (lines 43-125, 1881-1911)                                                                                                               | `mock/src/screen_tactical.js`     |
| T4   | `tui/ui/navigation.rs` (lines 30-598)                                                                                                                   | `mock/src/screen_navigation.js`   |
| T5   | `tui/ui/spawns.rs`                                                                                                                                      | `mock/src/screen_debug.js`        |
| T6   | `tui/ui/packets.rs` (lines 13-195)                                                                                                                      | `mock/src/screen_packets.js`      |
| T7   | `tui/ui/economy_controls.rs` (lines 91-116)                                                                                                             | `mock/src/screen_economy.js`      |
| T8   | `tui/ui/orchestrator_panel.rs` (lines 323-349)                                                                                                          | `mock/src/screen_orchestrator.js` |
| T9   | `tui/ui/mod.rs` (help + alerts overlays), `tui/command.rs`, `tui/config_panel.rs`, `tui/ui/ch_chain.rs`, `tui/wizard.rs`, `tui/menu.rs`, `tui/toast.rs` | `mock/src/overlays.js`            |

---

## Task 1: Global Chrome — Header & Status Bar

**Files:**

- Modify: `textquest/src/tui/ui/mod.rs` (draw_header ~359-398, build_header_tabs, build_header_meta, draw_status_bar ~704-776, build_status_left, build_status_right)

**What changes (header):**
The header currently uses adaptive layout logic that collapses tabs when narrow. The mock specifies a fixed 3-line rounded panel:

- Line 1: `╭── TextQuest ─────...╮` (top border with title inset)
- Line 2: `│ 6x EQ │ 1/6 Name │ G2 Label │ Server │ Zone    [tabs] │`
- Line 3: `╰─────────────...────╯` (bottom border)

Key rendering changes in `build_header_meta()`:

- Client count: `{len}x` in cyan + `EQ` in secondary
- Focused client cursor: `{idx}/{total}` bright + selected name in highlight
- Focus group: `G{n}` cyan + group label secondary
- Server: `text_server` (pink)
- Zone: `text_bright`

Key rendering changes in `build_header_tabs()`:

- Active tab: `[` magenta + inverse-magenta `N Label` + `]` magenta
- Inactive tab: `[` muted + secondary `N Label` + `]` muted

**What changes (status bar):**
Currently: left = hints, right = stats with horizontal split.
Mock specifies a dashed rule `─` above, then single line:

- Left: `SCREEN_NAME ▸ keybind hints` (bright bold + muted arrow + secondary)
- Right: `[HUNT] │ MA name │ MT name │ CH 3x@5.0s A │ [G2] │ Alerts N │ neriak`

Key right-side elements:

- Mode pill: inverse-magenta `HUNT` or inverse-cyan `CAMP`
- MA/MT: secondary label + bright name
- CH: cyan label + bright stats + green/muted adaptive indicator
- Focus group: magenta brackets + inverse-magenta group ID
- Alerts: amber count when unread > 0, muted when 0
- Theme: muted theme name

- [ ] **Step 1:** Read `ui/mod.rs` lines 359-500 (header functions) and lines 700-780 (status bar functions)
- [ ] **Step 2:** Rewrite `draw_header()` to render 3-line rounded panel matching mock's `renderHeader()` in `shell.js`
- [ ] **Step 3:** Rewrite `build_header_meta()` to produce: `6x EQ │ 1/6 Name │ G2 Label │ Server │ Zone`
- [ ] **Step 4:** Rewrite `build_header_tabs()` to produce: `[1 Char][2 Map]...` with inverse-magenta active, muted inactive
- [ ] **Step 5:** Rewrite `draw_status_bar()` to render dashed rule + single status line
- [ ] **Step 6:** Rewrite `build_status_right()` to produce: mode pill, MA/MT, CH summary, focus group pill, alerts count, theme name
- [ ] **Step 7:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 8:** Commit: `feat(tui): refresh header and status bar chrome`

---

## Task 2: Characters Screen — Dashboard

**Files:**

- Modify: `textquest/src/tui/ui/dashboard.rs` (draw_dashboard, draw_dashboard_grid, draw_group_focus_strip, sidebar panels)

**What changes:**
The current dashboard has: roster table (marker, name, group, class, zone, HP%, condition, state) + collapsible sidebar sections (Character, Groups, Filters, Combat, Session, SlotProfile, Kills).

The mock specifies:

1. **Group Focus strip** (1 row, cyan border) — Uptime, Kills/Deaths, XP rates, Plat rates, Top Loot
2. **Ops Roster** — new columns: Lvl (3, right), HP with bar (16), Mana with bar (12), Cond (9), Activity glyph (10). Footer activity legend. Panel footer with keybind hints.
3. **Sidebar (44-wide):** Character detail (HP/Mana/End bars w18 + state/activity/condition/position), Target·Cast (target with type color, casting progress bar w22), Groups (per-group with conn/lead), Session (server/mode/uptime/kills/xp/plat), Combat (MA/MT/CH chain/last+next CH)

Layout: main pane = full width - 44 cols. Sidebar = 44.

- [ ] **Step 1:** Read full `dashboard.rs` to understand current rendering logic
- [ ] **Step 2:** Update `draw_group_focus_strip()` to match mock: cyan border, inline stats (uptime │ kills/deaths │ XP rates │ plat │ top loot)
- [ ] **Step 3:** Update `draw_dashboard_grid()` roster columns to match mock: cursor(2), Name(14), Grp(3), Cls(4), Lvl(3,right), Zone(22), HP(16 with bar), Mana(12 with bar), Cond(9), State(8), Activity(10)
- [ ] **Step 4:** Add activity legend footer and keybind footer to roster panel
- [ ] **Step 5:** Update sidebar to fixed 44-wide. Restructure sections: Character·Name (cyan border, HP/Mana/End bars, state/activity/condition/position), Target·Cast (target with type color + casting progress), Groups (per-group entries), Session (server/mode/uptime/kills/xp/plat), Combat (MA/MT/CH chain)
- [ ] **Step 6:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 7:** Commit: `feat(tui): refresh Characters screen layout and density`

---

## Task 3: Tactical Screen — Map

**Files:**

- Modify: `textquest/src/tui/ui/map.rs` (draw_map_screen, draw_tactical_sidebar)

**What changes:**
Current map has complex adaptive layout with 6+ responsive modes, minimap, navmesh overlay, Z-level filter, etc. The mock specifies:

Main pane = full ASCII zone map with spawn markers:

- Named: `◆` in `text_highlight` (magenta)
- NPC: `○` in `text_normal`
- Corpse: `†` in `text_muted`
- PC: first letter of class, bold, class-colored (CLR cyan, WAR amber, MAG magenta, MNK red)
- Terrain: dim `·` field with `▒` ridges
- Footer: grid coordinates + legend

Sidebar (44-wide):

1. **Spawn List** — grouped by Named/NPCs/Corpses with HP coloring
2. **Target · Main Assist** (cyan border) — target name/HP, assisting/tanked by/aggro
3. **CH Chain · Active** — members, adaptive, target, 3 slot lines with cyan progress bars

The existing map rendering is very sophisticated (1143 lines). The refresh should preserve the core map algorithm but update the sidebar panels and spawn marker glyphs to match the mock.

- [ ] **Step 1:** Read `map.rs` spawn marker rendering and sidebar functions
- [ ] **Step 2:** Update spawn markers to use mock glyphs: `◆` Named, `○` NPC, `†` Corpse, class letter for PCs
- [ ] **Step 3:** Add 3-line caption below map grid (coordinates + legend)
- [ ] **Step 4:** Restructure sidebar: Spawn List (grouped by type, HP-colored), Target·MA (cyan border, assisting/tanked/aggro), CH Chain·Active (slots with progress bars)
- [ ] **Step 5:** Update panel title format: `Tactical · {zone}` with keybind footer
- [ ] **Step 6:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 7:** Commit: `feat(tui): refresh Tactical screen sidebar and spawn markers`

---

## Task 4: Navigation Screen

**Files:**

- Modify: `textquest/src/tui/ui/navigation.rs` (draw_navigation_screen)

**What changes:**
Current: dual-panel (nav status table left, commands right).
Mock: full-width **Nav Blockers** panel (red border) on top, then 2-column grid of per-client nav cards.

Blocker panel: `N blocker · N stuck · N routing nominal` header, then per-blocker entries with ⚠ marker, blocker description, last move, fallback, resolution options.

Per-client card (border=red when stuck, else magenta):

```
Class    Warrior  L65   Group G2
Zone     Plane of Fear
Position 820.1, -1418.7, 3.1  h94

Status   Holding   ETA at anchor
Destination fear.mt_spot
Route    mesh/37wp

Progress ████████████████·············· 42%
```

Stuck clients show red blocker callout instead of progress bar.

- [ ] **Step 1:** Read full `navigation.rs`
- [ ] **Step 2:** Add `draw_blocker_panel()` — full-width red-bordered panel with blocker summary
- [ ] **Step 3:** Add `draw_nav_card()` — per-client card with class/zone/position, status/dest/route, progress bar or blocker callout
- [ ] **Step 4:** Rewrite `draw_navigation_screen()` — blocker panel on top, then 2-column card grid
- [ ] **Step 5:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 6:** Commit: `feat(tui): refresh Navigation screen with blocker panel and client cards`

---

## Task 5: Debug Screen — Spawns

**Files:**

- Modify: `textquest/src/tui/ui/spawns.rs` (draw_debug_screen, draw_spawn_list, draw_hex_panel)

**What changes:**
Current: spawn list with columns (marker, name, ID, type, race, level, HP%, con, class).
Mock specifies main+44-col sidebar layout:

Main columns: cursor(2), ID(7,right), Name(26), Type(7), Cls(4), Lvl(4,right), HP%(5,right), Y(8,right), X(8,right), Z(6,right), State(6).
Filter line below table: `filter: type=* · hp>0 · showing 9 / 47`
Footer: `↑↓ select · / filter · t target · i inspect · d dump`

Sidebar = **Inspect · hex dump** (cyan border): struct field display + raw hex bytes with alternating cyan/bright/magenta coloring, amber ASCII gutter.

- [ ] **Step 1:** Read full `spawns.rs` draw_debug_screen and draw_hex_panel
- [ ] **Step 2:** Update spawn table columns to match mock (ID, Name, Type, Cls, Lvl, HP%, Y, X, Z, State)
- [ ] **Step 3:** Add filter echo line and keybind footer
- [ ] **Step 4:** Update hex dump panel to match mock: struct field display + colored hex rows
- [ ] **Step 5:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 6:** Commit: `feat(tui): refresh Debug screen columns and hex dump`

---

## Task 6: Packets Screen

**Files:**

- Modify: `textquest/src/tui/ui/packets.rs` (draw_packet_monitor)

**What changes:**
Current: packet log table (left) + stats sidebar (right). Columns: Time, Dir (→/←), Opcode (hex), Size, Client.
Mock specifies main+44-col sidebar:

Main columns: cursor(2), Time(13), Dir(4, S→C/C→S), Opcode(22), Size(5,right), Payload(50, first 16 bytes hex).
Direction colors: S→C cyan, C→S green.
Opcode colors: HP/Mana amber, BeginCast/MemorizeSpell magenta, Damage red, else bright.
Filter line: `filter: op=* · dir=any · client=* · ▶ live (N rows · N since HH:MM)`
Footer: `↑↓ select · / filter · p pause · space mark · e export`

Sidebar = **Packet · detail** (cyan border): decoded fields from opcode schema + hex dump.

- [ ] **Step 1:** Read full `packets.rs`
- [ ] **Step 2:** Update packet table columns: add Payload column, change Dir to S→C/C→S format, add opcode color coding
- [ ] **Step 3:** Add filter echo line and keybind footer
- [ ] **Step 4:** Restructure sidebar as Packet·detail: decoded fields + hex row
- [ ] **Step 5:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 6:** Commit: `feat(tui): refresh Packets screen layout and detail pane`

---

## Task 7: Economy Screen

**Files:**

- Modify: `textquest/src/tui/ui/economy_controls.rs` (draw_economy_screen)

**What changes:**
Current: 4 stacked panels (vendor cycle, banking, session tracker, loot queue) left + controls right.
Mock specifies two-row main + 44-col sidebar:

Main top = **Vendor / Bank Cycle**: Cycle State, Current Slot, Stage/Cycle Started, Cycles Today, Next Cycle In, Vendor, Bank, Plat pocket/banked. Footer: `s start · S stop · x skip client · r reload rules`

Main bottom = **Roster**: Slot(16), Status(10), Plat(6,right), Bags(6), Reason/Notes(34). Status coloring: Active=amber inverse, Done=green, Queued=cyan, Skipped=muted.

Sidebar top = **Rules** (cyan border): active rule file + numbered rules + hints.
Sidebar bottom = **Ledger**: day + last-cycle totals.

- [ ] **Step 1:** Read full `economy_controls.rs`
- [ ] **Step 2:** Restructure main panels: Vendor/Bank Cycle (KV layout) + Roster table
- [ ] **Step 3:** Add sidebar: Rules panel (cyan) + Ledger panel
- [ ] **Step 4:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 5:** Commit: `feat(tui): refresh Economy screen with roster and ledger`

---

## Task 8: Orchestrator Screen

**Files:**

- Modify: `textquest/src/tui/ui/orchestrator_panel.rs` (draw_orchestrator_screen)

**What changes:**
Current: tabbed interface (Session, Group, Navigation, Economy, Combat, System) with complex per-tab rendering.
Mock specifies simplified layout — main top+bottom, sidebar right:

Main top = **Fleet Orchestrator**: Active Intent, Phase (green=Execute, cyan=other), Cadence. Footer: `p pause · r resume · A abort intent · enter drill into slot`

Main bottom = **Slots · N live · N configured · N blocked**: Slot(5), Name(14), State(12), FSM(14), Lat(6,right), Health(22, capped bar), Profile(26). Health bar: Live=18 green blocks+OK, Recovering=11 amber+7 dots+RCV, Blocked=2 red+16 dots+BLK, Configured=18 dots+OFF.

Sidebar top = **Signal Feed** (cyan): `{time} {KIND} {message}` with kind-colored labels.
Sidebar bottom = **Phase Timeline**: `{HH:MM} {Phase} ── {note}` with green for active Execute phase.

- [ ] **Step 1:** Read full `orchestrator_panel.rs`
- [ ] **Step 2:** Replace tabbed layout with simplified: Fleet Orchestrator header + Slots table
- [ ] **Step 3:** Add Signal Feed and Phase Timeline sidebar panels
- [ ] **Step 4:** Implement health bar rendering (18-char bar with state-colored fills)
- [ ] **Step 5:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 6:** Commit: `feat(tui): refresh Orchestrator screen with fleet view`

---

## Task 9: Overlays Refresh

**Files:**

- Modify: `tui/ui/mod.rs` (draw_help_overlay, draw_alert_overlay)
- Modify: `tui/command.rs` (command palette rendering)
- Modify: `tui/config_panel.rs` (ConfigPanelWidget)
- Modify: `tui/ui/ch_chain.rs` (ChChainWidget)
- Modify: `tui/wizard.rs` (WizardWidget)
- Modify: `tui/menu.rs` (MenuBar, MenuDropdown)
- Modify: `tui/toast.rs` (Toast rendering)

**What changes:**
All overlays: modal floating panels at **96 cols**, centered, with dimmed underlying screen.

1. **Help** (`?`): 4 sections (Navigation, Overlays, Combat/Ops, Roster/Debug). Each row = inverse-cyan key pill + description.
2. **Command** (`:`): suggestion list + input line with `█` cursor in `border_primary`.
3. **Config** (`F2`): tree with `▾`/`▸`/`·` glyphs, leaf format `{key} = {value}`.
4. **CH-Chain** (`F3`): summary + slot schedule with `✓`/`◷`/queued states.
5. **Alert Feed** (`F8`): severity dots (●=unread, ○=ack), 2-line entries.
6. **Menu** (`F10`): top bar + dropdown with shortcut column, `─` dividers.
7. **Wizard** (`F4`): step-based with `◉`/`○` radio buttons, inverse button pills.
8. **Toast**: 62-wide, green border, 2-line content.

- [ ] **Step 1:** Update `draw_help_overlay()` — 4 sections, inverse-cyan key pills, 96-wide
- [ ] **Step 2:** Update command palette rendering — suggestion list + cursor
- [ ] **Step 3:** Update `ConfigPanelWidget` — tree glyphs `▾`/`▸`/`·`, 96-wide
- [ ] **Step 4:** Update `ChChainWidget` — slot schedule with progress + status glyphs
- [ ] **Step 5:** Update `draw_alert_overlay()` — severity dots, 2-line entries, 96-wide
- [ ] **Step 6:** Update `MenuBar`/`MenuDropdown` — top bar + dropdown + shortcut column
- [ ] **Step 7:** Update `WizardWidget` — radio buttons `◉`/`○`, inverse button pills
- [ ] **Step 8:** Update Toast rendering — 62-wide, green border, 2-line
- [ ] **Step 9:** Run `cargo check -p textquest` — fix compilation errors
- [ ] **Step 10:** Commit: `feat(tui): refresh all overlay panels`

---

## Task 10: State Additions & Final Polish

**Files:**

- Modify: `tui/app.rs` or relevant state structs
- Modify: `tui/state.rs`

**What changes:**
Add fields the mock surfaces that don't exist yet:

- `Session` or `App`: `top_loot: Vec<(String, u32)>`
- `ClientState`: `activity: Activity` enum (Nav, Arr, Stk, Ded, FD, Sit, Lot, Cst, Fgt, Rdy)
- `ClientState`: `condition: Condition` enum (Stable, Hurt, Critical)
- `NavState` per-client: `blocker_description: Option<String>`, `retry_count: u8`, `fallback_route: Option<String>`, `progress_pct: f32`
- `PacketEntry`: `decoded_fields: Vec<(String, String)>`
- `Economy`: ledger day totals struct
- `Orchestrator`: `signal_feed: Vec<Signal>`, `phase_timeline: Vec<PhaseEntry>`

- [ ] **Step 1:** Audit all state structs against mock fields — identify which already exist vs need adding
- [ ] **Step 2:** Add missing enums (Activity, Condition) and state fields
- [ ] **Step 3:** Wire new fields into demo data generation (`demo_data.rs`)
- [ ] **Step 4:** Run `cargo check -p textquest` — fix all compilation errors
- [ ] **Step 5:** Run `cargo clippy -p textquest` — fix warnings
- [ ] **Step 6:** Commit: `feat(tui): add state fields for refreshed screen data`
