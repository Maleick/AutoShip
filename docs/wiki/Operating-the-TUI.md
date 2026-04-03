# Operating the TUI

## Current Operator Workflow

DMFT's default interface is a ratatui-based dashboard with four main screens:

| Screen | Key | Main purpose |
| --- | --- | --- |
| Characters | `1` | Roster, selected character state, group and scope panels |
| Map | `2` | Zone geometry, spawn overlays, named tracking, nav path overlays |
| Navigation | `3` | Per-character Zone, Status, and Destination, with route progress, recovery state, and waypoint queue |
| Debug | `4` | Spawn table, filters, live search, target detail, hex dump |

### Core keys

| Key | Action |
| --- | --- |
| `1-4` | Switch screens |
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

- Themes currently cycle through Dark Modern, Dracula, and Classic.
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

### Debug

Use this for:

- spawn list inspection
- live text filtering
- target details
- raw hex dump of the selected spawn or demo payload

## Command and Overlay Surfaces

- `:` opens the command bar defined in `dmft/src/tui/app.rs`.
- `?` opens a context-sensitive help overlay.
- `config` opens the interactive configuration panel.
- `chui` opens the CH chain panel.
- `wizard` opens the setup wizard shell.

## Internals

- TUI state is centered in `dmft/src/tui/app.rs`.
- Rendering is split under `dmft/src/tui/ui/`.
- Demo content comes from `dmft/src/tui/demo_data.rs`.
- Theme definitions live in `dmft/src/tui/theme.rs`.
- Map and navigation overlays are fed from `dmft/src/tui/state.rs` and `dmft/src/nav/mesh.rs`.

## Current Behavior vs Roadmap

### Current behavior

- The four-screen layout is real and current.
- Help, config, CH panel, command history, search, filters, themes, and privacy mode all exist in the codebase now.

### Roadmap or partial wiring

- The TUI `:inject` command is still a placeholder instead of a full injection trigger.
- Some map orientation and overlay behavior has comments in code noting that final live-client verification is still deferred in a few cases.
