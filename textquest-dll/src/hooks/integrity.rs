//! Hook integrity self-check — verifies HWBP slot state before IPC activation.
//!
//! Called during DLL initialization after hooks are installed and before the IPC
//! command listener starts accepting commands. If any slot is in an inconsistent
//! state (active flag set without a valid address or callback, or vice-versa),
//! the check fails and the DLL enters safe mode: no hook callbacks will be
//! dispatched and IPC commands are rejected.
//!
//! "Trampoline integrity" in the context of this codebase means verifying
//! the HWBP (hardware-breakpoint) hook registry — since we use VEH-based
//! hardware breakpoints rather than byte-patching trampolines, the "trampoline"
//! is the combination of (slot address, callback pointer, active flag). A
//! corrupted entry is one where these three fields are mutually inconsistent.

use std::sync::atomic::{AtomicBool, Ordering};

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
///   (address/callback may be non-zero temporarily during registration,
///   but after `remove_all()` they must be zero)
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
/// Checks every HWBP slot for internal consistency. Returns an `IntegrityReport`
/// describing per-slot results and whether the overall check passed.
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

/// Returns `true` if the DLL is currently in safe mode (integrity check failed).
pub fn is_safe_mode() -> bool {
    SAFE_MODE.load(Ordering::Acquire)
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
}
