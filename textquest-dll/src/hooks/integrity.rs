//! Hook integrity self-check — verifies HWBP slot state before IPC activation.
//!
//! Called during DLL initialization after hooks are installed and before the
//! IPC command listener starts accepting commands. If any slot is in an
//! inconsistent state (active flag set without a valid address or callback, or
//! vice-versa), the check fails and the DLL enters safe mode: no hook callbacks
//! will be dispatched and IPC commands are rejected.
//!
//! "Trampoline integrity" in the context of this codebase means verifying
//! the HWBP (hardware-breakpoint) hook registry — since we use VEH-based
//! hardware breakpoints rather than byte-patching trampolines, the "trampoline"
//! is the combination of (slot address, callback pointer, active flag). A
//! corrupted entry is one where these three fields are mutually inconsistent.
//!
//! # OUTBOUND_MSG_COUNTER hook (A2)
//!
//! This module also owns the HWBP data-write hook for `OUTBOUND_MSG_COUNTER`
//! (issue #3398, parent #2173 A2). The hook fires whenever EQ writes the
//! outbound message counter and snapshots the new value via an atomic store.
//! During any frame decrypt/re-encrypt cycle that temporarily corrupts the
//! counter, callers can read [`last_outbound_counter`] and, if necessary,
//! write the preserved value back before the heartbeat function reads it.
//!
//! The hook does **not** modify the counter in-place — it only observes. This
//! keeps the hook behaviour invisible: no counter drift, no write-back latency,
//! no crash risk from mid-frame re-entry.
//!
//! ## Usage
//!
//! ```text
//! // Install during DLL init (after rebasing):
//! outbound_counter_hook::install(rebase(offsets::OUTBOUND_MSG_COUNTER))?;
//!
//! // Read the last observed value at any time:
//! let v = outbound_counter_hook::last_outbound_counter();
//!
//! // Shutdown:
//! outbound_counter_hook::remove();
//! ```

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use super::hwbp::{self, MAX_SLOTS};

/// Global safe-mode flag. Set to `true` when the integrity check fails.
/// When in safe mode, the IPC layer rejects all incoming commands.
pub static SAFE_MODE: AtomicBool = AtomicBool::new(false);

/// Result of a single slot integrity check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotCheckResult {
    pub slot_index: usize,
    pub active: bool,
    pub address: usize,
    pub has_callback: bool,
    pub ok: bool,
    pub reason: Option<&'static str>,
}

/// Result of the full hook integrity check.
#[derive(Debug, Clone)]
pub struct IntegrityReport {
    pub passed: bool,
    pub slots: Vec<SlotCheckResult>,
    pub veh_installed: bool,
}

impl IntegrityReport {
    /// Returns the list of failed slot checks.
    pub fn failures(&self) -> impl Iterator<Item = &SlotCheckResult> {
        self.slots.iter().filter(|s| !s.ok)
    }
}

/// Check a single HWBP slot for consistency.
///
/// A slot is consistent when:
/// - If `active` is true: `address != 0` AND `has_callback` is true
/// - If `active` is false: `address == 0` AND `has_callback` is false
///   (address/callback may be non-zero temporarily during registration, but
///   after `remove_all()` they must be zero)
///
/// We treat an active slot with zero address or missing callback as corrupted —
/// the VEH handler would dispatch to address 0 or a null function pointer.
///
/// We treat an inactive slot with a non-zero address or a live callback as
/// a leaked/stale entry — indicates `clear_slot_state` was not called.
fn check_slot(idx: usize) -> SlotCheckResult {
    let Some(slot) = hwbp::HwbpSlot::from_index(idx) else {
        return SlotCheckResult {
            slot_index: idx,
            active: false,
            address: 0,
            has_callback: false,
            ok: false,
            reason: Some("invalid slot index"),
        };
    };

    let active = hwbp::is_active(slot);
    let address = hwbp::get_address(slot);
    let has_callback = hwbp::has_callback(slot);

    let (ok, reason) = if active {
        if address == 0 {
            (false, Some("slot active but address is zero"))
        } else if !has_callback {
            (false, Some("slot active but callback is null"))
        } else {
            (true, None)
        }
    } else {
        // Inactive slots must have zero address and no callback after cleanup.
        if address != 0 {
            (
                false,
                Some("slot inactive but address is non-zero (stale entry)"),
            )
        } else if has_callback {
            (
                false,
                Some("slot inactive but callback is non-null (stale entry)"),
            )
        } else {
            (true, None)
        }
    };

    SlotCheckResult {
        slot_index: idx,
        active,
        address,
        has_callback,
        ok,
        reason,
    }
}

