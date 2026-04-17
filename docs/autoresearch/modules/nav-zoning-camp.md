# Navigation / Zoning / Camp Modules

Files: `nav/{camp,mesh,mesh_stub,mod,movement_queue,recorder,relocate,router,stuck_detection,zone_transition}.rs`, `zoning/{failure_codes,mod,recovery,state}.rs`, `camp/{aa_spend,banking,buffs,cc,class_config,collectibles,config,equipment,event_triggers,forage,hunt,loot,mod,personality,positioning,progression,puller,quest_tracker,recovery,skill_tracker}.rs`.

These are the three "movement / recovery / behaviour" layers stacked on top of each other.

---

## `nav/` — navmesh + routing + zone transitions

### `nav::mesh`

- **Purpose.** Download and parse navmesh files, expose A\* pathfinding.
- **Public API.** `download_zone_mesh`, `parse_navmesh`, `load_zone`, `find_path`.
- **Invariants.**
  - `download_zone_mesh` performs a synchronous `reqwest::blocking::get(url)` against `mqmesh.com`. There is **no explicit timeout or retry configuration** in `nav/mesh.rs` — callers get whatever default the `reqwest` blocking client provides.
  - `is_safe_coordinate` lives in `nav/zone_transition.rs:98` (and is consumed by `zone_transition` / `zoning::recovery`), **not** by `nav::mesh::find_path`. Pathfinding itself does not bounds-check waypoints.
  - A cached navmesh is reusable across clients in the same zone.

### `nav::mesh_stub`

- **Purpose.** No-op implementation of the mesh trait used for tests and non-Windows builds.

### `nav::router`

- **Purpose.** Compose zone → zone → waypoint travel plans.
- **Public API.** `TravelStep` (`WalkTo`, `ZoneTo`, `PortTo`, `RelocateTo`, `StaggerWait`), `TravelPlan`, `plan_group_travel`.
- **Invariants.** `TravelPlan::advance` is stateful — callers must advance steps in order; no random access.

### `nav::zone_transition`

- **Purpose.** Low-level per-client FSM: Walking → Zoning → Recovering → Complete/Failed.
- **Public API.** `ZoneTransitionFsm`, `ZoneTransitionState`, `is_safe_coordinate`.
- **Invariants.**
  - Pull-based: caller invokes `tick()` every orchestrator tick.
  - Stuck detection fires after `ZONE_TRANSITION_TIMEOUT = 30s` (`nav/zone_transition.rs:18`). Note the orchestrator-level `zoning::state::ZONE_TRANSITION_TIMEOUT` is a separate 60s timeout.
  - Failure surfaces a `ZoneFailureCode` for the `zoning` layer to handle.

### `nav::stuck_detection`

- **Purpose.** "Has this client moved in the last N ticks?" heuristic — triggers recovery paths.

### `nav::movement_queue`

- **Purpose.** Per-client buffer of movement commands.
- **Public API.** `ClientMovementQueue`, `MovementQueueManager`.
- **Invariants.** Queue depth is bounded; overflow drops oldest (movement is commutative in practice — latest wins).

### `nav::recorder`

- **Purpose.** Capture live movement trails and simplify them into reusable waypoint sequences.
- **Public API.** `WaypointRecorder`, `simplify_path`.

### `nav::relocate`

- **Purpose.** Enumerate the AA / click / spell relocation loadouts a group can use (e.g., Circle of Summer, Origin, gate).
- **Public API.** `RelocationLoadout`, `group_ready_relocations`.

### `nav::camp`

- **Purpose.** Named camp points — canonical safe locations per zone, consumed by the router + camp system.

### `nav::mod`

- **Purpose.** Entry point for slash-command interception (`try_handle_local_slash_command`) and re-exports.
- **Invariants.** `command_dispatch.rs` calls this before falling through to IPC.

---

## `zoning/` — recovery from zone failures

### `zoning::failure_codes`

- **Purpose.** Canonical mapping from zone-failure codes to recovery actions.
- **Public API.**
  - `ZoneFailureCode` (25+ variants: `Success`, `GeneralFailure`, `TooFar`, `InvalidCoordinates`, `PlayerInCombat`, …).
  - `ZoneFailureCode::{from_code, description}`.
  - `RecoveryAction` (`RetryZone`, `UseNewCoords`, `ClearQueue`, `WaitOutOfCombat`, `WaitManaRegen`, `Abandon`, `RevalidatePath`, `InvestigateState`).
  - `ZoneFailureState` + `map_action` — deterministic per-code.
- **Invariants.**
  - `ZoneFailureState::new` initialises `backoff_until = now()`; retries push it forward.
  - Deterministic map: same code → same action, always.

### `zoning::recovery`

- **Purpose.** Track recovery progress for a single failed transition: position on landing, safe-coord requests, fallback pos.
- **Public API.** `ZoneRecoveryState::{advance_retry, create_request}`.
- **Invariants.**
  - Backoff `= 2^(attempts-1)` seconds, capped at 30 s (`MAX_RECOVERY_BACKOFF`). Attempts ≥10 saturate.
  - `advance_retry` checks `can_retry()` **first**; when exhausted it returns `false` without incrementing. The counter only advances when a retry actually proceeds (`nav/zoning/recovery.rs:91`).
  - `moved_to_safe_pos` flag gates further retries.

