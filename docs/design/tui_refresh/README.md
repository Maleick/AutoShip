# Handoff: TextQuest TUI Refresh

## Overview

This bundle specifies a visual and structural refresh of the TextQuest TUI —
the ratatui-based dashboard at `textquest/src/tui/`. The goal is a
Neriak-themed, CRT-styled, information-dense fleet console that spans the
seven screens already present in the app plus the existing overlay surfaces
(Help, Command, Config, CH-Chain, Alert Feed, Menu Bar, Fleet Setup Wizard,
Toast).

## Themed tab names (user-facing)

Tabs are labeled with themed names in the header pills and status bar, but
the underlying Rust modules keep their existing file names. Map:

| #   | Themed label (UI) | Module (code)                  | Purpose                                      |
| --- | ----------------- | ------------------------------ | -------------------------------------------- |
| 1   | **Soul Tethers**  | `ui/dashboard.rs` (Characters) | Roster, selected client, groups, session     |
| 2   | **Cartography**   | `ui/tactical.rs` (Tactical)    | Zone map + spawn list + target + CH chain    |
| 3   | **Waypath**       | `ui/nav.rs` (Navigation)       | Route cards, blockers                        |
| 4   | **Oracle**        | `ui/debug.rs` (Debug)          | Spawn table + inspect/hex dump               |
| 5   | **Aethergram**    | `ui/packets.rs` (Packets)      | Packet stream + detail                       |
| 6   | **Coinmark**      | `ui/economy.rs` (Economy)      | Vendor cycle, roster, rules, ledger          |
| 7   | **Third Gate**    | `ui/orchestrator.rs`           | Fleet orchestrator, slots, signals, timeline |

The tab pills in the header read `[1 Teth][2 Cart][3 Way][4 Orc*][5 Aeth][6 Coin][7 Gate]`.
`Orc*` is the Oracle tab — the abbreviation is ambiguous with Orchestrator,
so use `Ora` instead: `[1 Teth][2 Cart][3 Way][4 Ora][5 Aeth][6 Coin][7 Gate]`.

Screen-name slot in the status bar uses the full themed name uppercased:
`SOUL TETHERS ▸ …`, `CARTOGRAPHY ▸ …`, etc.

## Known redundancies (fix during implementation — DO NOT re-implement as shown)

The HTML mock carries some overlap between panels. Treat the mock as the
visual direction but **remove these duplications** when you port to Rust:

1. **Roster appears on both Soul Tethers and Coinmark.** Keep the full
   `Ops Roster` on Soul Tethers only. On Coinmark, the "Roster" panel should
   show economy-specific columns only: `Slot · Status · Plat · Bags · Notes` —
   no HP bars, no class, no zone. It's a cycle-progress view, not a roster.
2. **Session stats appear on both Soul Tethers and Coinmark.** Session XP,
   kills, plat/h, uptime live on Soul Tethers only. Coinmark's `Ledger`
   sidebar shows economy-only deltas (platinum earned this session, items
   sold, cycles completed) — nothing else.
3. **Hex dump appears on both Oracle and Aethergram.** Hex is a wire-level
   concern; keep it on **Aethergram only**. Oracle's sidebar should be a
   _structured_ inspector — decoded spawn struct fields (entity_id, type,
   class, level, pos, flags) rendered as key/value rows, not bytes.
4. **Combat + CH Chain on Soul Tethers are two panels about the same thing.**
   Merge into one sidebar panel titled `Combat · CH Chain` with MA/MT on
   top and the chain rows below.

These are intentional reductions. The mock predates this cleanup; the
README is authoritative.

The refresh is intentionally conservative:

- **No new modules.** Every pane maps onto an existing file under
  `textquest/src/tui/ui/` or its companion state modules.
- **No new theme.** The target palette is `Theme::neriak()` already defined
  in `textquest/src/tui/theme.rs`.
- **No new widget primitives.** Layouts build from the helpers already in
  `textquest/src/tui/ui/widgets.rs` (`titled_block`, `kv_row`, `hp_bar`,
  `mana_bar`, `con_color`, etc.).

