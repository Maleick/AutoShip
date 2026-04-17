# Map Hotkeys and Controls

The Map screen has two layers of keyboard control:

- focused map-panel bindings, which apply when the map pane has focus
- tactical-screen shortcuts, which work anywhere on the Map screen

Use `Tab` to move focus between the map, spawn list, named tracker, and navigation panels. The map-specific bindings below only apply when the map pane is focused.

## Focused Map Panel

### Navigation and viewport

| Key | Action |
| --- | --- |
| `↑` / `↓` / `←` / `→` | Pan the current map viewport |
| `+` / `-` | Zoom in or out |
| `Home` | Center on the player and switch to local view |
| `End` | Fit the full zone and switch to global view |
| `v` | Cycle viewport mode: Auto -> Local -> Global |
| `Ctrl+A` | Set Auto view directly |
| `Ctrl+L` | Set Local view directly |
| `Ctrl+G` | Set Global view directly |
| `Shift+I` | Show current zone, map, and navmesh stats in the status lane |
| `m` | Maximize or restore the map panel |
| `?` | Open or close the help overlay |

### Layer toggles

| Key | Action |
| --- | --- |
| `g` | Toggle geometry |
| `s` | Toggle spawn markers |
| `w` | Toggle waypoint path overlays |
| `x` | Toggle navmesh overlay |
| `Shift+N` | Alternate navmesh toggle |
| `l` | Toggle labels |
| `a` | Toggle annotations |

### Entity filters

| Key | Action |
| --- | --- |
| `n` | Toggle NPC markers |
| `p` | Toggle PC markers |
| `c` | Toggle corpse markers |
| `Shift+G` | Toggle ground markers |
| `t` | Toggle pet markers |
| `r` | Toggle named markers |
| `u` | Toggle untargetable markers |

Focused map bindings take priority over broader tactical shortcuts. For example, `l` toggles map labels while the map pane is focused instead of falling through to the tactical quick-loot shortcut.

## Tactical Screen Shortcuts

These bindings work anywhere on the Map screen, even when the spawn list or another tactical panel has focus.

| Key | Action |
| --- | --- |
| `Alt+1` | Toggle geometry |
| `Alt+2` | Toggle spawn markers |
| `Alt+3` | Toggle waypoint path overlays |
| `Alt+4` | Toggle navmesh overlay |
| `Alt+5` | Toggle labels |
| `Alt+6` | Toggle annotations |
| `Alt+N` | Toggle NPC markers |
| `Alt+P` | Toggle PC markers |
| `Alt+C` | Toggle corpse markers |
| `Alt+G` | Toggle ground markers |
| `Alt+T` | Toggle pet markers |
| `Alt+R` | Toggle named markers |
| `Alt+U` | Toggle untargetable markers |
| `/` | Open spawn search |
| `f` | Cycle the spawn-list filter |
| `[` / `]` | Move between connected clients |
| `Tab` | Cycle focus between tactical panels |

## Operator Notes

- The map help overlay is screen-aware. Press `?` from the Map screen to see the current map key table in the TUI.
- The legend row at the bottom of the map now includes quick reminders for `?`, `Home`, and `End`.
- `Shift+I` is the fastest way to confirm whether you are looking at map-file data, navmesh data, or a spawn-only fallback view.
- `End` uses the same global bounds that drive the minimap and zone-fit rendering, so it is the recovery shortcut when panning too far off-center.

## Related Docs

- [Navigation and Maps](Navigation-and-Maps)
- [Operating the TUI](Operating-the-TUI)
- [Operator Guide](Operator-Guide)