### `zoning::state`

- **Purpose.** Orchestrator-level FSM: `Idle → Validating → Loading → InGame → Failed → Recovering`.
- **Public API.** `ZoneTransitionStateMachine::{request_zone, validation_ok, loaded, failure, retry}`.
- **Invariants.** Runs independently from `nav::zone_transition::ZoneTransitionFsm`; the two coordinate via failure codes, not by sharing state.

---

## `camp/` — pull / fight / loot behaviour

`camp/` is large (~20 submodules). It is a tick-driven behaviour tree for a single camp.

### `camp::state`

- **Purpose.** Top-level FSM that owns the group's camp cycle.
- **Public API.**
  - `CampState` (`Idle`, `Pulling`, `Fighting`, `Looting`, `Medding`, `Buffing`, `Recovery`).
  - `CampLoop` — main struct (config, members, tick counter, recovery tracker, buff/cc/loot state).
  - `CampEvent` (`CharmBreak`, `AddSpawned`, `CcExpiring`).
  - `CampSnapshot` — per-tick game state: healer mana %, tank HP %, target HP %, member HP array, combat flags.
  - `CampAction` (`Slash`, `CombatEngage { target_id }`, `CombatDisengage`) + builders (`from_slash_vec`, `as_slash`, `contains`, `starts_with`).
- **Invariants.**
  - Recovery is a pre-tick check: deaths in progress freeze pulling until everyone is alive.
  - `tick` is pull-based; `tick: u64` monotonically increases and is used as a phase-duration clock.
  - `member_hp` is gated by availability — missing HP data is treated as "no dead".
  - Camp actions include both slash commands **and** `CombatEngage` IPC so the DLL-side Combatant FSM activates its class rotation.
  - Buff phase precedes the next pull; `BuffTracker` tracks gem state and rebuff windows.
  - Loot cycle is exclusive to the `Looting` state and cleared on exit.

### Behaviour submodules

| Submodule              | Responsibility                                                                       |
| ---------------------- | ------------------------------------------------------------------------------------ |
| `camp::buffs`          | `BuffTracker`, `check_buffs` — rebuff scheduling.                                    |
| `camp::cc`             | `CcMember`, `CcTracker` — mez/root duration, expiry warnings.                        |
| `camp::recovery`       | `RecoveryTracker`, `death_commands_with_roles` — rez orchestration, death detection. |
| `camp::loot`           | `CorpseEntry`, `LootConfig`, `LootCycle` — corpse queue + filter rules.              |
| `camp::hunt`           | Free-roaming hunt mode alternative to fixed camping.                                 |
| `camp::forage`         | Forage skill usage during down-time.                                                 |
| `camp::collectibles`   | Tradeskill / collectible gather behaviour.                                           |
| `camp::puller`         | Pull-target selection (priority, distance, con).                                     |
| `camp::positioning`    | Tank-at-wall / melee arc positioning helpers.                                        |
| `camp::class_config`   | Per-class knobs (cast priorities, aggro caps).                                       |
| `camp::equipment`      | Weapon/shield swap rules during camp transitions.                                    |
| `camp::event_triggers` | Named-mob / AE / timer triggers.                                                     |
| `camp::quest_tracker`  | Quest objective progress state.                                                      |
| `camp::skill_tracker`  | Passive skill-up farming.                                                            |
| `camp::personality`    | RP-layer chatter / realism touches.                                                  |
| `camp::progression`    | Level-based camp advancement (checked every 50 ticks).                               |
| `camp::banking`        | Banker shuttle cycle.                                                                |
| `camp::aa_spend`       | AA-point spending policy.                                                            |
| `camp::config`         | `CampConfig` — pull range, med thresholds, phase durations.                          |
| `camp::mod`            | Re-exports + integration glue.                                                       |

### Timing constants to remember

- `CAMP_TICK_INTERVAL = 1s` (`tui/run.rs:64` — driven from the TUI tick, not a camp-module constant).
- `STALE_TICK_THRESHOLD = 3` — if a critical role's snapshot is older than 3 ticks, refuse to build a `CampSnapshot`.
- `PROGRESSION_CHECK_INTERVAL = 50` — level-gate camp advancement.
- `CC_EXPIRY_BUFFER = 3` — emit `CcExpiring` 3 ticks before a mez wears.

---

## Integration summary

1. **Nav ↔ Zoning.** `nav::zone_transition` reports failures; `zoning::{failure_codes, recovery, state}` own the retry/backoff policy.
2. **Nav ↔ Camp.** Decoupled — camp emits slash commands that imply movement; nav handles travel. The camp FSM does not call nav APIs directly.
3. **Zoning ↔ Camp.** Transparent — camp resumes once zoning reports the client back `InGame`.
4. **Nav → TravelPlan → ZoneTransitionFsm.** Mesh load → A\* → `TravelStep`s → per-step FSM.
5. **Camp → DLL Combatant FSM.** Camp emits `CampAction::CombatEngage`; orchestrator dispatches the IPC so the DLL's class rotation wakes up.
