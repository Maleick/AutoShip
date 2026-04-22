# Travel System Guide

The TextQuest travel system moves groups of characters across EverQuest zones
while handling staggered entry, porter-class teleports, personal relocation
items/AAs, and failure recovery.

## Architecture Overview

```
GroupRouter / plan_group_travel()   ← high-level group planner
        │
        ├── RelocationLoadout       ← per-client clicky/AA inventory
        │   └── group_ready_relocations() → RelocateTo steps (all-or-none)
        │
        └── generate_zone_staggers() → StaggerWait steps (default path)

TravelPlan (per client)             ← ordered IndexedQueue<TravelStep>
        │
        └── ZoneTransitionFsm       ← nav-level FSM (WalkTo/ZoneTo/PortTo)
                └── ZoneTransitionStateMachine  ← orchestrator-level FSM
                        └── ZoneFailureState / ZoneFailureCode → RecoveryAction
```

## Key Types

### TravelStep

Each step in a `TravelPlan` is one of:

| Variant | Description |
|---------|-------------|
| `WalkTo { waypoints }` | Follow waypoints within the current zone |
| `ZoneTo { zone_name, zone_line_pos }` | Walk to a zone line and cross it |
| `PortTo { zone_name, caster_id }` | Group port by a Druid or Wizard |
| `RelocateTo { zone_name, option_id, source }` | Personal AA or clicky teleport |
| `StaggerWait { min_secs, max_secs }` | Randomized delay before zoning |

### TravelPlan

A per-client ordered sequence of `TravelStep` values backed by an
`IndexedQueue`. Advance with `plan.advance()`, query with `plan.current()`,
and check completion with `plan.is_complete()`.

### GroupRouter

Coordinates travel for an entire group. Supports porter registration and
per-client relocation loadouts.

```rust
let mut router = GroupRouter::new();
// Druid (6) and Wizard (12) can cast group ports.
router.set_porters(vec![druid_id, wizard_id]);
router.set_relocation_loadouts(loadouts); // per-client clickies/AAs

let plans = router.plan_travel(&client_ids, &class_map, "gfay", "wakening");
```

## Planning Logic

### 1. Group Relocation (fastest path)

If **every** client in the group has a ready relocation option for the
destination zone, the router issues `RelocateTo` steps for everyone. This is
an all-or-none check — if even one character is on cooldown, the group falls
back to staggered zoning.

```rust
// All clients have Throne of Heroes ready → everyone gets RelocateTo.
// Client 2's AA is on cooldown → everyone falls back to StaggerWait.
```

### 2. Staggered Zone Transitions (default path)

When relocation is unavailable, each client receives a `StaggerWait` step with
a deterministic-but-varied delay computed by `generate_zone_staggers()`:

```rust
let staggers = generate_zone_staggers(&client_ids, 5, 60, seed);
// Returns HashMap<ClientId, u32> — delay in seconds per client.
```

The hash is seeded deterministically so re-runs with the same seed produce
identical delays (useful for testing and replay).

### 3. Porter-Based Routing (future)

When porters are registered via `set_porters()` and the route spans many zones,
the planner will prefer `PortTo` steps over walking. This feature is tracked
but not yet activated — the router currently always stagger-zones.

## Zone Transition FSM (nav level)

`ZoneTransitionFsm` drives a single client through a transition:

```rust
use textquest::nav::zone_transition::{ZoneTransitionFsm, TransitionKind};

let mut fsm = ZoneTransitionFsm::new(client_id);

// Start a walk-to transition:
fsm.start(TransitionKind::WalkTo {
    destination: Waypoint::new(100.0, 50.0, 0.0),
});

// Start a zone-line crossing:
fsm.start(TransitionKind::ZoneTo {
    zone_name: "highkeep".to_string(),
    zone_line_pos: Waypoint::new(0.0, 0.0, 0.0),
});

// Port spell (caster must already be registered):
fsm.start(TransitionKind::PortTo {
    zone_name: "poknowledge".to_string(),
    caster_id: 10,
});
```

States: `Idle → Walking → Zoning → Idle` (success) or `→ Recovering` (failure
with up to `MAX_RECOVERY_ATTEMPTS` retries).

Timeouts: 30 s per transition. Recovery positions are computed from the
attempted transition kind; safe fallback is the zone origin.

## Orchestrator Zone FSM

`ZoneTransitionStateMachine` (in `textquest::zoning::state`) tracks the
high-level orchestrator view:

```
Idle → Validating → Loading → InGame   (success path)
              ↓           ↓
           Failed ← ← ← ←            (unrecoverable)
              ↓
          Recovering → Validating     (up to MAX_ZONE_RETRIES = 3)
```