What changes is what each screen **composes** out of those parts — field
choices, column order, emphasis, density, and a consistent vocabulary of
box-drawing chrome so the whole app reads like one instrument.

## About the design files

The files in `mock/` are a **design reference** produced in HTML. They are
not production code and must not be shipped or linked into the Rust build.
Treat them the way you would treat a Figma export: a pixel-accurate
description of intended layout, color, and density for ratatui to imitate.

The implementation task is to reproduce each mocked screen inside the
existing ratatui renderer, using the existing `Theme` struct and existing
widget helpers. Where a mock shows a column or panel the Rust code does not
currently render, add the field to the corresponding screen module — do
**not** introduce a parallel widget system.

## Fidelity

**High-fidelity.** Colors, column widths, box-drawing styles, glyphs, and
copy in the mock are final. Match them when reasonable; deviate only when
a ratatui constraint forces it (e.g. terminal can't render a 146-column
layout at the user's current size — fall back to the existing responsive
breakpoints in `ui/mod.rs`).

## Target palette — `Theme::neriak()`

All values come verbatim from `textquest/src/tui/theme.rs::neriak`. Do not
introduce new constants; reference the theme.

| Role                 | Theme field                     | RGB                               |
| -------------------- | ------------------------------- | --------------------------------- |
| Background (panel)   | `panel_bg`                      | `#0d0618`                         |
| Background (row sel) | `row_selected_bg`               | `#1a0a2e`                         |
| Primary border       | `border_primary`                | `#cc44ff`                         |
| Active border        | `border_active`                 | `#00e5ff`                         |
| Warn border          | `border_warn`                   | `#fbbf24`                         |
| Danger border        | `border_danger`                 | `#ef4444`                         |
| Server emphasis      | `border_server`                 | `#ff00ff`                         |
| Bright text          | `text_bright` / `text_normal`   | `#e2d7f4`                         |
| Secondary text       | `text_secondary`                | `rgb(160,150,180)`                |
| Muted / border dim   | `text_muted` / `border_dim`     | `rgb(80,60,110)`                  |
| Accent               | `text_accent` (cyan)            | `#00e5ff`                         |
| Highlight            | `text_highlight` (magenta)      | `#cc44ff`                         |
| HP high / mid / low  | `hp_high` / `hp_mid` / `hp_low` | `#34d399` / `#fbbf24` / `#ef4444` |
| Mana                 | `mana`                          | `#60a5fa`                         |
| Con colors           | `con_red` … `con_green`         | match theme                       |

The scanline / phosphor-glow / vignette in the HTML mock are CRT affordances
only. They have no ratatui analog — do not attempt to emulate them.

## Global chrome

Above every screen: the header and status bar.

### Header (`ui/mod.rs::render_header`)

Single rounded block, `border_primary`. Title inset: `TextQuest`
(bright+bold). Middle row left→right:

```
 6x EQ │ 1/6 Venkhadrei │ G2 Fear Core │ Bertoxxulous │ Plane of Fear      [1 Teth][2 Cart][3 Way][4 Ora][5 Aeth][6 Coin][7 Gate]
```

- Client count is `ClientRegistry::len()` followed by the literal string `EQ`
  (cyan).
- `{idx}/{total}` is the focused client cursor.
- Focused-group label is `"G{n} {name}"` from `GroupDirectory`.
- Server name uses `text_server` (pink).
- Active tab pill uses `row_selected_bg` background + `text_bright` foreground.
  Inactive tabs use `text_muted` brackets + `text_secondary` label.

### Status bar (`ui/mod.rs::render_status_bar`)

Single row preceded by a dashed rule. Left: `SCREEN_NAME ▸ keybind hints`
where `SCREEN_NAME` is the themed name uppercased (`SOUL TETHERS`,
`CARTOGRAPHY`, `WAYPATH`, `ORACLE`, `AETHERGRAM`, `COINMARK`, `THIRD GATE`).
Right, delimited by `│`:

```
 [HUNT] │ MA Thurgrek │ MT Thurgrek │ CH 3x@5.0s A │ [G2] │ Alerts 2 │ neriak
```

- `HUNT` / `CAMP` is an inverse pill (magenta for hunt, cyan for camp).
- `MA` / `MT` labels in `text_secondary`, names in `text_bright`.
- `CH` is `{members}x@{interval}s` followed by green `A` when adaptive,
  muted `·` otherwise.
- `Alerts N` is amber when unread > 0, muted when zero.
- Final `neriak` is the active theme name in `text_muted`.

## Screens

Each section below lists the panels in reading order and the ratatui module
that owns them.

### 1. Soul Tethers (Characters) — `ui/dashboard.rs`

**Layout.** Two columns. Main pane = full width minus 44 cols. Sidebar = 44.

Main pane:

1. **Group Focus strip** — one-row block, `border_active` cyan.
   Title: `Group Focus · G2 Fear Core`. Content row:
   ```
   Uptime 03:47:12 │ Kills 148 / Deaths 2 │ XP 1.84M · +489k/h · +512k/15m │ Plat 384.6 · +101.4/h │ Top Loot Diamond×4 · Platinum Bar×3 · Fiery Avenger×1
   ```
2. **Ops Roster** — `border_primary`, title `Ops Roster · {n} clients · sorted by Group`.
   Columns, gap 1 space:

   | Col      | Width | Align | Source                                  |
   | -------- | ----- | ----- | --------------------------------------- |
   | cursor ▶ | 2     | left  | selection                               |
   | Name     | 14    | left  | `client.name`                           |
   | Grp      | 3     | left  | `client.group_label()` (G1/G2)          |
   | Cls      | 4     | left  | `client.class_abbr()`                   |
   | Lvl      | 3     | right | `client.level`                          |
   | Zone     | 22    | left  | `client.zone_short()`                   |
   | HP       | 16    | left  | `"{hp:>3}% " + hp_bar(hp, 10)`          |
   | Mana     | 12    | left  | melee shows `<muted>  -- </muted>`      |
   | Cond     | 9     | left  | Stable/Hurt/Critical, color by severity |
   | State    | 8     | left  | Stand/Sit/FD                            |
   | Activity | 10    | left  | glyph + tag (see legend below)          |

   Selected row uses `row_selected_bg` + bold. Last two lines of the panel
   are an empty spacer and an activity legend:

   ```
   Activity glyphs:  ➜ Nav  ✓ Arr  ! Stk  ☠ Ded  ⇣ FD   ☾ Sit  ⌕ Lot  ✦ Cst  ⚔ Fgt  ● Rdy
   ```

   Footer: `↑↓ select  ·  Enter focus  ·  g cycle group  ·  [ ] prev/next client`.

Sidebar panels (top to bottom), all 44-wide:

1. **Character · {name}** (`border_active` cyan) — HP / Mana / Endurance
   bars at width 18, then a block:
   ```
   State     Stand
   Activity  ⚔ Fight
   Condition Hurt
   Position  820.1, -1418.7, 3.1 h94
   ```
2. **Target · Cast** (`border_primary`) — Target name with type color
   (`text_highlight` for Named, green for PC, bright for NPC). Target HP bar
   width 18. Blank line. If casting: label + gem slot, cyan progress bar
   width 22, remaining seconds (append muted `±` when `exact=false`).
3. **Groups** — one entry per `GroupDirectory` entry:
   ```
   [G2]  Fear Core            ← inverse magenta when active
         Zone Plane of Fear
         Conn 4/4   Lead Sylunariel
   ```
4. **Session** — uptime, mode pill, kills/deaths, XP totals + rates, plat.
5. **Combat** — MA / MT names (highlight), CH chain summary (members,
   adaptive state, chain target), last CH id + time, next CH id + countdown.

### 2. Cartography (Tactical) — `ui/tactical.rs`

**Layout.** Main pane left, sidebar right (44).

Main pane = single panel, title `Tactical · Plane of Fear`, footer
`hjkl pan  ·  +/- zoom  ·  f center on focus  ·  t target under cursor`.

