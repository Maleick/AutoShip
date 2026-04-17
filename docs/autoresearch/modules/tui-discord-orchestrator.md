# TUI / Discord / Orchestrator Modules

Files: `tui/**/*.rs` (20+ in `tui/` + 16 in `tui/ui/`), `discord/{bot,bridge,mod,relay,webhook}.rs`, `orchestrator/{cross_group,mod,session_control,xassist}.rs`.

This is the user-facing layer plus the high-level fleet coordinator. Everything in this group consumes state; the previous groups (process, eq, combat, …) produce it.

---

## `tui/` — Ratatui app

### Central state: `tui::app::App`

- **Purpose.** Single owning struct for all UI state + subsystem handles. Every renderer and event handler takes `&App` or `&mut App`.
- **Fields (selection).**
  - `clients: Vec<ClientState>` — per-process metadata: PID, EQ base, spawns, HP/mana, zone, group info.
  - `spawns: Vec<SpawnInfo>` — live spawn snapshot.
  - `active_screen: ActiveScreen`, `active_panel: ActivePanel`.
  - Screen states: `overview_state`, `spawns_state`, `map_state`, `command_bar_state`, `nav_state`, …
  - Integration: `discord_bridge`, `discord_webhook`, `orchestrator_state`, `session_control` vec.
  - UX: `theme`, `toast`, `command_aliases`, `command_history`, `favorites`, `routing_scope`, `session_tokens`.
- **Public enums.**
  - `ActiveScreen`: `Overview`, `Tactical`, `Navigation`, `Debug`, `PacketMonitor`, `Economy`, `Orchestrator` (`tui/app.rs:60`).
  - `ActivePanel` — per-screen focus.
- **Invariants.**
  - **Privacy mode.** When `app.privacy_mode` is enabled (toggled via `toggle_privacy`, `tui/app.rs:2706`), `App::redact_name` redacts names matching a connected character's `displayed_name` or `name` (replaced with `Toon-NN`). The check is not channel-aware.
  - **Caching.** Filtered spawn indices, map presentation cells, live group rosters, client name indices are cached and invalidated via `spawn_revision` bumps.

### Render loop: `tui::run`

- **Purpose.** Drives the UI/event loop on a configurable refresh interval — `app.refresh_rate_ms` (default 250 ms in `App::new`, `tui/app.rs:811`). Each iteration polls input, refreshes visible state, and re-renders; longer-running subsystem work is scheduled on its own timers.
- **Invariants.**
  - Data-poll cadence is _fast_ for the active client, _slow_ for backgrounded ones.
  - Discord sender auth check runs per iteration.
  - Independent interval timers fire from this loop: orchestrator camp ticks ~1 s, soul engine ~5 s, log watcher / packet monitor / named-tracker polls maintain their own cadence.
  - Event-to-action flow: keyboard input → `event.rs` → `App::execute_command()` → IPC pipe or Discord bridge routing.

### Per-screen state: `tui::state`

Holds `OverviewScreenState`, `MapScreenState` (z-filter, viewport, filter presets, highlights, named markers, camp overlays), `HexDumpState`, `CommandBarState` (history + completion + favorites), `Toast` with TTL expiry, etc.

### `tui::ui::*` renderers

- **Purpose.** One renderer per screen / panel; primarily presentation, though draw paths take `&mut App` and may update UI-local caches or view state during rendering.
- **Entry point.** `ui::mod::draw(frame: &mut Frame, app: &mut App)` (`tui/ui/mod.rs:51`) — applies the outer margin and dispatches to the active screen.
- **Public helpers.** `panel` (bordered block), `themed_header_row`, `hp_color`, `spawn_row_style`, `cast_summary`, `truncate_inline`.
- **Screens.** `dashboard`, `spawns`, `spawn_events`, `map`, `navigation`, `groups`, `ch_chain`, `dps_bars`, `economy_controls`, `eq_internals`, `explorer`, `orchestrator_panel`, `packets`, `zone_status_panel`, `widgets`.
- **Invariants.**
  - **UI-scoped mutation only.** Renderers may mutate transient UI state (scroll offsets, selections, filtered-spawn caches) during drawing, but game/system state changes and command side effects remain outside the render path — those live in `run.rs` event handlers and subsystem/orchestrator code.
  - **Responsive.** Width-based branching (stacked vs. side-by-side); thresholds around 80 / 100+ columns.
  - **Focus styling.** Active panels draw bright borders; inactive panels dim.

### Other `tui/` modules worth noting

- `tui::command` + `tui::menu` — command palette and context menus.
- `tui::companion`, `tui::cast`, `tui::live_cast_capture`, `tui::dps` — derived subsystems (cast summaries, DPS board).
- `tui::client` — client-tab helpers.
- `tui::config_panel` — edit config live.
- `tui::group_builder` — drag-together group builder UI.
- `tui::priorities` — heal/assist priority configuration.
- `tui::achievements` / `tui::sprites` / `tui::sound` — cosmetic + audio glue (rodio).
- `tui::demo_data` — fixture data for development builds.
- `tui::session_monitor` — live session dashboard.

---

## `discord/` — three-layer integration

### `discord::webhook`

- **Purpose.** Outbound alerts routed per category.
- **Public API.** `EventCategory` (`Kills`, `Loot`, `Timers`, `Feats`, `Status`), `AlertLevel` (`Info`, `Warning`, `Error`, `Critical`), `DiscordAlert` (title, message, level, category, embed fields, route_key, message_mode, mention_policy), `WebhookSender`.
- **Invariants.**
  - Rate limit: 30 posts per minute per webhook (sliding window in a `VecDeque`).
  - Runs on a background thread.
  - Webhook URLs live in config (`DiscordRouteConfig`), never in code.

