# Unit Test Template — TextQuest

Reusable patterns for writing unit tests in this codebase. All examples are
drawn from live production tests.

---

## Anatomy of a Test Module

Every source file places its tests in an inline `#[cfg(test)]` module at the
bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;        // bring the file's public + private items into scope

    // helper functions, test cases …
}
```

Windows-only code is additionally gated:

```rust
#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::some_windows_fn;

    #[cfg(windows)]
    #[test]
    fn windows_only_case() { … }
}
```

---

## Setup / Execute / Assert (SEA) Pattern

All tests follow the **Setup → Execute → Assert** rhythm. Use blank lines to
separate the three phases for readability:

```rust
#[test]
fn update_objective_marks_objective_complete() {
    // Setup
    let objs = vec![QuestObjective::new("Kill 5 rats", 5)];
    let (mut tracker, qid) = make_tracker_with_quest(objs);

    // Execute
    tracker.update_objective(qid, 0, 5);

    // Assert
    let quest = tracker.quests.iter().find(|q| q.id == qid).unwrap();
    assert!(quest.objectives[0].completed, "objective should be complete");
}
```

---

## Helper / Builder Functions

Extract repeated construction into free functions — not `Default::default()` or
`new()` calls scattered across tests. Name them after what they build:

```rust
fn make_kill(pid: u32, ts: i64) -> FleetEvent {
    FleetEvent::Kill {
        source_pid: pid,
        target_name: "mob".into(),
        target_level: 50,
        zone: "gfaydark".into(),
        timestamp: ts,
    }
}

fn new_sm() -> LoginStateMachine {
    LoginStateMachine::new(1, test_account())
}
```

---

## Happy-Path Template

```rust
#[test]
fn <unit>_<action>_<expected_result>() {
    // Setup — construct minimal valid inputs
    let mut subject = MyType::new(…);

    // Execute — call the function under test
    let result = subject.do_thing(input);

    // Assert — check observable outcomes
    assert_eq!(result, expected_value);
    assert!(subject.field == post_condition, "descriptive failure message");
}
```

Real example:

```rust
#[test]
fn push_and_len() {
    let mut log = FleetEventLog::new(10);
    assert!(log.is_empty());

    log.push(make_kill(1, 100));

    assert_eq!(log.len(), 1);
    assert!(!log.is_empty());
}
```

---

## Error-Case Template

Use `.unwrap_err()` to capture and inspect the `Err` variant without panicking
on the happy path. Assert on the error message string:

```rust
#[test]
fn <unit>_rejects_<bad_input>() {
    // Setup
    let bad_input = …;

    // Execute
    let err = function_under_test(bad_input).unwrap_err();

    // Assert — check the error message contains the key phrase
    assert!(err.to_string().contains("expected phrase"), "{err}");
}
```

Real example:

```rust
#[cfg(windows)]
#[test]
fn dll_module_name_requires_dll_extension() {
    let err = dll_module_name(Path::new(r"C:\temp\textquest_dll")).unwrap_err();
    assert!(err.to_string().contains("must end with .dll"));
}
```

For `Result`-returning functions, prefer `is_err()` when you only care that the
call failed:

```rust
#[test]
fn parse_rejects_too_small() {
    let result = parse_navmesh(&[0; 4]);
    assert!(result.is_err());
}
```

---

## Edge-Case Template

Cover boundary conditions: empty collections, zero values, max values, repeated
operations, and "impossible" inputs that should be handled gracefully.

```rust
#[test]
fn ida_pattern_all_wildcards() {
    let p = Pattern::from_ida("?? ?? ??");
    assert_eq!(p.len(), 3);
    assert!(p.mask.iter().all(|&m| !m));
}

#[test]
fn ida_pattern_no_wildcards() {
    let p = Pattern::from_ida("48 89 5C 24 08");
    assert_eq!(p.len(), 5);
    assert!(p.mask.iter().all(|&m| m));
}
```

---

## Async Tests — `#[tokio::test]`

Use `#[tokio::test]` (not `#[test]`) for any `async fn`. Import `tokio` only
in `dev-dependencies` (it is already present via `tokio = { features = ["full",
"test-util"] }` in the workspace).

```rust
#[tokio::test]
async fn <unit>_<action>_<expected>() {
    // Setup
    let state = BotState { … };

    // Execute
    state.lockouts.write().await.insert("Toon".into(), lockout);

    // Assert
    let guard = state.lockouts.read().await;
    assert_eq!(guard.len(), 1);
}
```

Real example:

```rust
#[tokio::test]
async fn bot_state_lockout_tracking() {
    let state = BotState {
        lockouts: RwLock::new(HashMap::new()),
        spawn_events: RwLock::new(Vec::new()),
        bridge: None::<SyncBridge>,
        guild_id: 0,
    };

    let lo = DzLockout { … };
    state.lockouts.write().await
        .entry("Toonname".to_string())
        .or_default()
        .push(lo);

    let guard = state.lockouts.read().await;
    assert_eq!(guard["Toonname"].len(), 1);
}
```

---

## Temporary Files

Use `tempfile::tempdir()` for tests that touch the filesystem. The `TempDir`
guard auto-removes the directory on drop:

```rust
use tempfile::tempdir;

#[test]
fn saves_and_loads_correctly() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("markers.json");

    save_named_markers(&path, &sample_markers()).unwrap();

    let loaded = load_named_markers(&path).unwrap();
    assert_eq!(loaded.len(), sample_markers().len());
}
```

---

## State Machine Tests

State machines are tested by feeding events and asserting on the resulting
phase and returned action:

```rust
#[test]
fn initial_state_is_not_started() {
    let sm = new_sm();
    assert!(matches!(sm.phase, LoginPhase::NotStarted));
    assert_eq!(sm.attempts, 0);
    assert!(!sm.is_terminal());
}

#[test]
fn process_started_transitions_to_launching() {
    let mut sm = new_sm();
    let action = sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
    assert!(matches!(sm.phase, LoginPhase::Launching));
    assert!(action.is_some());
}
```

---

## Quick Reference

| Scenario         | Pattern                                                  |
| ---------------- | -------------------------------------------------------- |
| Happy path       | `assert_eq!` / `assert!` with descriptive messages       |
| Expected error   | `.unwrap_err()` + `assert!(err.to_string().contains(…))` |
| Just fails       | `assert!(result.is_err())`                               |
| Async code       | `#[tokio::test]` + `async fn`                            |
| Filesystem       | `tempfile::tempdir()` — auto-cleanup on drop             |
| Windows-only     | `#[cfg(windows)]` on both `use` and `#[test]`            |
| State machine    | Feed event, assert `.phase` and returned action          |
| Pattern matching | `assert!(matches!(value, Pattern::Variant))`             |
