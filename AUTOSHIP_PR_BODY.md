Closes #2178.

Implements **M5.5 A4 — Zone entry integrity handler** for opcode `0xe4b3`.

## Summary

- `textquest-dll/src/hooks/zone_entry_integrity.rs` (new, 369 lines): `retour::static_detour!` byte-patch on `FUN_1402827C0` (rebased `ZONE_ENTRY_INTEGRITY = 0x1402827C0`). Captures three hashed region specs (player_name/32 bytes, spell_data, ui_strings) on zone connect; logs via `tracing::debug!`.
- `textquest-common/src/offsets.rs`: new `ZONE_ENTRY_INTEGRITY` constant.
- `SPOOF_ENABLED: AtomicBool` config flag for future hash substitution; passthrough default.
- `last_regions()` telemetry accessor for integration tests.

**DR3 is claimed by `#2175` server memcheck responder** — this handler uses retour detour, not HWBP. Pattern matches `hooks::fingerprint` and `hooks::file_integrity_dispatcher`.

## Tests

All existing tests pass. Non-Windows stub so macOS CI doesn't need `#[cfg(windows)]` skips.

## Notes

Provisional Ghidra offsets for context-struct region fields — verify via live session on Frostreaver before enabling `SPOOF_ENABLED`.

---
🤖 AutoShip dispatch — Sonnet worker, $(date -u +%Y-%m-%d)