Content: an ASCII zone map of the current zone, plotted at
`width = MAIN-4` cells × `height = 22` rows. Terrain renders as a dim `·`
field with a few `▒` ridges. Spawn markers:

- Named — `◆` in `text_highlight`
- NPC — `○` in `text_normal`
- Corpse — `†` in `text_muted`
- PC — first letter of class, bold, colored by class (CLR cyan, WAR amber,
  MAG magenta, MNK red, ENC/NEC bright)

Below the grid: a 3-line caption:

```
grid: {zone} · y{min}..{max}, x{min}..{max} · resolution 1 cell ≈ N.Nu
Legend:  ◆ Named  ·  ○ NPC  ·  † Corpse  ·  CWMK clients
```

Sidebar:

1. **Spawn List** — title `Spawn List · {visible}/{total} filtered`. Grouped
   by `Named` (spawn-named color), `NPCs` (normal), `Corpses` (muted).
   Rows: `  {name:<24} L{lvl:>2} {hp:>3}%`. HP colored by band; corpses use
   `---` in muted.
2. **Target · Main Assist** (`border_active` cyan) — name, type/level/class
   subtitle, HP bar width 24, then:
   ```
   Assisting  Venkhadrei
   Tanked by  Thurgrek
   On tank    100% agg  · no add
   ```
3. **CH Chain · Active** — members line, adaptive flag, chain target, then
   three slot lines:
   ```
   Slot 1  Sylunariel   T+0.0s  ██████·····
   ```
   Each slot uses a cyan progress bar width 12. Offset is
   `interval * (idx-1) / members`.

### 3. Waypath (Navigation) — `ui/nav.rs`

**Layout.** Full-width **Nav Blockers** panel (`border_danger` red) on top,
then a 2-column grid of per-client cards (width ≈ `(TOTAL-2)/2` each).

Blocker panel copy template:

```
1 blocker · 1 stuck · 5 routing nominal

⚠ {name}  slot {slot} · {zone}
    blocker    obstacle: corpse pile @ ({y},{x})
    last move  3.2s ago   retries 3/5
    fallback   mesh/fallback → fear.zone_in
    resolution options: :nav unstick · :nav reroute · :nav force_tp
```

Per-client card (border color = `border_danger` when stuck, else
`border_primary`):

```
Class    Warrior  L65   Group G2
Zone     Plane of Fear
Position 820.1, -1418.7, 3.1  h94

Status   Holding   ETA at anchor
Destination fear.mt_spot
Route    mesh/37wp

Progress ████████████████·············· 42%
```

For stuck clients replace the progress line with the red blocker callout:

```
⚠ Blocker: obstacle: corpse pile @ (821,-1420)
  retries 3/5 · fallback route queued · :nav unstick
```

### 4. Oracle (Debug) — `ui/debug.rs`

**Layout.** Main pane + 44-col sidebar.

Main pane = **Spawns · zone {zone} · {n} entities**, title in
`border_primary`. Columns, gap 1 space:

| Col    | Width | Align                |
| ------ | ----- | -------------------- |
| cursor | 2     | left                 |
| ID     | 7     | right                |
| Name   | 26    | left                 |
| Type   | 7     | left (color by type) |
| Cls    | 4     | left (highlight)     |
| Lvl    | 4     | right                |
| HP%    | 5     | right                |
| Y      | 8     | right                |
| X      | 8     | right                |
| Z      | 6     | right                |
| State  | 6     | left                 |

Footer line below the table:
`filter: type=*  · hp>0 · showing 9 / 47`.

Panel footer:
`↑↓ select  ·  / filter  ·  t target  ·  i inspect  ·  d dump`.

Sidebar = **Inspect · hex dump** (`border_active` cyan). Template:

```
inspect <name> (id N, 0xNN)

struct Spawn {
  entity_id   = N
  name        = "…"
  type        = SpawnType::Named
  class_id    = N  /* CLS */
  level       = N
  hp_pct      = N
  pos         = (y, x, z)
  heading     = N
  flags       = NAMED | AGGRO | SEE_INVIS
}

raw bytes (first 64)
0000  f5 47 00 00 43 61 7a 69 63 2d 54 68 75 6c 65 00  │ .G..Cazic-Thule.
0010  ...
```

