# Test Guide

Operator and developer guide for running and interpreting the TextQuest test suite.

---

## Overview

TextQuest uses a layered test strategy:

| Layer | Location | Tool | When it runs |
|---|---|---|---|
| Unit tests | Inline `#[cfg(test)]` blocks in source files | `cargo test` | Every PR |
| Integration tests | `textquest/tests/*.rs` | `cargo test -p textquest --test <name>` | Every PR |
| Scenario tests | `textquest/tests/scenarios/` | `cargo test scenario_` | Every PR |
| Coverage gate | `scripts/coverage-report.py` | cargo-tarpaulin | Every PR (60% baseline threshold) |
| Python tests | `tests/test_*.py` | `python3 -m unittest` | Advisory only |

---

## Running Tests

### Quick check (recommended before every push)

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all --all-features
python3 scripts/dev-preflight.py
```

The `dev-preflight.py` script bundles the above checks into a single command.

### Individual test suites

```bash
# All Rust tests across the workspace
cargo test --all --all-features

# Integration tests only (login, navigation, combat flows)
cargo test -p textquest --test integration

# Scenario tests only (end-to-end multibox workflows)
cargo test -p textquest --test integration scenario_

# Specific scenario
cargo test -p textquest --test integration scenario_solo_farming_loop -- --nocapture

# Python tests (advisory)
python3 -m unittest discover -s tests -p 'test_*.py' -v
```

### Coverage report

```bash
# Text summary only
python3 scripts/coverage-report.py

# With HTML output (opens in browser)
python3 scripts/coverage-report.py --html

# Custom threshold
python3 scripts/coverage-report.py --threshold 80
```

Requires `cargo install cargo-tarpaulin`.

---

## Test Suites

### Login FSM (`textquest/tests/integration.rs`)

Tests the `LoginStateMachine` through all 8+ states and error recovery paths.

| Test | Covers |
|---|---|
| `login_enter_world_navigate_pipeline` | Full login → post-login → navigation flow |
| `multi_client_launch_coordination` | `LaunchCoordinator` queue management |
| `coordinator_tick_attempts_launch_from_queue` | Single dequeue per tick |
| `login_wrong_password_is_fatal` | Error branch: WrongPassword → Abort |
| `login_server_full_retries_then_aborts` | Retry logic (3 attempts) |
| `login_character_mismatch_aborts` | Character name validation |
| `login_mass_failure_triggers_pause_all` | MassFailure → PauseAll action |
| `dll_reported_in_world_triggers_post_login` | DLL-reported phase transitions |
| `dll_reported_ready_sets_terminal` | Terminal state handling |

### Camp Loop / Combat FSM (`textquest/tests/integration.rs`)

Tests the `CampLoop` state machine through farming cycles.

| Test | Covers |
|---|---|
| `camp_loop_full_cycle_with_snapshot` | Idle → Pulling → Fighting → Looting → Medding → Idle |
| `camp_loop_emergency_heal_on_low_tank_hp` | Emergency heal trigger (tank HP < 20%) |
| `camp_idle_respects_healer_mana_threshold` | Mana gate on pull decisions |
| `camp_snapshot_driven_fight_to_loot_on_target_death` | Snapshot-driven state transitions |

### Navigation / Zone Routing (`textquest/tests/integration.rs`)

Tests `GroupRouter`, `TravelPlan`, and zone stagger logic.

| Test | Covers |
|---|---|
| `zone_routing_generates_staggered_travel_plans` | Per-client staggered plans |
| `zone_stagger_delays_are_within_range` | 36-client stagger bounds |
| `zone_stagger_delays_are_deterministic` | Same seed = same delays |
| `travel_plan_with_zone_transitions` | Multi-zone traversal |
| `group_router_with_porters` | Druid/wizard porter routing |

### Scenarios (`textquest/tests/scenarios/`)

End-to-end multibox workflow tests. See [Integration Scenario Testing Framework](../dev/testing-scenarios.md) for detailed documentation.

| Scenario | Description |
|---|---|
| `SoloFarmingScenario` | Single client: pull → kill → loot → med → repeat (10 cycles) |
| `GroupHealingScenario` | 6-person group: healer responds to tank HP changes |
| `ZoneRecoveryScenario` | Zone transitions with camp stability verification |

---

## CI Coverage Check

The CI pipeline enforces a **60% workspace coverage baseline**:

```yaml
- name: Run coverage (threshold 60%)
  if: steps.scope.outputs.docs_only != 'true'
  run: python3 scripts/coverage-report.py --threshold 60
