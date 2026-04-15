# Operating the TUI

## Current Operator Workflow

TextQuest's default interface is a ratatui-based dashboard with seven top-level screens:

| Screen | Key | Main purpose |
| --- | --- | --- |
| Characters | `1` | Roster, selected character state, group and scope panels |
| Map | `2` | Tactical map, spawn overlays, named tracking, tactical nav section, and viewport controls |
| Navigation | `3` | Per-character route status, selected-route detail, blockers, recovery state, and command reference |
| Debug | `4` | Raw spawn table, filters, target detail, hex dump, explorer, and EQ internals |
| Packets | `5` | Live packet monitor with pause state, filtering, opcode decode, and send/receive separation |
| Economy | `6` | Vendor cadence, loot queue, banking consolidation, and profit/watchlist metrics |
| Orchestrator | `7` | Consolidated session, group, navigation, economy, combat, and system operator dashboard |

### Core keys

| Key | Action |
| --- | --- |
| `1-7` | Switch screens directly |
| `Shift+1-6` | Focus group G1-G6 |
| `Shift+0` | Clear group focus |
| `Tab` | Cycle focused pane |
| `[` / `]` | Cycle between clients |
| `/` | Spawn search |
| `f` | Cycle spawn filter |
| `g` | Toggle group section |
| `v` | Toggle scope section |
| `z` | Collapse focused section |
| `+` / `-` | Adjust map Z slice |
| `m` | Maximize map |
| `p` | Privacy mode |
| `T` | Cycle theme |
| `:` | Command mode |
| `?` | Help overlay |
| `q` | Quit |

### Themes and privacy

- Themes currently cycle through Dark Modern, Dracula, Classic, and Neriak Third Gate.
- Privacy mode redacts your character names and server label for screenshots or streaming.

### Demo mode expectations

In demo mode the TUI still shows:

- simulated characters and groupings
- cast bars and activity
- map overlays
- navigation states, including moving and stuck states
- CH chain panel state

That makes it the normal workflow for non-Windows UI development.

## Screen-by-Screen Notes

### Characters

Use this for:

- roster health and mana checks
- group focus and scope filtering
- seeing selected-character status at a glance

### Map

Use this for:

- zone geometry from `config/maps`
- spawn overlays
- navmesh overlay when available
- named tracking with timers

### Navigation

Use this for:

- route status across focused characters
- current destination and waypoint counts
- quickly spotting stuck or arrived states

### Economy

Use this for:

- vendor cycle timing and abort state
- loot queue depth and distribution backlog
- banking progress and consolidated plat totals
- recent loot-watch and wishlist activity

### Orchestrator

Use this for:

- live per-client session status, mana/endurance, DPS estimate, and location
- group readiness, formation spread, and spell-sync visibility
- route state, zoning FSM, blocker, and recovery summaries
- combat trend, spell frequency, death/recovery log, and system health

The Orchestrator dashboard is a tabbed surface inside screen `7` with:

- `←` / `→` or `h` / `l` to cycle dashboard tabs
- `↑` / `↓` or `j` / `k` to change the selected client
- `Space` to pause or resume automation
- `E` to dispatch `engage`
- `D` to dispatch `disengage`
- `C` to dispatch `camp status`
- `N` to dispatch `nav ui`
- `X` or `Delete` to eject the selected client from the orchestrator

### Debug

Use this for:

- spawn list inspection
- live text filtering
- target details
- raw hex dump of the selected spawn or demo payload

### Packets

Use this for:

- live send/receive packet capture
- pausing the packet stream without leaving the screen
- opcode decode and filter inspection
- comparing raw traffic while other screens stay focused on state

## Command and Overlay Surfaces

- `:` opens the command bar defined in `textquest/src/tui/app.rs`.
- `?` opens a context-sensitive, scrollable help overlay with command usage and jump targets.
- `config` opens the interactive configuration panel.
- `chui` opens the CH chain panel.
- `wizard` opens the setup wizard shell.
- Shared TUI surfaces include breadcrumbs, tab bars, the dropdown command menu bar, toast notifications, keybinding hint rows, badges, cast bars, sparklines, scrollable lists with scrollbar indicators, tooltips, popup selectors, the config tree editor, and the first-run wizard overlay.

## Internals

- TUI state is centered in `textquest/src/tui/app.rs`.
- Rendering is split under `textquest/src/tui/ui/`.
- Demo content comes from `textquest/src/tui/demo_data.rs`.
- Theme definitions live in `textquest/src/tui/theme.rs`.
- Map and navigation overlays are fed from `textquest/src/tui/state.rs` and `textquest/src/nav/mesh.rs`.

## Current Behavior vs Roadmap

### Current behavior

- The seven-screen layout is real and current.
- The Characters screen is still rendered by `textquest/src/tui/ui/dashboard.rs` via the `ActiveScreen::Overview` dispatch in `textquest/src/tui/ui/mod.rs`; the old PR #507 handoff note about a missing dashboard renderer is historical only.
- Help, config, CH panel, command history, search, filters, themes, privacy mode, economy controls, and the Orchestrator operator dashboard all exist in the codebase now.

### Roadmap or partial wiring

- If a new Characters-screen renderer regression appears, track it with a fresh issue against the current TUI surface instead of reusing the old PR #507 handoff note.
- The TUI `:inject` command is still a placeholder instead of a full injection trigger.
- Some map orientation and overlay behavior has comments in code noting that final live-client verification is still deferred in a few cases.
