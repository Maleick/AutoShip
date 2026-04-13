# Result: #929 — Trampoline hardening integrity self-check

## Status: DONE

## Changes Made

- `textquest-dll/src/hooks/integrity.rs` (new): Core integrity module.
  - `SAFE_MODE: AtomicBool` — global flag set on check failure; IPC layer can gate on this.
  - `run_integrity_check()` — checks all 4 HWBP slots for internal consistency: active slots must have non-zero address and non-null callback; inactive slots must have zero address and null callback (stale entries are flagged as corruption).
  - `verify_hooks_or_safe_mode()` — calls `run_integrity_check()`, logs per-slot errors, sets SAFE_MODE on failure, returns `Err` with first failure description.
  - `is_safe_mode()` — read-accessor for external callers.
  - `IntegrityReport` and `SlotCheckResult` — typed report structs for diagnostics.

- `textquest-dll/src/hooks/hwbp.rs`: Added `has_callback(slot)` and `is_veh_installed()` public functions. Made `SlotEntry`, `SLOTS`, and `CALLBACKS` `pub(crate)` so integrity tests can inject corrupted state directly.

- `textquest-dll/src/hooks/mod.rs`: Registered `pub mod integrity`.

- `textquest-dll/src/lib.rs`: Added step 5.5 in `initialize()` — calls `verify_hooks_or_safe_mode()` after hook installation and before `ipc::start()`. Failure is logged at ERROR level and the DLL continues (non-fatal: operators see the log entry and can re-inject).

## Architecture Note

This codebase uses HWBP (hardware breakpoint via VEH) hooks rather than classic byte-patching trampolines. The "trampoline integrity" concept maps to verifying the HWBP slot registry — the invariant being that (active, address, callback) are mutually consistent for every slot. A corrupted entry would cause the VEH handler to dispatch to address 0 or a null function pointer, which is the equivalent threat that trampoline integrity guards against in detour-based hooking.

## Tests

- Command: `cargo test -p textquest-dll -- --test-threads=1`
- Result: PASS (913 passed, 0 failed)
- New tests added: 8 (all in `hooks::integrity::tests`)
  - `integrity_passes_when_all_slots_empty`
  - `slot_check_fails_on_active_with_zero_address`
  - `slot_check_fails_on_active_with_null_callback`
  - `slot_check_fails_on_inactive_with_stale_address`
  - `slot_check_fails_on_inactive_with_stale_callback`
  - `verify_hooks_enters_safe_mode_on_corruption`
  - `verify_hooks_ok_when_clean`
  - `register_then_check_passes` (non-Windows only)

## Notes

- **Pre-existing flaky test**: `stealth::text_encrypt::tests::xor_is_self_inverse` fails intermittently under parallel test execution (shared global text-section state races with concurrent tests). This is pre-existing — it passes in isolation and with `--test-threads=1`. CI should use `--test-threads=1` for this crate or the text_encrypt tests need a serialization mutex.
- The integrity check is cross-platform: on non-Windows builds all slots are inactive (stubs), so the clean state passes correctly.
- `SAFE_MODE` is exposed but IPC doesn't yet gate on it — a follow-up PR can add `if hooks::integrity::is_safe_mode() { return Err(...) }` in the IPC command dispatcher.
