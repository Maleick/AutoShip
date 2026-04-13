# Testing Best Practices — TextQuest

Guidelines for writing, naming, and maintaining tests across the four workspace
crates (`textquest`, `textquest-common`, `textquest-dll`, `textquest-web`).

---

## Test Naming Conventions

### Format

```
<unit>_<action>_<expected_result>
```

All three segments are required. Use snake_case throughout.

| Segment             | Meaning                                   | Examples                                                                    |
| ------------------- | ----------------------------------------- | --------------------------------------------------------------------------- |
| `<unit>`            | The function, type, or concept under test | `push`, `dll_module_name`, `parse_navmesh`, `login_sm`                      |
| `<action>`          | What is being done or what input is given | `and_len`, `requires_dll_extension`, `rejects_bad_magic`, `process_started` |
| `<expected_result>` | The observable outcome                    | `returns_empty`, `returns_error`, `transitions_to_launching`                |

### Examples

```
push_and_len
ida_pattern_all_wildcards
dll_module_name_requires_dll_extension
parse_rejects_bad_magic
initial_state_is_not_started
process_started_transitions_to_launching
bot_state_lockout_tracking
shutdown_signal_stops_loop
```

### Anti-patterns to avoid

```
// Too vague — what is being tested?
test_push()
test_error()
it_works()

// Describes implementation, not behavior
test_vec_push_increments_len_field()

// Missing the expected result
dll_module_name_with_bad_extension()
```

---

## Test Organization

- All unit tests live in an inline `#[cfg(test)] mod tests { … }` block at the
  bottom of the source file they test. No separate `tests/` directory for unit
  tests.
- Integration tests (tests that require multiple modules or real I/O) go in
  `textquest/tests/` as separate `*.rs` files and are invoked with
  `cargo test -p textquest --test <name>`.
- Python behavioral tests live in `tests/test_*.py` and run via
  `python3 -m unittest discover -s tests -p 'test_*.py' -v`.

---

## Mock vs. Real — When to Use Each

### Use real implementations by default

TextQuest tests favor real, lightweight implementations over mocks. The project
deliberately designs units so they can be exercised with plain data structures
and no external dependencies:

- State machines (`LoginStateMachine`, `OrchestratorLoop`) are tested by
  feeding events directly — no mock process handles needed.
- Data structures (`FleetEventLog`, `QuestTracker`, `Pattern`) are tested with
  in-memory values — no I/O stubs needed.
- File-system code uses `tempfile::tempdir()` — a real temp directory, not a
  mock filesystem.

### Use stubs/cfg gates instead of mocks for OS APIs

All Windows-specific code is behind `#[cfg(windows)]`. On macOS the stubs
return dummy values. Do not write mock wrappers to simulate OS behavior —
simply gate the test:

```rust
#[cfg(windows)]
#[test]
fn validate_dll_path_rejects_nonexistent_file() { … }
```

Tests that genuinely cannot run on macOS are gated `#[cfg(windows)]` — they
run in CI on Frostreaver (self-hosted Windows runner) and are simply skipped
locally.

### When a mock/stub IS appropriate

| Situation                        | Approach                                                                                  |
| -------------------------------- | ----------------------------------------------------------------------------------------- |
| Async channel or watch channel   | Create a real `tokio::sync::watch::channel` in the test                                   |
| Named pipe / shared memory       | Skip or gate `#[cfg(windows)]`; do not mock the OS                                        |
| External service (Discord, HTTP) | Gate behind a feature flag or integration test; do not mock in unit tests                 |
| Time-dependent behavior          | Inject a `Duration` or timestamp parameter; avoid `std::time::SystemTime::now()` in logic |

---

## Coverage Targets

TextQuest does not enforce a hard line-coverage percentage in CI, but the
following targets guide what to test:

| Layer                                                 | Target                       | Rationale                                                        |
| ----------------------------------------------------- | ---------------------------- | ---------------------------------------------------------------- |
| Pure logic (parsers, state machines, data structures) | 90%+ branch coverage         | These are the highest-value tests; easy to run everywhere        |
| Platform-independent orchestration                    | 80%+                         | Test the decision logic; leave the OS calls to `#[cfg(windows)]` |
| Windows-only paths                                    | Best-effort                  | Run on Frostreaver; aim for every error branch to have a test    |
| TUI rendering                                         | Smoke-only                   | Verify no panic on common inputs; pixel-exact output is fragile  |
| IPC wire protocol                                     | All encode/decode roundtrips | Serialization bugs are silent and expensive to debug             |

Run the full suite before pushing:

```bash
cargo test                    # all crates
python3 scripts/dev-preflight.py   # fmt + clippy + test + python (same as CI)
```

---

## Platform Discipline

- Use `#[cfg(windows)]` / `#[cfg(not(windows))]` — never `#[cfg(target_os = "windows")]`.
- Apply `#[cfg(windows)]` to both the `use` import and the `#[test]` function
  when the test requires Windows-only symbols.
- Tests gated `#[cfg(windows)]` are **not** skipped failures — they simply do
  not compile or run on macOS. This is intentional and expected.

---

## Async Tests

- Use `#[tokio::test]` for any `async fn` test.
- Do not add `#[tokio::main]` — that is for binaries.
- `tokio` is already in `dev-dependencies` with `features = ["full", "test-util"]`.
- For time-sensitive async tests, prefer `tokio::time::pause()` +
  `tokio::time::advance()` over real sleeps.

---

## Assertion Style

| Situation         | Preferred form                                                 |
| ----------------- | -------------------------------------------------------------- |
| Equality          | `assert_eq!(actual, expected, "message")`                      |
| Boolean condition | `assert!(condition, "message")`                                |
| Enum variant      | `assert!(matches!(value, Enum::Variant))`                      |
| Expected error    | `result.unwrap_err().to_string().contains("phrase")`           |
| Just fails        | `assert!(result.is_err())`                                     |
| Floating point    | `(actual - expected).abs() < 1e-6` (no `assert_eq!` on floats) |

Always include a descriptive failure message for non-obvious assertions:

```rust
assert!(quest.objectives[0].completed, "objective should be complete after full progress");
```

---

## Do Not

- Do not use `println!` / `eprintln!` inside `#[cfg(test)]` blocks. Use
  `dbg!()` for transient debugging and remove before committing.
- Do not use the `log` crate in tests. Logging goes through `tracing`; use
  `tracing_subscriber::fmt::init()` only in integration tests when you need
  log output.
- Do not `unwrap()` in Setup unless failure is truly impossible. Prefer
  `expect("descriptive message")` so test failures have context.
- Do not share mutable state across tests via `static`. Each test must be
  self-contained and order-independent.
- Do not sleep (`std::thread::sleep`) in tests. Use `tokio::time` test utilities
  or redesign to avoid time coupling.