### `discord::bridge`

- **Purpose.** Bidirectional command channel between the TUI and the embedded bot.
- **Public API.** `BridgeCommand`, `BridgeResponse`, `TuiBridge`, `BotBridge`, `create_bridge()`.
- **Invariants.** `mpsc` under the hood; command auth happens on the TUI side via `discord_sender_is_authorized` (case-insensitive allow-list).

### `discord::relay`

- **Purpose.** Translate `FleetEvent` and chat into `DiscordAlert` / per-channel webhook post.
- **Public API.** `ChatChannel` (Group/Raid/Guild), `ChatRelay`, `EventRelay`.
- **Invariants.** `Kill → Kills`, `Death → Status`, `CombatRound → buffered DPS`.

### `discord::bot`

- **Purpose.** Serenity-based embedded bot (tokio task inside orchestrator).
- **Public API.**
  - `DzLockout`, `LockoutType` — 48 h replay / 6.5 d full lockout timers per expedition.
  - `SpawnEvent` — contested mob sightings.
  - `BotState` — RwLock-wrapped shared state (lockouts, spawn events, bridge handle, guild_id).
  - `Handler: EventHandler` — registers slash commands on ready, handles interactions.
  - `start(token)` — initialises bot; empty token skips connection.
- **Invariants.**
  - `SyncBridge` wraps a non-Sync mpsc in a `Mutex` so `Arc<BotState>` stays `Sync`.
  - OAuth token held only in config, never logged.

---

## `orchestrator/` — camp loop + cross-group + session control

### `orchestrator::mod` — `Orchestrator` struct

- **Purpose.** Top-level fleet conductor.
- **Fields (selection).**
  - `client_pids: Vec<u32>`.
  - `active_camp: Option<CampLoop>`.
  - `combat: CombatCoordinator`.
  - `tick_count: u64`.
  - `last_dispatched: [(u32, CampAction)]` — for status display.
  - `game_states: HashMap<u32, GameState>`.
  - `state_readers: HashMap<u32, SharedStateReader>`.
  - `session_tokens: HashMap<u32, SessionToken>` — cryptographically random, generated at registration.
  - `operating_mode: Camp | Hunt`, `active_hunt: Option<HuntLoop>`.
  - `sell_cycle`, `camp_db`.
- **Invariants.**
  - Camp tick cadence: `CAMP_TICK_INTERVAL = 1s` (`tui/run.rs:64` — driven from the TUI).
  - Stale data threshold: `STALE_TICK_THRESHOLD = 3` (older than 3 ticks → snapshot refuses).
  - Progression checks: every 50 ticks.
  - CC expiry: emit `CcExpiring` 3 ticks before it wears.
  - Session tokens required for every IPC command (authentication).
  - Pipe is one-shot: DLL disconnects after each command → orchestrator connects per call.
  - Roster snapshot persistence for web consumers (e.g. `live_sessions.json`) is planned/future work; no writer currently exists in the `textquest` crate.

### `orchestrator::session_control`

- **Purpose.** Per-session record: `SessionState` (`Active`, `Paused`, `Error`), `session_id`, `group_id`, `routing_scope`.
- **Public API.** `apply_command(...)` — Pause / Resume / SetGroup with change detection.

### `orchestrator::cross_group`

- **Purpose.** Same-zone emergency rescue across groups.
- **Public API.** `MemberSnapshot`, `GroupSnapshot`, `RequestKind` (`Rez` | `Assist`).
- **Invariants.**
  - Low-HP threshold: 35 %.
  - Responder minimum HP: 50 %.
  - `REZ_GEM` slot: 5 (typical cleric).
  - Only triggered when a distressed group cannot self-stabilise.

### `orchestrator::xassist`

- **Purpose.** Cross-assist targeting — multiple groups can share a main-assist anchor.

---

## End-to-end flows

| Flow                | Direction                                                                                                                                 |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Input → state       | Keyboard → TUI event handler → `App::execute_command` → IPC pipe or Discord bridge                                                        |
| Memory → render     | EQ DLL → `SharedStateReader` (mmap) → `ClientState` → cached snapshots → render                                                           |
| Fleet event → alert | Metrics → `EventRelay` → `DiscordAlert` → `WebhookSender` → POST                                                                          |
| Discord → command   | User chat → `BridgeCommand` → TUI polls each tick → execute locally → `BridgeResponse`                                                    |
| App → orchestrator  | TUI session/group/routing commands → orchestrator routing scope → `SessionControl` mutations → `CrossGroupCoordinator` watches for rescue |

### State-flow summary

```
App (central)
├── clients: Vec<ClientState>          ← memory reads
├── spawns: Vec<SpawnInfo>             ← live spawn snapshot
├── active_screen, active_panel        ← keyboard
├── theme, toast, command_aliases      ← config
├── discord_bridge, discord_webhook    ← Discord
├── orchestrator_state                 ← camp/hunt/combat
└── session_control[]                  ← per-client pause/group/routing

Render pass (per tick)
├── ui::mod::draw(frame, &mut App)
│   ├── dashboard | spawns | map | …
│   ├── reads App state (immutable)
│   └── caches filtered spawns / map cells
└── mutates per-screen UI state (selection, scroll, collapsed sections)

Event loop (per tick — run.rs)
├── keyboard input → App::execute_command()
├── Discord poll → TuiBridge → execute → respond
├── memory refresh → ClientState updates → spawn_revision bump
├── Orchestrator.tick → CampLoop FSM → IPC dispatch
└── UI mutations (focus, selections, collapsed sections)
```
