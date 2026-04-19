# Testing Guidelines

## Overview

This document describes the testing conventions, patterns, and practices used in TextQuest.

For coverage expectations, see [coverage-policy.md](./coverage-policy.md). For integration scenario testing, see [testing-scenarios.md](./testing-scenarios.md).

## Test Organization

### Unit Tests

Inline tests in the same module using `#[test]` and `#[cfg(test)]` modules:

```rust
// src/some_module.rs

pub fn calculate(value: i32) -> i32 {
    value * 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_doubles_value() {
        assert_eq!(calculate(5), 10);
    }
}
```

### Integration Tests

Located in `<crate>/tests/` directory. Use for tests that require the full crate compiled:

```
textquest/
├── src/
│   └── ...
└── tests/
    ├── integration.rs      # Platform-specific Windows tests
    ├── integration_cli.rs   # CLI behavior tests
    ├── box_chat.rs         # Box chat feature tests
    └── scenarios/
        └── mod.rs          # Scenario framework
```

Common crates put integration tests in `tests/` under each crate:

```
textquest-common/tests/
├── box_controller.rs
├── box_chat.rs
├── window_title.rs
└── inventory_utility_parity.rs
```

### Test Types

| Type | Location | Use For |
|------|-----------|---------|
| Unit | Inline `#[cfg(test)]` | Pure functions, type behavior |
| Integration | `tests/*.rs` | Cross-module logic, FSM transitions |
| Scenario | `tests/scenarios/` | Multi-client multibox workflows |
| Platform | `tests/*.rs` with `#[cfg(windows)]` | Windows-only APIs |

## Naming Conventions

Follow snake_case with descriptive names:

```rust
#[test]
fn login_enter_world_navigate_pipeline() { }

#[test]
fn camp_loop_full_cycle_with_snapshot() { }

#[test]
fn pause_and_mode_commands_update_client_state() { }

#[test]
fn parse_bc_route_accepts_slash_or_mq2_double_slash_payloads() { }
```

Pattern: `<feature>_<specific_behavior>` or `<state_machine>_<transition_name>`

For scenario tests, prefix with `scenario_`:

```rust
#[test]
fn scenario_solo_farming_loop() { }
```

## Test Data Creation

### Helper Functions

Define test data builders at the top of test files:

```rust
// In textquest/tests/integration.rs

fn make_account(name: &str, character: &str, class: &str) -> AccountInfo {
    AccountInfo {
        account_name: name.to_string(),
        character_name: character.to_string(),
        class_name: class.to_string(),
        level: 60,
        group_id: 1,
        server_name: "TestServer".to_string(),
    }
}

fn make_game_state() -> GameState {
    GameState {
        client_id: 1,
        local_player: None,
        target: None,
        nearby_spawns: Vec::new(),
        timestamp_ms: 0,
        nav_status: NavStatus::Idle,
        combat_status: CombatStatus::Idle,
        zone_short_name: String::new(),
        zone_long_name: String::new(),
        active_buffs: Vec::new(),
        pet: None,
        actual_version: None,
    }
}

fn test_camp_config() -> CampConfig {
    CampConfig {
        name: "crushbone_entrance".into(),
        zone: "crushbone".into(),
        camp_center: [100.0, 200.0, 0.0],
        // ... other fields with reasonable defaults
    }
}
```

### Snapshot Data

For camp loop testing, use `CampSnapshot` to simulate game state:

```rust
fn low_tank_snapshot() -> CampSnapshot {
    CampSnapshot {
        healer_mana_pct: 50.0,
        tank_hp_pct: 15.0,
        target_hp_pct: Some(50.0),
        target_is_dead: false,
        target_spawn_id: Some(9999),
        member_hp: vec![],
        member_in_combat: vec![],
    }
}

fn dead_target_snapshot() -> CampSnapshot {
    CampSnapshot {
        healer_mana_pct: 80.0,
        tank_hp_pct: 100.0,
        target_hp_pct: Some(0.0),
        target_is_dead: true,
        target_spawn_id: Some(9999),
        member_hp: vec![],
        member_in_combat: vec![],
    }
}
```

## Mocking Patterns

### Direct Construction

Most tests use direct struct construction rather than mocks:

```rust
#[test]
fn pause_and_mode_commands_update_client_state() {
    let mut state = BoxControllerClientState::new("Frostreaver".to_string());
    state.apply_command(&BoxControllerCommand::Pause);
    assert_eq!(state.mode, BoxControllerMode::Paused);
}
```

### Stubs for Platform-Specific Code

Use `#[cfg(not(windows))]` stubs for cross-platform development:

```rust
#[cfg(not(windows))]
fn send_key_to_eq(_window: &WindowState, _key: VK) { /* no-op on macOS/Linux */ }
```

### Test-Only Mocks