The hex rows use four 4-byte groups colored alternately
cyan / bright / bright / magenta; the ASCII gutter is amber.

### 5. Aethergram (Packets) — `ui/packets.rs`

**Layout.** Main + sidebar (44).

Main = **Packet Stream · {captured} captured · {peak}/s peak**. Columns:

| Col                      | Width     |
| ------------------------ | --------- |
| cursor                   | 2         |
| Time                     | 13        |
| Dir (S→C/C→S)            | 4         |
| Opcode                   | 22        |
| Size                     | 5 (right) |
| Payload (first 16 bytes) | 50        |

Direction color: `S→C` cyan, `C→S` green. Opcode color:
`OP_HPUpdate`/`OP_ManaUpdate` amber; `OP_BeginCast` / `OP_MemorizeSpell`
magenta; `OP_Damage` red; else bright. Below the table a filter echo line:
`filter: op=* · dir=any · client=* · ▶ live  (12 rows · 1,248 since 11:42)`.

Footer: `↑↓ select · / filter · p pause · space mark · e export`.

Sidebar = **Packet · detail** — decoded fields from the current opcode's
schema, blank line, `hex`, then one hex row. Opcode (and its numeric id in
parentheses) uses `text_highlight`.

### 6. Coinmark (Economy) — `ui/economy.rs`

**Layout.** Two-row main column, 44-col sidebar on the right.

Main top panel — **Vendor / Bank Cycle**:

```
Cycle State    Active · Selling
Current Slot   Izzlewink
Stage Started  12:01:03
Cycle Started  11:42:18
Cycles Today   4
Next Cycle In  22:41

Vendor  Sarsk the Provisioner
Bank    The Bank of Bertoxxulous

Plat (pocket)  384.6
Plat (banked)  12,480
```

Footer: `s start · S stop · x skip client · r reload rules`.

Main bottom panel — **Roster**:

| Col            | Width     |
| -------------- | --------- |
| Slot           | 16        |
| Status         | 10        |
| Plat           | 6 (right) |
| Bags           | 6         |
| Reason / Notes | 34        |

Status coloring: `Active` amber + inverse on the slot cell; `Done` green;
`Queued` cyan; `Skipped` muted. Notes templates:

- skipped → `skip: <reason>` (amber)
- done → `cycle complete · bagged 3 lore items`
- active → `selling 14 items · 20% through`
- queued → `waiting in queue`

Sidebar top — **Rules** (`border_active`). Shows active rule file name in
the first line, then numbered rules from the current ruleset, then a
trailing hint block:

```
rules load from ~/.config/textquest/economy/
press e to edit · r to reload · t to test
```

Sidebar bottom — **Ledger** — day + last-cycle totals.

### 7. Third Gate (Orchestrator) — `ui/orchestrator.rs`

**Layout.** Main top+bottom; sidebar right.

Main top — **Fleet Orchestrator**:

```
Active Intent   fear.ch_chain.alpha
Phase           Execute   ← green when Execute, cyan otherwise
Cadence         2.0 Hz    tick 12:04:17.980
```

Footer: `p pause · r resume · A abort intent · enter drill into slot`.

Main bottom — **Slots · 6 live · 1 configured · 1 blocked**. Columns:

| Col     | Width     |
| ------- | --------- |
| Slot    | 5         |
| Name    | 14        |
| State   | 12        |
| FSM     | 14        |
| Lat     | 6 (right) |
| Health  | 22        |
| Profile | 26        |

State colors: `Live` green, `Recovering` amber, `Blocked` red,
`Configured` cyan, anything else muted. Health column is a capped 18-char
bar drawn with `█` + `·`, followed by one of `OK` / `RCV` / `BLK` / `OFF`:

- Live → 18 green blocks + `OK`
- Recovering → 11 amber blocks + 7 dots + `RCV`
- Blocked → 2 red blocks + 16 dots + `BLK`
- Configured/Blank → 18 dots + `OFF`

