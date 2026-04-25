# Result: #1154 — #870: Testing Infrastructure - Mocks & Stubs [PARENT]
Status: DONE

Changes:
- Added macOS-safe process lifecycle abstractions in `textquest/src/testing/mocks.rs`:
  - `MockProcessState` and `MockProcessLifecycle` trait
  - `MockProcess` stub with `start` / `stop` / state helpers
- Added stubbed scenario helpers in `textquest/src/testing/scenario.rs`:
  - `MockScenario`, `CountdownScenario`, `FastFailScenario`
  - Shared test-data generators: `account_info`, `spawn_entry`, `spawn_wave`
- Updated `docs/wiki/Testing.md` with issue #870 usage notes for scenario stubs and test-data generators.

Validation:
- `cargo check`

Next steps:
- If desired, add call-site fixture usage in integration test binaries (`textquest/tests/*`) to consume the new helpers.

COMPLETE