/// Run the full hook integrity self-check.
///
/// Checks every HWBP slot for internal consistency. Returns an
/// `IntegrityReport` describing per-slot results and whether the overall check
/// passed.
///
/// This is designed to run cross-platform: on non-Windows builds all slots are
/// inactive (stubs), so all slots must have zero address and no callback —
/// that is a valid state and the check passes.
pub fn run_integrity_check() -> IntegrityReport {
    let veh_installed = hwbp::is_veh_installed();
    let slots: Vec<SlotCheckResult> = (0..MAX_SLOTS).map(check_slot).collect();
    let passed = slots.iter().all(|s| s.ok);

    IntegrityReport {
        passed,
        slots,
        veh_installed,
    }
}

/// Run the integrity check and enter safe mode on failure.
///
/// Returns `Ok(())` if the check passed, or `Err` with a description of
/// the first failure. On failure, `SAFE_MODE` is set to `true`.
///
/// Call this during DLL initialization **after** `install_hooks()` returns
/// and **before** `ipc::start()` so the IPC layer can gate on `SAFE_MODE`.
pub fn verify_hooks_or_safe_mode() -> Result<(), String> {
    let report = run_integrity_check();

    if report.passed {
        tracing::info!(
            active_slots = report.slots.iter().filter(|s| s.active).count(),
            veh_installed = report.veh_installed,
            "Hook integrity check passed"
        );
        return Ok(());
    }

    // Enter safe mode immediately.
    SAFE_MODE.store(true, Ordering::Release);

    // Log every failure for diagnostics.
    for failure in report.failures() {
        tracing::error!(
            slot = failure.slot_index,
            active = failure.active,
            address = format!("{:#x}", failure.address),
            has_callback = failure.has_callback,
            reason = failure.reason.unwrap_or("unknown"),
            "Hook integrity check FAILED"
        );
    }

    let first = report
        .failures()
        .next()
        .expect("passed is false so at least one failure exists");
    Err(format!(
        "Hook integrity check failed on slot {}: {}",
        first.slot_index,
        first.reason.unwrap_or("unknown")
    ))
}

