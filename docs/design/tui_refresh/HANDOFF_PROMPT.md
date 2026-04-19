# Prompt for Claude Opus (Claude Code)

Copy everything below the line into Claude Code's prompt. The design bundle
is expected to live at `docs/design/tui_refresh/` in the repo — adjust the
path in the first line if you put it somewhere else.

---

I'm handing you a visual + structural refresh for the TextQuest TUI
(ratatui dashboard at `textquest/src/tui/`). The complete design bundle is
in this repo at **`docs/design/tui_refresh/`** and contains:

- `README.md` — the authoritative spec. Read this first, end to end.
- `mock/TextQuest TUI.html` — a single-file HTML mock. Open it in a
  browser to see the intended layout, color, density, and copy for every
  screen and overlay. Treat it like a Figma export: pixel-accurate
  reference, not code to port.
- `mock/src/` — the JSX fragments the HTML is assembled from. Reference
  only; do not port the JS.

## Scope

Refresh the seven screens + eight overlays the app already has. **No new
modules, no new theme, no new widget primitives.** Every layout change
must compose out of the helpers already in `ui/widgets.rs`
(`titled_block`, `kv_row`, `hp_bar`, `mana_bar`, `con_color`, etc.) and
colors already in `theme.rs::neriak()`.

## What to read in the Rust code first

1. `textquest/src/tui/theme.rs` — palette. `neriak()` is the target.
2. `textquest/src/tui/ui/widgets.rs` — helpers you must reuse.
3. `textquest/src/tui/ui/mod.rs` — header, status bar, overlay plumbing,
   responsive breakpoints.
4. `textquest/src/tui/ui/dashboard.rs` — the largest screen; sets the
   density/typography bar for everything else.
5. Each per-screen module under `ui/` that the README references.

## Important conventions from the spec

- **Themed tab labels.** The seven tabs are labeled `Soul Tethers`,
  `Cartography`, `Waypath`, `Oracle`, `Aethergram`, `Coinmark`,
  `Third Gate`. Header pills read
  `[1 Teth][2 Cart][3 Way][4 Ora][5 Aeth][6 Coin][7 Gate]`. Status-bar
  screen name is the full themed label uppercased. File names stay as
  they are (`dashboard.rs`, `tactical.rs`, etc.).
- **Known redundancies — DO NOT copy them from the mock.** The README has
  a section called "Known redundancies"; it lists four panels that appear
  duplicated in the mock and must be reduced in the implementation:
    - Coinmark's roster column set (cycle-only columns; no HP/class/zone)
    - Coinmark's ledger (economy-only deltas; no XP/kills/uptime)
    - Oracle's sidebar (structured key/value spawn inspector; no hex)
    - Soul Tethers' Combat + CH-Chain merged into one sidebar panel
- **No CRT effects in ratatui.** The mock has scanlines, phosphor glow,
  and a vignette. These are browser affordances only. Do not attempt
  terminal equivalents.
- **Data.** Every field in the mock maps onto an existing state struct
  listed under "State / data requirements" in the README. If a field has
  no backing yet (e.g. "top loot rollup"), add it to the owning state
  struct — do not stub in the renderer.
- **Keybinds.** Honor existing bindings in `input::KeyHandler`. If the
  mock advertises a key that's already bound to a different action, keep
  the existing binding and update the Help overlay copy instead.

## How I'd like you to work

1. Start by reading `README.md` end to end and opening the HTML mock.
2. Implement **Soul Tethers** first (it's the hub and sets the density
   bar). Get it reviewed before moving on.
3. Then **Cartography** (highest visual complexity — good early test of
   whether the theme survives porting).
4. Then the remaining five screens in any order.
5. Overlays last.

For each screen, before writing code, post a short plan:
- which modules you'll touch
- which fields (if any) need adding to existing state structs
- any places the spec conflicts with what's already in the code

Then implement, and on completion list the diff summary so I can review.

Out of scope: new state plumbing beyond what's in the README, new palette
variants, changes to `input::KeyHandler` (except Help copy), CRT effects.
