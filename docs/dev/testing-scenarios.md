# Integration Scenario Testing Framework

This document explains how to use and extend the integration testing framework for end-to-end multibox scenarios.

## Overview

The scenario framework (`textquest/tests/scenarios/mod.rs`) provides a structured way to test complex multibox behaviors such as:

- Farming loops (pull, fight, loot, med, repeat)
- Group healing (healer responding to tank damage)
- Zone recovery (zoning and re-establishing camp)
- Multi-client coordination

## Architecture

### Key Components

#### `Scenario` Trait

Defines the contract for any testable scenario:

```rust
pub trait Scenario {
    /// Set up the initial game state and return a scenario context.
    fn setup(&self) -> ScenarioContext;

    /// Run the scenario with the given context.
    fn run(&self, ctx: &mut ScenarioContext) -> Vec<ScenarioEvent>;

    /// Verify that the scenario executed correctly.
    fn verify(&self, events: &[ScenarioEvent]) -> ScenarioResult;

    /// Human-readable name for this scenario.
    fn name(&self) -> &'static str;
}
```

#### `ScenarioContext`

Holds the mutable state during a scenario run:

- `camp`: The `CampLoop` state machine
- `clients`: Client ID → account name mapping
- `game_state`: Current game state snapshot
- `tick`: Tick counter

#### `ScenarioEvent`

Records what happened at each tick:

- `tick`: Tick number
- `commands`: Commands issued (client_id, command string)
- `snapshot`: Optional game state snapshot for verification

#### `ScenarioResult`

Outcome of a scenario:

- `passed`: Boolean success/failure
- `reason`: Failure reason (if any)
- `total_ticks`: Total ticks executed
- `summary`: Human-readable summary

#### `ScenarioRunner`

Executor that runs a scenario through its three phases:

1. **Setup**: Initialize game state and camp configuration
2. **Run**: Execute the scenario for N ticks, recording events
3. **Verify**: Check that events matched expected behavior

### Example Scenarios

#### 1. Solo Farming (`SoloFarmingScenario`)

Tests a single client repeatedly pulling and killing mobs:

- Starts in Idle state
- Pulls a mob
- Advances through PULL_DURATION
- Simulates target death (0 HP snapshot)
- Transitions to Looting
- Meds and returns to Idle
- Repeats 10 times
- Verifies at least 1 kill occurred

#### 2. Group Healing (`GroupHealScenario`)

Tests healer response to tank damage:

- Sets up a 6-person group (tank, healer, CC, puller, 2 DPS)
- Pulls a mob
- Transitions to Fighting
- Sends two snapshots:
  - High tank HP (100%) → healer should NOT cast
  - Low tank HP (15%) → healer SHOULD cast emergency heal
- Verifies correct healing behavior

#### 3. Zone Recovery (`ZoneRecoveryScenario`)

Tests camp stability across zone transitions:

- Establishes a camp in a zone
- Runs the camp loop for 5 ticks
- Marks the camp as stable
- Verifies the camp remained in a consistent state

## Survivability-Core Recovery Scenarios

Issue `TextQuest#1594` adds a design constraint for unattended automation: scenario coverage should prove that groups with the survivability core fail soft when tanks, healers, or add control break down. These do not all need to exist in the codebase yet, but they are the minimum scenario set to add as the camp loop and combat engine harden.

### 1. `TankDeathPromotesPickupScenario`

Use a group with an actual pickup tank (`PAL` or `SK`) and verify that tank death does not immediately collapse the camp.

- Start in `Fighting` with a stable target lock
- Mark the main tank dead in the next snapshot
- Expect the camp loop to suppress new pulls and emit metadata such as `pickup_tank_promoted: true`
- Verify the backup tank receives the aggro/assist commands and the healer target swaps to that actor
- Pass only if the group remains in recovery or fighting state without transitioning straight to wipe/reset

### 2. `TankDeathStallAndDisengageScenario`

Use a warrior-led group without a real backup tank and verify that the group stalls rather than pretending a DPS character can absorb the fight indefinitely.

- Start from the standard melee pod (`WAR / CLR / BRD / SHM / MNK / MNK`)
- Kill the main tank in a snapshot while the mob is still alive
- Expect new pulls to be blocked and metadata such as `stall_mode_enabled: true`
- Verify bard peel/mez or disengage commands are emitted before burn or re-engage commands
- Pass only if the scenario ends in controlled disengage, regroup, or corpse recovery instead of full-group death

### 3. `PrimaryHealerDeathSecondaryHealerTakeoverScenario`

Verify that the second healer becomes the temporary primary healer and that DPS backs off while coverage is thin.

- Start with `CLR + SHM` or `CLR + PAL` healer coverage
- Mark the cleric dead while tank HP is trending down
- Expect metadata such as `secondary_healer_takeover: true` and `new_pull_suppressed: true`
- Verify the surviving healer receives direct-heal commands and DPS receives reduced-burn or mana-light instructions
- Pass only if the tank survives long enough to finish the fight or execute an orderly disengage

### 4. `UnexpectedAddPickupScenario`

Verify that a named add, bad split, or mez resist forces control-first behavior.

- Start a normal pull, then inject a second hostile target before the kill target is dead
- Expect the camp loop to pause pull advancement and emit `unexpected_add_isolated: true` only after control lands
- Verify the first-line response is bard or CC control, with pickup-tank commands only if control fails
- Pass only if the primary target remains stable and the group does not continue normal burn while the add is loose

### 5. `CoreBrokenAbortScenario`

Verify that the automation chooses the least-loss retreat path when the survivability core is gone.