/// Returns `true` if the DLL is currently in safe mode (integrity check
/// failed).
pub fn is_safe_mode() -> bool {
    SAFE_MODE.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------
// OUTBOUND_MSG_COUNTER HWBP hook (issue #3398 — A2)
// ---------------------------------------------------------------------------

/// Snapshot of the outbound message counter as last seen by the HWBP hook.
///
/// Written atomically by [`outbound_counter_hook::callback`] each time EQ
/// updates `OUTBOUND_MSG_COUNTER`. Initialized to `0`; callers must treat `0`
/// as "not yet observed" until the hook has fired at least once.
///
/// Atomic ordering note: the callback stores with `Release`; readers should
/// load with `Acquire` to establish a happens-before edge.
static LAST_OUTBOUND_COUNTER: AtomicI32 = AtomicI32::new(0);

/// Returns the last value written to `OUTBOUND_MSG_COUNTER` as captured by
/// the HWBP hook, or `0` if the hook has not fired yet this session.
///
/// Thread-safe; may be called from any thread including the VEH handler.
pub fn last_outbound_counter() -> i32 {
    LAST_OUTBOUND_COUNTER.load(Ordering::Acquire)
}

/// HWBP data-write hook for `OUTBOUND_MSG_COUNTER`.
///
/// Installs and removes the hardware breakpoint that observes writes to the
/// outbound message counter global.
pub mod outbound_counter_hook {
    use super::super::hwbp;
    use super::LAST_OUTBOUND_COUNTER;
    use std::sync::atomic::Ordering;

    /// HWBP callback invoked by the VEH handler each time `OUTBOUND_MSG_COUNTER`
    /// is written.
    ///
    /// # Safety / re-entry
    ///
    /// Called from within the VEH exception handler on the thread that triggered
    /// the write breakpoint (EQ's main game thread). The function must be
    /// signal-safe: no heap allocation, no mutex acquisition, no recursion.
    ///
    /// The counter address is passed as a raw pointer to the watched memory
    /// location. We read the 32-bit value at that address and snapshot it.
    ///
    /// Returns `true` to resume execution (EXCEPTION_CONTINUE_EXECUTION).
    pub fn callback(ctx: *mut ()) -> bool {
        // ctx is the address of OUTBOUND_MSG_COUNTER (a 32-bit signed integer).
        // SAFETY: the address was validated at registration time; the pointer
        // remains valid for the lifetime of the EQ process. We read only —
        // no write-back, no side-effects on the counter value.
        if !ctx.is_null() {
            // SAFETY: ctx is the address of a valid i32 global in eqgame.exe.
            let value = unsafe { (ctx as *const i32).read_volatile() };
            LAST_OUTBOUND_COUNTER.store(value, Ordering::Release);
            tracing::trace!(counter = value, "OUTBOUND_MSG_COUNTER write observed");
        }
        // Return true → EXCEPTION_CONTINUE_EXECUTION
        true
    }

    /// Install the HWBP data-write breakpoint on `OUTBOUND_MSG_COUNTER`.
    ///
    /// `counter_addr` must be the rebased runtime address of
    /// `offsets::OUTBOUND_MSG_COUNTER`.
    ///
    /// On non-Windows or in tests the HWBP registration is a no-op stub that
    /// always succeeds (DR registers are not touched).
    pub fn install(counter_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        hwbp::register_available(counter_addr, callback).map(|outcome| {
            tracing::info!(
                addr = format!("{:#x}", counter_addr),
                slot = format!("{outcome:?}"),
                "OUTBOUND_MSG_COUNTER HWBP hook installed"
            );
        })
    }

    /// Remove the HWBP for `OUTBOUND_MSG_COUNTER` if it was installed.
    ///
    /// Scans all active slots for the one whose address matches
    /// `counter_addr` and unregisters it. Safe to call even if the hook was
    /// never installed (no-op in that case).
    pub fn remove(counter_addr: usize) {
        use super::super::hwbp::MAX_SLOTS;
        use super::super::hwbp::HwbpSlot;
        for idx in 0..MAX_SLOTS {
            if let Some(slot) = HwbpSlot::from_index(idx) {
                if hwbp::get_address(slot) == counter_addr {
                    if let Err(e) = hwbp::unregister(slot) {
                        tracing::warn!(
                            addr = format!("{:#x}", counter_addr),
                            error = %e,
                            "Failed to unregister OUTBOUND_MSG_COUNTER HWBP"
                        );
                    } else {
                        tracing::info!(
                            addr = format!("{:#x}", counter_addr),
                            "OUTBOUND_MSG_COUNTER HWBP hook removed"
                        );
                    }
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::hwbp::{self, CALLBACKS, HwbpSlot, SLOTS};
    use std::sync::atomic::Ordering;

    /// Helper: acquire the HWBP serialization mutex, clear all slots, and reset
    /// the SAFE_MODE flag. The returned guard must be held for the entire test
    /// body to prevent concurrent tests from racing on shared global state.
    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = hwbp::test_guard();
        hwbp::remove_all();
        SAFE_MODE.store(false, Ordering::Release);
        guard
    }

    #[test]
    fn integrity_passes_when_all_slots_empty() {
        let _guard = setup();
        let report = run_integrity_check();
        assert!(report.passed, "empty registry should pass integrity check");
        assert_eq!(report.slots.len(), MAX_SLOTS);
    }

    #[test]
    fn slot_check_fails_on_active_with_zero_address() {
        let _guard = setup();
        // Force an inconsistent state: active=true, address=0, no callback.
        let idx = HwbpSlot::Dr0 as usize;
        SLOTS[idx].active.store(true, Ordering::Release);
        SLOTS[idx].address.store(0, Ordering::Release);
        CALLBACKS[idx].store(0, Ordering::Release);

        let result = check_slot(idx);
        assert!(!result.ok);
        assert_eq!(result.reason, Some("slot active but address is zero"));
    }

    #[test]
    fn slot_check_fails_on_active_with_null_callback() {
        let _guard = setup();
        let idx = HwbpSlot::Dr1 as usize;
        SLOTS[idx].active.store(true, Ordering::Release);
        SLOTS[idx].address.store(0xDEAD_BEEF, Ordering::Release);
        CALLBACKS[idx].store(0, Ordering::Release); // null callback

        let result = check_slot(idx);
        assert!(!result.ok);
        assert_eq!(result.reason, Some("slot active but callback is null"));
    }

    #[test]
    fn slot_check_fails_on_inactive_with_stale_address() {
        let _guard = setup();
        let idx = HwbpSlot::Dr2 as usize;
        SLOTS[idx].active.store(false, Ordering::Release);
        SLOTS[idx].address.store(0xCAFE_BABE, Ordering::Release);
        CALLBACKS[idx].store(0, Ordering::Release);

        let result = check_slot(idx);
        assert!(!result.ok);
        assert_eq!(
            result.reason,
            Some("slot inactive but address is non-zero (stale entry)")
        );
    }

    #[test]
    fn slot_check_fails_on_inactive_with_stale_callback() {
        let _guard = setup();
        let idx = HwbpSlot::Dr3 as usize;
        fn dummy(_: *mut ()) -> bool {
            false
        }
        SLOTS[idx].active.store(false, Ordering::Release);
        SLOTS[idx].address.store(0, Ordering::Release);
        CALLBACKS[idx].store(dummy as *const () as usize, Ordering::Release);

        let result = check_slot(idx);
        assert!(!result.ok);
        assert_eq!(
            result.reason,
            Some("slot inactive but callback is non-null (stale entry)")
        );
    }

    #[test]
    fn verify_hooks_enters_safe_mode_on_corruption() {
        let _guard = setup();
        // Corrupt slot Dr0: active with valid address but null callback.
        let idx = HwbpSlot::Dr0 as usize;
        SLOTS[idx].active.store(true, Ordering::Release);
        SLOTS[idx].address.store(0x1234, Ordering::Release);
        CALLBACKS[idx].store(0, Ordering::Release);

        let result = verify_hooks_or_safe_mode();
        assert!(result.is_err(), "should fail with corrupted slot");
        assert!(is_safe_mode(), "should be in safe mode after failure");
    }

    #[test]
    fn verify_hooks_ok_when_clean() {
        let _guard = setup();
        let result = verify_hooks_or_safe_mode();
        assert!(result.is_ok(), "clean registry should pass");
        assert!(
            !is_safe_mode(),
            "should not be in safe mode after passing check"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn register_then_check_passes() {
        let _guard = setup();
        fn dummy(_: *mut ()) -> bool {
            true
        }
        // On non-Windows, register() is a stub that sets slot state.
        hwbp::register(HwbpSlot::Dr0, 0x4000, dummy)
            .expect("register should succeed on non-windows stub");
        let report = run_integrity_check();
        assert!(
            report.passed,
            "registered valid slot should pass integrity check"
        );
    }

    // -----------------------------------------------------------------------
    // OUTBOUND_MSG_COUNTER hook tests (issue #3398 — A2)
    // -----------------------------------------------------------------------

    /// Verify the callback snapshots the value at the pointed-to address and
    /// stores it in LAST_OUTBOUND_COUNTER.
    #[test]
    fn counter_callback_snapshots_value() {
        use super::outbound_counter_hook;
        use super::LAST_OUTBOUND_COUNTER;

        // Reset the global snapshot.
        LAST_OUTBOUND_COUNTER.store(0, Ordering::Release);

        // Simulate a counter write: place a known value in a stack variable
        // and pass its address as the ctx pointer (mimicking the VEH handler).
        let counter_value: i32 = 0x37;
        let ctx = &counter_value as *const i32 as *mut ();
        let continued = outbound_counter_hook::callback(ctx);

        assert!(continued, "callback must return true (continue execution)");
        assert_eq!(
            last_outbound_counter(),
            0x37,
            "snapshot should match the counter value written"
        );
    }

    /// Verify the callback is a no-op and does not panic when passed a null
    /// pointer (defensive guard for mid-frame edge cases).
    #[test]
    fn counter_callback_null_pointer_is_safe() {
        use super::outbound_counter_hook;
        use super::LAST_OUTBOUND_COUNTER;

        LAST_OUTBOUND_COUNTER.store(0x55, Ordering::Release);
        let continued = outbound_counter_hook::callback(std::ptr::null_mut());

        assert!(continued, "null-pointer callback must still return true");
        // Snapshot must not change on a null pointer.
        assert_eq!(
            last_outbound_counter(),
            0x55,
            "snapshot should be unchanged after null-pointer callback"
        );
    }

    /// Verify the counter snapshot is preserved correctly across multiple
    /// simulated frame boundaries (write → read → write → read).
    #[test]
    fn counter_preserved_across_frame_boundaries() {
        use super::outbound_counter_hook;
        use super::LAST_OUTBOUND_COUNTER;

        LAST_OUTBOUND_COUNTER.store(0, Ordering::Release);

        // Frame 1: EQ writes 0x10 to the counter.
        let frame1: i32 = 0x10;
        outbound_counter_hook::callback(&frame1 as *const i32 as *mut ());
        assert_eq!(last_outbound_counter(), 0x10, "after frame 1 write");

        // Frame 2: EQ decrements to 0x09.
        let frame2: i32 = 0x09;
        outbound_counter_hook::callback(&frame2 as *const i32 as *mut ());
        assert_eq!(last_outbound_counter(), 0x09, "after frame 2 write");

        // Frame 3: heartbeat refills to 0x10 + 0x37 = 0x47 (typical EQ refill).
        let frame3: i32 = 0x47;
        outbound_counter_hook::callback(&frame3 as *const i32 as *mut ());
        assert_eq!(last_outbound_counter(), 0x47, "after heartbeat refill");
    }

    /// Verify install/remove round-trip does not panic on the non-Windows stub.
    #[cfg(not(windows))]
    #[test]
    fn outbound_counter_hook_install_remove_stub() {
        let _guard = setup();
        let fake_addr: usize = 0x0001_40F6_0FC8; // OUTBOUND_MSG_COUNTER offset
        outbound_counter_hook::install(fake_addr)
            .expect("stub install should succeed");
        outbound_counter_hook::remove(fake_addr);
        // After remove, no slot should hold the counter address.
        for idx in 0..hwbp::MAX_SLOTS {
            if let Some(slot) = HwbpSlot::from_index(idx) {
                assert_ne!(
                    hwbp::get_address(slot),
                    fake_addr,
                    "slot {idx} should not hold counter addr after remove"
                );
            }
        }
    }
}