Sidebar top — **Signal Feed**. Each line:
`{time}  {KIND:<5}  {message}`. Kind colors: CH cyan, NAV amber, ALERT red,
XP green, LOOT magenta, CAST cyan, default bright.

Sidebar bottom — **Phase Timeline** — one row per phase transition:
`{HH:MM}  {Phase}  ── {note}`. Active phase line uses `Execute` in green.

## Overlays

All overlays are modal floating panels rendered at width **96 cols**,
centered on the current screen. Dim the underlying screen with
`Color::Black`-tinted `Style::default().fg(muted)` — use existing overlay
plumbing in `ui/mod.rs`; do not reinvent it.

| Key       | Overlay                                     | Border  | Source module        |
| --------- | ------------------------------------------- | ------- | -------------------- |
| `?`       | **Help · Keybinds**                         | cyan    | `ui/help.rs`         |
| `:`       | **Command mode**                            | magenta | `ui/command.rs`      |
| `F2`      | **Config · ~/.config/textquest/config.ron** | cyan    | `ui/config.rs`       |
| `F3`      | **CH Chain · panel**                        | magenta | `ui/ch_chain.rs`     |
| `F4`      | **Fleet Setup · wizard**                    | magenta | `ui/setup_wizard.rs` |
| `F8`      | **Alert Feed · N unread**                   | amber   | `ui/alerts.rs`       |
| `F10`     | **Menu · F10**                              | cyan    | `ui/menu.rs`         |
| transient | **Toast**                                   | green   | `ui/toast.rs`        |

### Help overlay content structure

Four sections — `Navigation`, `Overlays`, `Combat / Ops`,
`Roster / Debug`. Each row = inverse-cyan key pill + description. See
`mock/` for exact key→description pairs.

### Command overlay content structure

Lines:

```
suggestions
  :assist         assist the main assist
  :pull [target]  send puller to target or selected
  :ch start|stop|adaptive on|off
  :mode camp|hunt
  :nav <dest>     move all / focus group to named location
  :vendor run|skip|abort

: nav fear.ch_anchor focus=G2█
```

The `█` is the active cursor; rendered in `border_primary` color.

### Config overlay

Tree, two-space indent per depth. Glyphs:

- `▾` = expanded branch
- `▸` = leaf with non-default override (colored by severity)
- `·` = normal leaf

Leaf format: `{key} = {value}` (value in `text_bright`).
Footer: `↑↓ navigate · enter edit · s save · r reload`.

### CH-Chain overlay

Summary header + three cleric slots with T-offsets and status
(`✓ {time}` green, `◷ Ns out` amber, `queued` muted).

### Alert Feed overlay

Rows of two lines:

```
● CRITICAL 11:38:47  Slot S08 login_fail — account throttled by server
  kind=ClientLogin source=orch/S08
```

Severity dot color: Critical red, Warning amber, Info cyan. Acknowledged
rows render the dot as an empty muted `○`.

### Menu bar (F10)

Top row: menu titles separated by spaces; the active one uses
`row_selected_bg + text_bright`. Under the active title a dropdown:

```
  a  Assist Main Assist                                         :assist
  p  Pull Target                                                 :pull
  ...
  ─────────────────────────────────────────────── (muted rule)
  m  Set Operating Mode                                          :mode
```

### Wizard (F4)

Steps 1–5. Step 2 shown in the mock is the profile picker. Radio list with
magenta `◉` for selected, muted `○` for unselected. Footer row with
`[ ◀ Back ] [ Next ▶ ]` inverse buttons and `esc cancel` hint.

### Toast

62-wide panel, green border. Two lines:

```
● CH #142 landed on Thurgrek
  Sylunariel · Complete Healing · 10,000 hp · 12:04:16.5
```

## Interactions & keybinds (for parity with existing `tui::input`)

