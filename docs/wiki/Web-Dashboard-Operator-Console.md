# Web Dashboard Operator Console

The M6 web dashboard now exposes a single operator console instead of the earlier fantasy-themed demo shell. The live screen is organized around the control surfaces operators asked for in issue `#1561`:

- Session command center
- Spawn finder
- Group coordination
- Navigation control
- Economy monitoring
- Combat analytics
- System health

## Layout

The console is split into two vertical rails on desktop and collapses to a single stack on narrower widths.

```text
+----------------------------------------------------------------------------------+
| Header: cluster, shard, zone, alert count, websocket status, last update         |
+-----------------------------------------+----------------------------------------+
| Session Command Center                  | Economy Monitoring                     |
| - live client rows                      | - loot intake                          |
| - create session wizard                 | - vendor cadence                       |
| - terminate / recover actions           | - profit trend                         |
| - heartbeat + recovery state            | - wishlist editor                      |
+-----------------------------------------+----------------------------------------+
| Spawn Finder                            | Combat Analytics                       |
| - per-observer live spawn snapshots     | - DPS line chart                       |
| - realtime text filtering               | - spell usage bars                     |
| - sort by name, level, class, race,     | - death log                            |
|   distance, or HP%                      | - rotation efficiency                  |
| - click-to-target actions               |                                        |
+-----------------------------------------+----------------------------------------+
| Group Coordination                      | Combat Analytics                       |
| - create group                          | - DPS line chart                       |
| - role reassignment                     | - spell usage bars                     |
| - command buttons (camp/pull/navigate)  | - death log                            |
| - command log                           | - rotation efficiency                  |
+-----------------------------------------+----------------------------------------+
| Navigation Control                      | System Health                          |
| - route list                            | - memory per client                    |
| - waypoint editor                       | - frame rate                           |
| - map preview                           | - IPC latency p50 / p95 / p99          |
| - stuck-client count                    | - error log + recovery action          |
+-----------------------------------------+----------------------------------------+
```

## Live Data Contract

The frontend uses two backend surfaces:

- `GET /api/dashboard` for the full initial snapshot
- `POST /api/dashboard/action` for mutating operator actions such as session creation, recovery, group commands, route creation, and wishlist updates

Realtime refresh is driven by `/ws` events. The backend now broadcasts `dashboard.snapshot` messages after actions and on a periodic tick so the console can refresh latency, economy, navigation, and combat telemetry without polling each subsystem separately.

The spawn finder uses the same snapshot feed. The orchestrator now persists
live observer snapshots to `data/runtime/live_spawns.json`, and the dashboard
hydrates a dedicated panel that can filter and sort each observer's nearby
spawn list without leaving the browser.

## Operator Notes

- Session creation is profile-driven and intentionally lightweight so operators can recover capacity quickly during live runs.
- Spawn Finder supports type-to-filter and one-click targeting for the selected observer.
- Group role edits are inline and post immediately to the dashboard action endpoint.
- Navigation routes are stored as named waypoint sets with a map preview rendered directly in the console.
- Wishlist entries are editable in-place to keep loot routing and vendor priorities close to the economy data.

## Review Artifact

Figma was not available in-session, so this page is the in-repo mockup artifact for review. The implemented React layout matches the section ordering and interaction model documented here.