When external dependencies are needed, create `#[cfg(test)]` modules:

```rust
#[cfg(test)]
mod mock_external {
    pub fn create_test_process() -> TestProcess {
        // Minimal test implementation
    }
}
```

## Assertion Styles

### Standard Assertions

```rust
assert!(value.is_some());
assert_eq!(expected, actual);
assert_ne!(different_a, different_b);
```

### Pattern Matching with `matches!`

For enum and variant checking:

```rust
#[test]
fn login_fsm_starts_in_launching() {
    let mut sm = LoginStateMachine::new(1, account);
    assert!(matches!(sm.phase, LoginPhase::Launching));
}

#[test]
fn login_screen_detected_produces_send_credentials() {
    let action = sm.advance(LoginEvent::LoginScreenDetected);
    assert!(matches!(action, LoginAction::SendCredentials));
}
```

### Match with Extraction

```rust
#[test]
fn select_server_emits_correct_server_name() {
    let action = sm.advance(LoginEvent::CredentialsSent);
    match &action {
        LoginAction::SelectServer { name } => assert_eq!(name, "TestServer"),
        other => panic!("expected SelectServer, got {:?}", other),
    }
}
```

### Combined Assertions

```rust
assert!(
    events.iter().any(|e| matches!(e, SomeEvent)),
    "tick should produce events when clients are queued"
);
```

### Custom Error Messages

```rust
assert!(
    result.passed,
    "Solo farming scenario failed: {}",
    result.reason.unwrap_or_default()
);
```

## Test Examples by Module Type

### FSM/State Machine Tests

```rust
#[test]
fn login_fsm_full_happy_path() {
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut sm = LoginStateMachine::new(1, account);

    // Drive through each state transition
    sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
    assert!(matches!(sm.phase, LoginPhase::ProcessLaunching));

    sm.advance(LoginEvent::LoginScreenDetected);
    assert!(matches!(sm.phase, LoginPhase::AtLoginScreen));

    // Verify terminal state
    assert!(sm.is_terminal());
}
```

### Config Parsing Tests

```rust
#[test]
fn app_config_parses_box_chat_section() {
    let cfg: AppConfig = toml::from_str(
        r#"
[box_chat]
enabled = true
host = "192.168.1.25"
port = 3002
auto_connect = true
"#,
    ).expect("box chat config should parse");

    assert!(cfg.box_chat.enabled);
    assert_eq!(cfg.box_chat.port, 3002);
}
```

### Snapshot-Driven Tests

```rust
#[test]
fn camp_snapshot_driven_fight_to_loot_on_target_dead() {
    let mut camp = CampLoop::new(test_camp_config(), test_camp_members());

    // Drive to Fighting state
    camp.tick(None);
    assert!(matches!(camp.state, CampState::Fighting));

    // Target dies -> transition to Looting
    let snapshot = dead_target_snapshot();
    camp.tick(Some(&snapshot));
    assert!(matches!(camp.state, CampState::Looting));
}
```

### Serialization Roundtrip Tests

```rust
#[test]
fn command_roundtrip_preserves_payload() {
    let command = BoxControllerCommand::RaidAssistNum { assist_num: 2 };
    let json = serde_json::to_string(&command).expect("serialize");
    let decoded: BoxControllerCommand = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded, command);
}
```

## Running Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p textquest
cargo test -p textquest-common

# Integration tests only
cargo test --test integration

# Specific test
cargo test test_name_here

# With output
cargo test -- --nocapture

# Scenarios
cargo test scenario_
```

## Coverage

See [coverage-policy.md](./coverage-policy.md) for detailed coverage requirements:

- **New code**: 80%+ line coverage
- **Modified code**: 70%+ coverage
- **Workspace baseline**: 60%+ in CI

### Debug Tools Testing

For testing debug formatting, binding diagnostics, and observability infrastructure, see [debug-tools-testing.md](./debug-tools-testing.md).

Run coverage locally:

```bash
python3 scripts/coverage-report.py
```

## Benchmark Guidelines

For performance-critical code, add benchmark tests in `<crate>/src/perf_tests.rs`:

```rust
#[cfg(test)]
mod bench {
    use super::*;

    #[bench]
    fn bench_camp_tick(b: &mut Bencher) {
        let camp = CampLoop::new(config, members);
        b.iter(|| camp.tick(None));
    }
}
```

Run benchmarks:

```bash
cargo bench
```

## Key Patterns Summary

| Pattern | Example |
|---------|---------|
| Test data helpers | `make_account()`, `test_camp_config()` |
| Snapshot injection | `camp.tick(Some(&snapshot))` |
| FSM driving | `sm.advance(Event::Variant)` |
| State checking | `assert!(matches!(state, Variant))` |
| Command filtering | `cmds.iter().filter(\|(id, _)| *id == pid)` |