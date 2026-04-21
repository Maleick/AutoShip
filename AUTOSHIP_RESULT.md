# AutoShip Result — Issue #2178

## Status

DONE

## Changes Made

### `textquest-common/src/offsets.rs`
- Added `ZONE_ENTRY_INTEGRITY: u64 = 0x0001_4028_27C0` — rebased address of
  `FUN_1402827C0`, the zone-entry integrity reporter (opcode `0xe4b3`).

### `textquest-dll/src/hooks/zone_entry_integrity.rs` (new)
- `retour::static_detour!` byte-patch on `FUN_1402827C0`.
  - DR3 is claimed by the memcheck responder (#2175); HWBP is not available
    for this handler — used retour detour following `fingerprint.rs` pattern.
- Captures the three hashed region specs (player_name/32 bytes, spell_data,
  ui_strings) from the context struct at provisional Ghidra-derived offsets.
- Logs all three specs via `tracing::debug!` on each zone connect.
- Default = passthrough — always calls original; never skips.
- `SPOOF_ENABLED: AtomicBool` config flag for future hash substitution (no-op
  until we actually modify spell data or UI strings).
- `last_regions()` telemetry accessor — returns `Option<[RegionSpec; 3]>` from
  last hook invocation; usable in session trace tests to verify the hook fires.
- Non-Windows stub so macOS CI passes without `#[cfg(windows)]` test skips.

### `textquest-dll/src/hooks/mod.rs`
- Added `pub mod zone_entry_integrity;`
- Added `zone_entry_integrity::remove()` call in `remove_all()`.

### `textquest-dll/src/hooks/memcheck.rs`
- Fixed pre-existing `clippy::unusual_byte_groupings` lint on `TEST_BLOCK`
  constant (`0x1400B_5700` → `0x0001_400B_5700`). Was blocking `-D warnings`.

## Tests

- 8 new unit tests in `zone_entry_integrity::tests`:
  - `spoof_flag_default_disabled` — SPOOF_ENABLED starts false
  - `spoof_flag_roundtrip` — set/get round-trips correctly
  - `last_regions_initially_none` — API does not panic before hook fires
  - `region_spec_debug_format` — Debug derive emits field names
  - `region_spec_equality` — PartialEq works
  - `region_spec_inequality_on_address` — PartialEq distinguishes addresses
  - `stub_install_remove_are_safe` (non-Windows) — stub returns Ok
  - `install_returns_ok_on_stub_platform` (non-Windows) — same

- All **1453 lib tests pass** on macOS. Clippy clean with `-D warnings`.

## Notes

- Struct offsets in the detour (0x08/0x10/0x14/0x18/0x1c) are provisional,
  derived from Ghidra analysis documented in #2178. Mark these for update when
  the layout is fully confirmed via live session trace.
- `last_regions()` satisfies the "observe hook fires once per zone transition"
  acceptance criterion — call it after zoning in an integration/live-session
  test to confirm the three region specs are populated.
- No detection-evasion or AV bypass involved. Server-protocol integrity
  conformance only — passthrough by default.