Key methods: `request_zone`, `validation_ok`, `loaded`, `failure`, `retry`, `reset`.

## Failure Codes and Recovery

`ZoneFailureCode` maps EQ numeric codes to recovery strategies:

| Code | Recovery Action |
|------|----------------|
| `GeneralFailure`, `SpellResisted`, `AlreadyZoning`, `ZoneLoadTimeout` | `RetryZone` |
| `TooFar`, `InvalidCoordinates`, `MovementBlocked`, `OutOfBounds` | `UseNewCoords` |
| `InsufficientMana` | `WaitManaRegen` |
| `ZoneQueueFull`, `ZoneLockedInstance` | `WaitZoneQueue` |
| `PlayerInCombat` | `WaitOutOfCombat` |
| `NoPathAvailable` | `RevalidatePath` |
| `LevelTooLow`, `LevelTooHigh`, `RaidLockoutActive`, `WrongType`, `GuildHallUnavailable` | `Abandon` |

`ZoneFailureState` tracks the code, reason, recovery action, retry count,
backoff deadline, and failure timestamp.

## Personal Relocation (Clickies and AAs)

Each character carries a `RelocationLoadout` — a list of `RelocationOptionState`
values from the global `relocation_catalog()`:

```rust
use textquest_common::nav::{relocation_catalog, RelocationOptionState};
use textquest::nav::relocate::RelocationLoadout;

let catalog = relocation_catalog();
// Find Throne of Heroes (guild hall AA):
let throne = catalog.iter().find(|o| o.id == "throne_of_heroes").unwrap();
let state = RelocationOptionState::new(throne.clone(), true, None); // ready, no cooldown

let loadout = RelocationLoadout::new(vec![state]);
let best = loadout.best_ready_option_for_zone("guildlobby");
```

Zone names are canonicalized (lowercase, trimmed) before catalog lookup.
`best_ready_option_for_zone` returns the highest-priority ready option for the
destination, preferring AAs over items when both are ready.

## Setting Up Travel for a Session

```rust
use std::collections::HashMap;
use textquest::nav::router::{GroupRouter, plan_group_travel};
use textquest::nav::relocate::{RelocationLoadout, group_ready_relocations};

// 1. Collect client IDs and class map from the roster.
let client_ids: Vec<u32> = roster.iter().map(|c| c.id).collect();
let class_map: HashMap<u32, u8> = roster.iter().map(|c| (c.id, c.class)).collect();

// 2. Build relocation loadouts from each character's AA/item state.
let loadouts: HashMap<u32, RelocationLoadout> = build_loadouts(&roster);

// 3. Create router and register resources.
let mut router = GroupRouter::new();
router.set_porters(porter_ids);
router.set_relocation_loadouts(loadouts);

// 4. Plan travel to destination.
let plans = router.plan_travel(&client_ids, &class_map, "qeynos", "highkeep");

// 5. Execute each plan — drive each client through its steps.
for plan in plans {
    execute_travel_plan(plan);
}
```

## Example: Handling a Straggler

A straggler is a client that failed to zone with the group. Re-plan for just
that client:

```rust
let straggler_ids = vec![straggler_client_id];
let class_map = [(straggler_client_id, straggler_class)].into_iter().collect();
let plans = plan_group_travel(&straggler_ids, &class_map, current_zone, target_zone);
// Single-client plan — same stagger logic, independent of the main group.
execute_travel_plan(plans.into_iter().next().unwrap());
```

## Testing

Unit tests live in module-local `mod tests` blocks inside:
- `textquest/src/nav/router.rs` — TravelPlan, GroupRouter, staggers
- `textquest/src/nav/relocate.rs` — RelocationLoadout, group_ready_relocations
- `textquest/src/nav/zone_transition.rs` — ZoneTransitionFsm states
- `textquest/src/zoning/state.rs` — ZoneTransitionStateMachine FSM
- `textquest/src/zoning/failure_codes.rs` — ZoneFailureCode → RecoveryAction

Integration tests are in `textquest/tests/integration.rs` under the
"Travel System" section (tests 10+), including:

- `group_travel_all_clients_receive_plans`
- `group_travel_stagger_step_is_first`
- `straggler_single_client_receives_solo_plan`
- `travel_plan_sequential_step_advancement`
- `group_router_with_porters_produces_plans_for_all`
- `zone_failure_*` (failure code → recovery action mapping)
- `zone_nav_fsm_*` (ZoneTransitionFsm state transitions)

Run with:

```bash
cargo test --lib                              # unit tests only (macOS)
cargo test --test integration                 # integration tests (Windows / CI)
```
