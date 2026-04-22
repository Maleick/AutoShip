# Operating the TUI

## Current Operator Workflow

TextQuest's default interface is a ratatui-based dashboard with five main screens:

| Screen | Key | Main purpose |
| --- | --- | --- |
| Characters | `1` | Roster, selected character state, group and scope panels |
| Map | `2` | Tactical map, spawn overlays, named tracking, tactical nav section, and viewport controls |
| Navigation | `3` | Per-character route status, selected-route detail, blockers, recovery state, and command reference |
| Debug | `4` | Raw spawn table, filters, target detail, hex dump, explorer, and EQ internals |
| Packets | `5` | Packet monitor UI for captured opcode events, with pause state, filtering, and send/receive separation |

### Core keys

| Key | Action |
| --- | --- |
| `1-5` | Switch screens |
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
| `F8` | Toggle the alert history overlay |
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
- seeing cross-client vitals in the group panel even when the EQ group window is incomplete
- reading target detail lines, buff counts, and pet presence from the shared roster feed

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

### Debug

Use this for:

- spawn list inspection
- live text filtering
- target details
- raw hex dump of the selected spawn or demo payload

### Packets

Use this for:

- inspecting captured packet rows when the current DLL build is emitting packet events
- pausing the packet stream without leaving the screen
- selecting individual packets with `j` / `k` or arrow keys
- inspecting packet payloads in hex and ASCII text for the selected row
- confirming the live and peak packet rates reported by the capture stream
- validating packet-monitor output during attended troubleshooting runs

Current limitation:

- The packet monitor screen exists today, but packet-hook activation and
  zone-transition proof are tracked by issue `#1270`. Do not treat an empty
  packet table as proof that zoning has no packet traffic.

## Command and Overlay Surfaces

- `:` opens the command bar defined in `textquest/src/tui/app.rs`.
- `?` opens a context-sensitive, scrollable help overlay with command usage and jump targets.
- `F8` opens the operational alert overlay with the last 100 alerts, unread count, and acknowledgment controls.
- `config` opens the interactive configuration panel.
- `chui` opens the CH chain panel.
- `wizard` opens the setup wizard shell.
- Shared TUI surfaces include breadcrumbs, tab bars, the dropdown command menu bar, toast notifications, keybinding hint rows, badges, cast bars, sparklines, scrollable lists with scrollbar indicators, tooltips, popup selectors, the config tree editor, and the first-run wizard overlay.

### Alert overlay controls

When the alert overlay is open:

- `Esc`, `q`, or `F8` closes the overlay.
- `j` / `k` or arrow keys move through alert history.
- `Enter` or `a` acknowledges the selected alert.
- `Shift+A` acknowledges every unread alert in the local store.
- `r` refreshes the history from `data/alerts.db`.

The status bar also shows an unread alert badge even when the overlay is closed.

## Internals

- TUI state is centered in `textquest/src/tui/app.rs`.
- Rendering is split under `textquest/src/tui/ui/`.
- Demo content comes from `textquest/src/tui/demo_data.rs`.
- Theme definitions live in `textquest/src/tui/theme.rs`.
- Map and navigation overlays are fed from `textquest/src/tui/state.rs` and `textquest/src/nav/mesh.rs`.

## Current Behavior vs Roadmap

### Current behavior

- The five-screen layout is real and current.
- The Characters screen is still rendered by `textquest/src/tui/ui/dashboard.rs` via the `ActiveScreen::Overview` dispatch in `textquest/src/tui/ui/mod.rs`; the old PR #507 handoff note about a missing dashboard renderer is historical only.
- Help, config, CH panel, command history, search, filters, themes, and privacy mode all exist in the codebase now.
- The Characters screen group panel consumes the orchestrator's shared roster feed, so HP, mana, endurance, target, and buff context can come from any connected client rather than only the locally selected one.
- When target data is available, member rows render a compact target line with target name plus HP percent so heal or burn decisions are visible without leaving the Characters screen.

### Roadmap or partial wiring

- If a new Characters-screen renderer regression appears, track it with a fresh issue against the current TUI surface instead of reusing the old PR #507 handoff note.
- The TUI `:inject` command is still a placeholder instead of a full injection trigger.
- Some map orientation and overlay behavior has comments in code noting that final live-client verification is still deferred in a few cases.

## Fix #2171

Packet monitor panel now displays live captured packets instead of 5 hardcoded stub entries.
