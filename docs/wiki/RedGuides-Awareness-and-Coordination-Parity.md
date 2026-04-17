# RedGuides Awareness and Coordination Parity

Issue `#1691` closes the remaining operator-surface gap for RedGuides awareness and coordination utilities by reusing the existing TextQuest session, spawn, routing, alert, and tell infrastructure instead of creating a second transport or state model.

The goal of this page is not to claim literal UI cloning. The goal is auditable parity accounting:

- every plugin in scope is mapped to a concrete TextQuest surface
- every mapping is tagged as `Native`, `Adapted`, or `Deferred`
- every mapping has an explicit GitHub issue owner

## Current Operator Surface

The mounted web app now treats the operator dashboard as the default `engagements` view and keeps the issue-`#1691` utility surfaces reachable from the left rail:

- `OperatorDashboard` for session status, spawn visibility, group state, navigation, and health
- `Player Watch` for MQ2Paranoid / MQ2Posse-style friend and stranger proximity rules
- `Say Detection` for MQ2Say-style fast `/say` rule matching and notification routing
- `X-Assist` for cross-group main-assist targeting configuration
- `Alert Routing`, `Rare Spawn Alerts`, and `Network Box Chat` for adjacent alerting and coordination infrastructure already reused by the parity pack

The TUI remains a first-class operator surface for target info, extended spawn visibility, map labels, and spawn event streams where TextQuest already has native terminal-native panels.

## Plugin Mapping

| Plugin | TextQuest mapping | Operator surface | Classification | Owner |
| --- | --- | --- | --- | --- |
| `MQ2Status` | Session health, recovery state, heartbeat, and client telemetry | Web `OperatorDashboard` session and health panels; TUI session monitor | Native | `#1691` |
| `MQ2Targets` | Current-target visibility through spawn targeting and target-info readouts instead of a separate target window clone | Web `Spawn Finder`; TUI target info in `textquest/src/tui/ui/spawns.rs` | Adapted | `#1691` |
| `MQ2Spawns` | Spawn list plus spawn/despawn event awareness backed by the shared spawn snapshot pipeline | Web `Spawn Finder`; TUI spawn list and `spawn_events` panel | Adapted | `#1691`, `#851` |
| `MQ2SpawnSort` | Filterable, sortable spawn index with one-click targeting | Web `Spawn Finder` | Native | `#1691` |
| `MQ2Tracking` | Route tracking, observer distance context, and map-aware navigation instead of a standalone tracking HUD | Web navigation control; TUI map and navigation panels | Adapted | `#1691`, `#838` |
| `MQ2ToolTip` | Important status, spawn, and route metadata is exposed directly in panels instead of mouseover-only overlays | Existing dashboard cards and TUI detail panels; no separate tooltip-only clone | Deferred | `#1691`, `#1690` |
| `MQ2OTD` | Target direction is covered through route preview, spawn distance, and labeled map context rather than an overhead compass overlay | Web navigation and spawn panels; TUI map labels | Adapted | `#1691`, `#838` |
| `MQ2Posse` | Friend-or-stranger proximity filtering, list management, and zone-in alert preferences | Web `Player Watch` panel | Native | `#1691`, `#859` |
| `MQ2WorstHurt` | Reuse the heal coordinator and extended-target heuristics before adding a dedicated “worst hurt” operator widget | Existing heal and CH-chain infrastructure; no dedicated web picker yet | Deferred | `#1691`, `#836` |
| `MQ2GroupInfo` | Group composition, role assignment, command state, and recent coordination log | Web `OperatorDashboard` group coordination panel; TUI group views | Native | `#1691`, `#836` |
| `MQ2XAssist` | Per-character cross-group main-assist targeting config with persisted API-backed edits | Web `X-Assist` panel and `/api/xassist/*` routes | Native | `#1691`, `#838` |
| `MQ2Paranoid` | Player zone-entry and zone-exit awareness with stranger/friend filtering and optional sound on entry | Web `Player Watch`; TUI spawn event awareness | Native | `#1691`, `#859` |
| `MQ2Say` | `/say` rule matching, rule actions, and notification routing through the existing alerting stack | Web `Say Detection` panel and `/api/say-detection/*` routes | Native | `#1691`, `#859` |

## Related MacroQuest Core Surfaces

| Surface | TextQuest mapping | Classification | Owner |
| --- | --- | --- | --- |
| `targetinfo` | TUI target info block in `textquest/src/tui/ui/spawns.rs`, plus the web spawn finder's current-target marker | Adapted | `#1691` |
| `xtarinfo` | Existing extended-target aware heal and combat coordination backends, with dedicated operator readouts still deferred | Deferred | `#1691`, `#836` |
| map-label style surfaces | Brewall map labels and waypoint labels in `textquest/src/tui/ui/map.rs` plus the web route preview map | Native | `#1691`, `#838` |

## Notes

- `Native` means TextQuest already exposes the operator behavior directly through its own web or TUI surface.
- `Adapted` means the behavior is covered, but through a TextQuest-native panel rather than a literal MacroQuest window or overlay clone.
- `Deferred` means the issue now records the gap explicitly and keeps ownership attached to a live GitHub issue instead of leaving the plugin untracked.
- Notification delivery continues to reuse the shared alert routing and tell/discord plumbing from `#851` and `#859`.