- Remove the second healer and introduce either healer death or multiple uncontrolled adds
- Expect metadata such as `recovery_floor_broken: true`
- Verify the scenario issues evac, disengage, or regroup commands instead of continuing the damage rotation
- Pass only if the system chooses orderly retreat over optimistic all-in combat

## Running Scenarios

### Run all scenario tests:

```bash
cargo test scenario_
```

### Run a specific scenario:

```bash
cargo test scenario_solo_farming_loop
```

### Run with output:

```bash
cargo test scenario_ -- --nocapture
```

## Adding a New Scenario

To add a new scenario, follow these steps:

### 1. Define Your Scenario Struct

```rust
pub struct MyCustomScenario;
```

### 2. Implement the `Scenario` Trait

```rust
impl Scenario for MyCustomScenario {
    fn setup(&self) -> ScenarioContext {
        // Initialize camp config, members, clients, game_state
        // Return ScenarioContext
    }

    fn run(&self, ctx: &mut ScenarioContext) -> Vec<ScenarioEvent> {
        // Tick the camp loop, collect events
        // Return Vec<ScenarioEvent>
    }

    fn verify(&self, events: &[ScenarioEvent]) -> ScenarioResult {
        // Inspect events and return pass/fail result
        // Example: check that at least 3 events occurred
        // Example: verify specific commands were issued
    }

    fn name(&self) -> &'static str {
        "My Custom Scenario"
    }
}
```

### 3. Add Integration Test

In `textquest/tests/integration.rs`, add a new test function:

```rust
#[test]
fn scenario_my_custom_scenario() {
    use scenarios::{Scenario, ScenarioRunner, MyCustomScenario};

    let scenario = MyCustomScenario;
    let result = ScenarioRunner::run(&scenario);

    assert!(
        result.passed,
        "My custom scenario failed: {}",
        result.reason.unwrap_or_default()
    );
}
```

## Writing Effective Verifications

### Check Event Count

```rust
if events.len() < 3 {
    return ScenarioResult::fail(events.len(), "Insufficient events".to_string());
}
```

### Inspect Command History

```rust
let heal_commands = events.iter().filter_map(|e| {
    e.commands.iter().find(|(pid, cmd)| *pid == healer_id && cmd == "/cast 1")
}).count();
```

### Check State Snapshots

```rust
let dead_events = events.iter().filter(|e| {
    if let Some(snap) = &e.snapshot {
        snap.target_hp_pct == Some(0.0)
    } else {
        false
    }
});
```

### Metadata Markers

Use the `commands` field to store verification metadata:

```rust
let metadata = vec![
    (0, format!("healer_cast_emergency_heal: {}", healed)),
    (0, format!("tank_survived: {}", tank_alive)),
];
events.push(ScenarioEvent {
    tick: ctx.tick,
    commands: metadata,
    snapshot: None,
});
```

## Common Patterns

### Simulating Combat Damage

```rust
let damage_snapshot = CampSnapshot {
    healer_mana_pct: 50.0,
    tank_hp_pct: 15.0,  // Low HP
    target_hp_pct: Some(50.0),
    target_is_dead: false,
    target_spawn_id: Some(9999),
    member_hp: vec![],
    member_in_combat: vec![],
};
let cmds = ctx.camp.tick(Some(&damage_snapshot));
```

### Advancing Through State Transitions

```rust
// Pull phase: wait PULL_DURATION ticks
for _ in 0..PULL_DURATION - 1 {
    ctx.camp.tick(None);
    ctx.tick += 1;
}

// Then tick once more to transition to Fighting
ctx.camp.tick(None);
```

### Checking for Specific Commands

```rust
let puller_cmds: Vec<_> = cmds
    .iter()
    .filter(|(pid, _)| *pid == 103)  // Puller ID
    .collect();

let has_target = puller_cmds.iter().any(|(_, cmd)| cmd.contains("/target"));
let has_attack = puller_cmds.iter().any(|(_, cmd)| cmd == "/attack");
```

## Platform-Independent Testing

All scenario tests run on macOS and Windows:

- Tests use `#[test]` (standard Rust test harness)
- No platform-specific APIs or Windows-only features
- Game state snapshots use cross-platform types
- Camp loop logic is platform-independent

## Debugging Failing Scenarios

### 1. Check Event Count

Ensure the scenario ran for expected duration:

```bash
cargo test scenario_my_test -- --nocapture 2>&1 | grep "total_ticks"
```

### 2. Print Event Details

Add debug output in your scenario:

```rust
for (i, event) in events.iter().enumerate() {
    eprintln!("Tick {}: {} commands", i, event.commands.len());
}
```

### 3. Verify State Transitions

Check that the camp loop is transitioning as expected:

```rust
eprintln!("Camp state: {:?}", ctx.camp.state);
```

### 4. Inspect Snapshots

Verify that game state snapshots are being created correctly:

```rust
if let Some(snap) = &event.snapshot {
    eprintln!("Tank HP: {}%", snap.tank_hp_pct);
}
```

## Integration with CI

All scenario tests are run as part of the standard test suite:

```bash
cargo test                    # Runs all tests including scenarios
cargo test -p textquest      # Runs orchestrator tests including scenarios
```

The CI pipeline (`.github/workflows/`) automatically runs these tests on every push.

## Future Extensions

Possible enhancements to the framework:

1. **Replay recording**: Save command sequences to files for deterministic replay
2. **Performance metrics**: Track command execution latency, state transition times
3. **Visual debugging**: Generate ASCII diagrams of camp state transitions
4. **Chaos testing**: Randomly inject network failures, combat interruptions
5. **Multi-scenario orchestration**: Run 2+ scenarios in parallel to test client coordination