| Key                               | Action                                        |
| --------------------------------- | --------------------------------------------- |
| `1`–`7`                           | Jump to screen                                |
| `Tab` / `Shift+Tab`               | Cycle pane within screen                      |
| `[` / `]`                         | Prev / next client                            |
| `g`                               | Cycle focus group                             |
| `?`                               | Toggle help                                   |
| `:`                               | Enter command mode                            |
| `F2` / `F3` / `F4` / `F8` / `F10` | Open corresponding overlay                    |
| `↑` `↓`                           | Select row                                    |
| `/`                               | Filter current list                           |
| `t`                               | Target from selection                         |
| `i`                               | Inspect packet/spawn                          |
| `p`                               | Pull / Pause orchestrator (context-sensitive) |
| `a` / `d` / `e`                   | Assist / Disengage / Engage all               |
| `c` / `s` / `S`                   | Start CH chain / Start cycle / Stop cycle     |
| `l`                               | Loot all corpses                              |
| `m`                               | Toggle Camp ↔ Hunt                            |

These are the keybinds the mock advertises in its status-bar hints and Help
overlay. Confirm against `input::KeyHandler` — if any already bind to a
different action, prefer the existing binding and update the Help copy.

## State / data requirements

Every field shown in the mock exists on some combination of the following
already-present state objects. No new data sources are introduced.

- `ClientRegistry` — per-client: pid, name, class, level, zone, HP%, mana%,
  endurance%, group id, condition enum, state enum, activity enum,
  position (y, x, z, heading), current target, current cast, profile id.
- `GroupDirectory` — group id, label, zone, connected count, member count,
  leader name.
- `Session` — server, mode (Camp/Hunt), main assist, main tank, CH chain
  config (members, interval secs, adaptive flag, chain target), elapsed
  time, kills, deaths, XP totals (current, /h, /15m), plat pocket, plat
  banked, plat/h, top loot rollup, unread alert count.
- `NavState` — per-client: status (Idle/Holding/Moving/Stuck), destination,
  route name, stuck flag, blocker description, retry count, fallback route.
- `SpawnTable` — per-spawn: id, name, type, class, level, HP%, pos, state.
- `PacketRingBuffer` — per-packet: timestamp, direction, opcode, size,
  payload (first 16 bytes for the list, full for detail), decoded fields
  keyed by opcode schema, originating client.
- `Economy` — state, stage, current slot, stage started, cycle started,
  cycles today, vendor name, bank name, plat pocket, plat banked, next
  cycle eta, active rule file, rule list, per-slot status rows, ledger
  day totals.
- `Orchestrator` — active intent, phase, cadence, last tick, slot rows
  (slot id, name, state enum, fsm state, latency ms, profile id), signal
  feed (time, kind, message), phase timeline.
- `AlertFeed` — severity, kind, message, source, time, ack flag.
- `MenuBarState` — current top-level menu, item list for the dropdown.

If a field in the mock has no current backing (e.g. "top loot rollup" may
not yet exist), add it to the owning state struct rather than stubbing it
in the renderer.

## Files in this bundle

- `README.md` — this document
- `HANDOFF_PROMPT.md` — detailed prompts and context for implementation
- `mock/TextQuest TUI.html` — the single-file HTML mock (self-contained;
  open in a browser, no build)
- `mock/src/` — the source modules the HTML is assembled from, included for
  reference only

## Source code you should read first

On the Rust side, in rough order of relevance:

1. `textquest/src/tui/theme.rs` — palette definitions; `neriak()` is the target.
2. `textquest/src/tui/ui/widgets.rs` — existing panel / bar / table helpers.
3. `textquest/src/tui/ui/mod.rs` — global chrome (header, status bar, overlay
   plumbing, responsive breakpoints).
4. `textquest/src/tui/ui/dashboard.rs` — the Characters screen, which is the
   largest and sets the density/typography bar.
5. Each per-screen module under `ui/` that this doc references.

## Out of scope

- New state plumbing beyond the fields listed in **State / data**.
- New theme / palette variants.
- Changes to `input::KeyHandler` bindings (except Help copy).
- Any attempt to reproduce the CRT scanline / phosphor / vignette look —
  those are browser affordances only and have no terminal equivalent.
