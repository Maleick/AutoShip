# AutoShip Result — Issue #1066

## Status: READY

## Files Created

- `textquest/tests/pull_system_integration.rs` — 20 integration tests covering:
  - Pull sequence (Idle → Pulling → Fighting → Looting → Medding → Idle)
  - Multi-pull (3 consecutive cycles)
  - Puller role assignment and tank fallback
  - Pull target selection: configured names, HVT, closest fallback, radius filtering
  - CC-tracked mob exclusion, player/corpse exclusion
  - Leash radius config validation
  - `return_no_aggro` suppresses /sit for members with aggro
  - Return-to-camp restarts pull cycle
  - Member death triggers Recovery state
  - Empty spawn list returns None

- `docs/guides/pull-system.md` — complete guide covering:
  - State machine diagram
  - `CampConfig` field reference table
  - `Role` pull behavior table
  - Pull target selection priority order
  - Single-pull, directional pull, burn pull, HVT pull patterns
  - Leash enforcement and radius relationships
  - Return-to-camp behavior and `return_no_aggro`
  - Mana gating
  - Recovery phase
  - Example configs (Crushbone entrance, Sebilis disco)
  - Test commands

## Verification

- `cargo check --lib -p textquest` — clean
- `cargo test --lib -p textquest` — 405 tests pass
- `cargo clippy -p textquest --lib -- -D warnings` — no issues
- Integration tests gated `#[cfg(windows)]` per project convention
