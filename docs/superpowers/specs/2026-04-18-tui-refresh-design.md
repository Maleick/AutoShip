# TUI Visual Refresh — Design Spec

## Overview

Conservative visual refresh of the ratatui TUI. Updates what each screen
**composes** from existing primitives — field choices, column order, emphasis,
density, box-drawing chrome — so the whole app reads like one instrument.

**Source of truth:** `design_handoff_tui_refresh/README.md` (bundled with
HTML mock at `design_handoff_tui_refresh/mock/TextQuest TUI.html`).

**Constraints:**

- No new modules — every pane maps onto an existing file
- No new theme — target palette is `Theme::neriak()` in `theme.rs`
- No new widget primitives — build from helpers in `ui/widgets.rs`
- No CRT scanline/phosphor/vignette emulation (browser-only affordance)
- No changes to `input::KeyHandler` bindings (except Help overlay copy)

## File Mapping (Spec → Actual)

| Spec Reference       | Actual File                            |
| -------------------- | -------------------------------------- |
| `ui/dashboard.rs`    | `tui/ui/dashboard.rs`                  |
| `ui/tactical.rs`     | `tui/ui/map.rs`                        |
| `ui/nav.rs`          | `tui/ui/navigation.rs`                 |
| `ui/debug.rs`        | `tui/ui/spawns.rs`                     |
| `ui/packets.rs`      | `tui/ui/packets.rs`                    |
| `ui/economy.rs`      | `tui/ui/economy_controls.rs`           |
| `ui/orchestrator.rs` | `tui/ui/orchestrator_panel.rs`         |
| `ui/help.rs`         | `tui/ui/mod.rs` (`draw_help_overlay`)  |
| `ui/alerts.rs`       | `tui/ui/mod.rs` (`draw_alert_overlay`) |
| `ui/command.rs`      | `tui/command.rs`                       |
| `ui/config.rs`       | `tui/config_panel.rs`                  |
| `ui/setup_wizard.rs` | `tui/wizard.rs`                        |
| `ui/menu.rs`         | `tui/menu.rs`                          |
| `ui/toast.rs`        | `tui/toast.rs`                         |

## Implementation Order

1. **Global chrome** — header + status bar (`ui/mod.rs`)
2. **Characters** — `dashboard.rs` (largest screen, sets density bar)
3. **Tactical** — `map.rs`
4. **Navigation** — `navigation.rs`
5. **Debug** — `spawns.rs`
6. **Packets** — `packets.rs`
7. **Economy** — `economy_controls.rs`
8. **Orchestrator** — `orchestrator_panel.rs`
9. **Overlays** — Help, Command, Config, CH-Chain, Alerts, Menu, Wizard, Toast

Each step is a self-contained unit. Compile-check after each screen.

## Global Chrome Changes

### Header (`ui/mod.rs::render_header` / `draw_header`)

Single rounded block, `border_primary`. Layout:

```
╭── TextQuest ─────────────────────────────────────────────────────────────╮
│ 6x EQ │ 1/6 Venkhadrei │ G2 Fear Core │ Bertoxxulous │ Plane of Fear  [tabs] │
╰──────────────────────────────────────────────────────────────────────────╯
```

- Client count: `ClientRegistry::len()` + literal `EQ` (cyan)
- Focused client: `{idx}/{total} {name}` (highlight)
- Focus group: `G{n} {label}` (cyan + secondary)
- Server: `text_server` (pink/magenta)
- Zone: `text_bright`
- Active tab: `row_selected_bg` bg + `text_bright` fg
- Inactive tab: `text_muted` brackets + `text_secondary` label

### Status Bar (`ui/mod.rs::render_status_bar` / `draw_status_bar`)

Dashed rule above. Left: `SCREEN_NAME ▸ keybind hints`. Right:

```
[HUNT] │ MA Thurgrek │ MT Thurgrek │ CH 3x@5.0s A │ [G2] │ Alerts 2 │ neriak
```

- Mode pill: inverse magenta (Hunt) / inverse cyan (Camp)
- MA/MT: `text_secondary` label + `text_bright` name
- CH: `{members}x@{interval}s` + green `A` (adaptive) or muted `·`
- Alerts: amber when unread > 0, muted otherwise
- Theme name: `text_muted`

## Screen Specifications

Full layout specs for each screen are in the handoff README sections
"### 1. Characters" through "### 7. Orchestrator". Key dimensions:

- Terminal width: 146 chars (responsive fallback to existing breakpoints)
- Sidebar width: 44 cols (Characters, Tactical, Debug, Packets, Economy, Orchestrator)
- Main width: 101 cols (146 - 44 - 1 gap)
- Navigation: full-width blocker panel + 2-col card grid (72 each)

## State Additions

Fields the mock surfaces that may not exist yet on state structs:

| Field                 | Owner Struct   | Type                                    |
| --------------------- | -------------- | --------------------------------------- |
| `top_loot` rollup     | `Session`      | `Vec<(String, u32)>`                    |
| `activity` glyph enum | `ClientState`  | `Activity` enum                         |
| `condition` enum      | `ClientState`  | `Condition` enum (Stable/Hurt/Critical) |
| `blocker_description` | `NavState`     | `Option<String>`                        |
| `retry_count`         | `NavState`     | `u8`                                    |
| `fallback_route`      | `NavState`     | `Option<String>`                        |
| `progress_pct`        | `NavState`     | `f32`                                   |
| `decoded_fields`      | `PacketEntry`  | `Vec<(String, String)>`                 |
| `ledger_day_totals`   | `Economy`      | struct                                  |
| `signal_feed`         | `Orchestrator` | `Vec<Signal>`                           |
| `phase_timeline`      | `Orchestrator` | `Vec<PhaseEntry>`                       |

Add these to the owning state struct, not stub in the renderer.

## Overlay Specifications

All overlays: modal floating panels at 96 cols, centered. Dim underlying
screen with existing overlay plumbing in `ui/mod.rs`.

Full content specs in handoff README sections "### Help overlay" through
"### Toast". Key overlays:

- **Help** (`?`): 4 sections (Navigation, Overlays, Combat/Ops, Roster/Debug)
- **Command** (`:`): suggestion list + input line with cursor
- **Config** (`F2`): tree with `▾`/`▸`/`·` glyphs
- **CH-Chain** (`F3`): summary + 3 slot lines with progress bars
- **Alert Feed** (`F8`): severity-colored dots + 2-line entries
- **Menu** (`F10`): top bar + dropdown with key shortcuts
- **Wizard** (`F4`): step 2 profile picker with radio buttons
- **Toast** (transient): 62-wide, green border, 2 lines

## Out of Scope

- New state plumbing beyond fields listed above
- New theme/palette variants
- Changes to keybind handlers (except Help copy)
- CRT scanline/phosphor/vignette emulation
- New responsive breakpoint logic (use existing)