```

If coverage drops below 60%, the PR fails and cannot merge. Review expectations for new and modified logic remain higher than the workspace baseline.

Coverage targets by layer:

| Layer | Target | Rationale |
|---|---|---|
| Pure logic (FSMs, parsers, data structures) | 90%+ | Highest value, runs everywhere |
| Platform-independent orchestration | 80%+ | Decision logic; OS calls in `#[cfg(windows)]` |
| Windows-only paths | Best-effort | Runs on Frostreaver self-hosted runner |
| TUI rendering | Smoke-only | Pixel-exact output is fragile |

---

## Test Fixtures

Test fixtures are defined inline in the test files using helper functions:

| Fixture | Location | Usage |
|---|---|---|
| `make_account(name, character, class)` | `textquest/tests/integration.rs` | Creates `AccountInfo` for login tests |
| `make_game_state()` | `textquest/tests/integration.rs` | Creates minimal `GameState` |
| `test_camp_config()` | `textquest/tests/integration.rs` | Crushbone camp config for farming tests |
| `test_camp_members()` | `textquest/tests/integration.rs` | 6-person group (tank, healer, CC, puller, 2 DPS) |

Example:

```rust
#[test]
fn my_test() {
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let game_state = make_game_state();
    let config = test_camp_config();
    let members = test_camp_members();
}
```

---

## Debugging Failing Tests

### Integration tests

```bash
# Run with output
cargo test -p textquest --test integration -- --nocapture

# Run a specific test
cargo test -p textquest --test integration login_enter_world_navigate_pipeline -- --nocapture
```

### Scenario tests

```bash
# Print tick-by-tick output
cargo test -p textquest --test integration scenario_ -- --nocapture 2>&1 | grep "total_ticks"
```

Add debug output in the scenario:

```rust
for (i, event) in events.iter().enumerate() {
    eprintln!("Tick {}: {} commands", i, event.commands.len());
}
```

Check camp state transitions:

```rust
eprintln!("Camp state: {:?}", ctx.camp.state);
```

---

## Adding New Tests

### Unit test

Add an inline `#[cfg(test)]` block at the bottom of the source file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_action_expected_result() {
        // Setup
        let subject = MyType::new();

        // Execute
        let result = subject.do_thing();

        // Assert
        assert_eq!(result, expected_value);
    }
}
```

See [Unit Test Template](../dev/unit-test-template.md) for patterns.

### Integration test

Add to `textquest/tests/integration.rs`:

```rust
#[test]
fn integration_flow_description() {
    // Test implementation
}
```

### Scenario test

Add to `textquest/tests/scenarios/mod.rs` and register in `textquest/tests/integration.rs`:

```rust
// In scenarios/mod.rs
pub struct MyScenario;

impl Scenario for MyScenario {
    fn setup(&self) -> ScenarioContext { ... }
    fn run(&self, ctx: &mut ScenarioContext) -> Vec<ScenarioEvent> { ... }
    fn verify(&self, events: &[ScenarioEvent]) -> ScenarioResult { ... }
    fn name(&self) -> &'static str { "My Scenario" }
}

// In integration.rs
#[test]
fn scenario_my_scenario() {
    use scenarios::{ScenarioRunner, MyScenario};
    let result = ScenarioRunner::run(&MyScenario);
    assert!(result.passed, "My scenario failed: {}", result.reason.unwrap_or_default());
}
```

---

## Platform Notes

- All integration and scenario tests are **platform-independent** (macOS/Linux/Windows).
- Tests use pure FSM APIs directly — no live EQ process, no mocks.
- Windows-only tests are gated `#[cfg(windows)]` and skipped on macOS/Linux.
- Python tests are **advisory only** (do not block merge) in CI.

---

## Coverage Reporting

TextQuest uses [cargo-tarpaulin](https://github.com/xd009642/tarpaulin) for coverage reporting.

Install:
```bash
cargo install cargo-tarpaulin
```

Run locally:
```bash
python3 scripts/coverage-report.py --threshold 60
```

The script parses tarpaulin output and exits with code 1 if coverage falls below the requested threshold.

---

## Related Documentation

- [Testing Best Practices](../dev/testing-scenarios.md)
- [Unit Test Template](../dev/unit-test-template.md)
- [Coverage Policy](../dev/coverage-policy.md)
