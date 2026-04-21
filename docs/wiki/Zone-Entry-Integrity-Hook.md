# Zone Entry Integrity Hook

**Module**: `textquest-dll/src/hooks/zone_entry_integrity.rs`
**Scope**: M5.5 — Server-initiated integrity defense (issue #2178)

## Purpose

On zone connect, `FUN_1402827C0` (rebased `ZONE_ENTRY_INTEGRITY = 0x140282_7C0`) builds three hashed region specs — player_name (32 bytes), spell_data, ui_strings — and reports them via opcode `0xe4b3`. Server compares received hashes against its own expectations. Any client modification that affects hashed regions (spell data manipulation, UI string rewrites) is detectable here.

## Design

- **`retour::static_detour!`** byte-patch detour on `FUN_1402827C0`. DR3 is claimed by `#2175` server memcheck responder — not available for this handler, so retour engine is used (same pattern as `hooks::fingerprint` and `hooks::file_integrity_dispatcher`).
- **Region capture** — reads the three hashed region specs from context struct at Ghidra-derived offsets; logs via `tracing::debug!` on each zone connect.
- **`SPOOF_ENABLED: AtomicBool`** — config flag for future hash substitution (no-op until we actually modify spell data or UI strings client-side).
- **`last_regions()` telemetry accessor** — returns `Option<[RegionSpec; 3]>` from the last hook invocation; usable in integration tests to verify the hook fires.
- **Passthrough default** — always calls original; never skips.

## Operator Workflow

No runtime control surface. Detour installs at DLL init, removes on `hooks::remove_all()`. To enable hash substitution: populate server-expected hashes and set `SPOOF_ENABLED` true.

## Related

- Epic: #2173 (M5.5)
- Siblings: #2175 (server memcheck), #2176 (counter watchdog), #2177 (file integrity)
