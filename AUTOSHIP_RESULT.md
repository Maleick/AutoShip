# Result: #1092 — [M11] Zone awareness: Location-based behavior constraints and mood effects

**Status:** DONE

## Changes Made

### New files
- `textquest-soul/src/zones.rs` — Full zone awareness implementation: `NpcDensity` enum, `ZoneMetadata` struct, `ZoneDatabase` (TOML-loadable lookup table), `apply_zone_constraints()` function that adjusts idle behavior weights based on zone properties and character mood.
- `config/soul_zones.toml` — Runtime zone config file (optional; loaded at coordinator startup if present).

### Modified files
- `textquest-soul/src/zone_classifier.rs` — Thinned to a single re-export (`pub use crate::zones::*;`). All logic migrated to `zones.rs`. **Confirmed legitimate** — not accidental deletion.
- `textquest-soul/src/lib.rs` — Added `pub mod zones;` module declaration.
- `textquest-soul/src/coordinator.rs` — `SoulCoordinator` now holds a `ZoneDatabase`, loads `config/soul_zones.toml` at init (tolerates missing file), and passes the DB into each `IdleScheduler` at character registration. Also fixed clippy: collapsed nested `if memory_written { if let Some(audit) }` into a single `if memory_written && let Some(audit)` guard.
- `textquest-soul/src/idle.rs` — Updated import from `zone_classifier` to `zones`; `ZoneDatabase::with_defaults()` → `ZoneDatabase::new()`; fixed borrow in `apply_zone_constraints` call; test helpers updated to use `ZoneMetadata`/`NpcDensity` directly.
- `feature-list.json` — Feature entry updated for #1092.

### Clippy fixes (rescue)
- `coordinator.rs:676` — collapsed `if A { if let Some(b) }` pattern.
- `zones.rs:174` — collapsed inner `if` into match guard on `Emote | RandomJump`.

## Tests

- `cargo check -p textquest-soul` — PASS
- `cargo test -p textquest-soul` — **488 passed** (2 suites)
- `cargo clippy -p textquest-soul --all-targets -- -D warnings` — **0 errors, 0 warnings**

## Notes

- zone_classifier.rs thinning was **legitimate**: codex-spark moved all logic into `zones.rs` (improved API with `NpcDensity` enum replacing `f32`, `ZoneMetadata` replacing `ZoneClassification`) and left a re-export shim for backward compat.
- Rescue scope: fixed two clippy errors that blocked Codex's cargo compile; no logic changes required.
- Attribution: codex-spark (initial implementation) → claude-sonnet-4-6 (cargo fix + clippy cleanup + commit).
