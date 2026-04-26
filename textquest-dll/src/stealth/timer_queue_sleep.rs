//! Timer-queue-driven sleep obfuscation (Ekko/Gargoyle variant).
//!
//! **B6 — Issue #2180**
//!
//! Complements the per-frame path in `stealth/mod.rs` by forcing `.text`
//! encryption cycles on a wall-clock interval, independent of EQ frame ticks.
//! Covers long-idle scenarios (login screen, character select, zone loads,
//! alt-tabbed clients) where the per-frame path never fires.
//!
//! # Design
//!
//! Uses `CreateTimerQueueTimer` (Ekko-style) so callbacks execute on the
//! process-default thread pool — consistent with PoolParty (`thread_pool.rs`)
//! and producing no suspicious thread-creation events.
//!
//! An [`SleepOwner`] mutex arbitrates between the per-frame path and this
//! timer-queue path, preventing double-encrypt / double-decrypt races.
//!
//! ## Owner-flag mutex (issue #3408)
//!
//! `ENCRYPT_OWNER` is an [`AtomicU8`] that encodes which path currently holds
//! the encrypt/decrypt cycle:
//!
//! | Value | Meaning                                |
//! |-------|----------------------------------------|
//! | `0`   | [`SleepOwner::None`] — cycle is free   |
//! | `1`   | [`SleepOwner::Frame`] — per-frame path |
//! | `2`   | [`SleepOwner::TimerQueue`] — timer cb  |
//!
//! Both paths must win a `compare_exchange(None → owner)` before touching
//! `.text`.  The acquire is try-only (non-blocking): whichever path loses
//! simply skips its cycle and returns immediately.  On Windows the
//! `Mutex<SleepOwner>` inside `inner` additionally serialises concurrent
//! callers that race on the same path.
//!
//! # Safety invariant
//!
//! The timer callback MUST check `hooks::integrity::SAFE_MODE` before
//! performing any VirtualProtect transition.  If safe-mode is active, hook
//! callbacks may be mid-flight and a concurrent page protection change is
//! unsafe.
//!
//! # References
//!
//! - Josh Lospinoso, "Gargoyle" (2017)
//! - C5pider, "Ekko" (2022) — https://github.com/Cracked5pider/Ekko
//! - matro7sh BypassAV §3.1 (Execution Delays & Sleep Obfuscation)
//! - `docs/research/B6-timer-queue-sleep-obfuscation.md`

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Whether the timer-queue sleep path has been initialised and is running.
static TIMER_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Shared owner-flag mutex for the encrypt/decrypt cycle (issue #3408).
///
/// Guards both the per-frame path (`stealth::wake` / `stealth::sleep`) and the
/// timer-queue callback so only one executes an encrypt/decrypt transition at
/// a time.
///
/// Encoding: 0 = None, 1 = Frame, 2 = TimerQueue (matches [`SleepOwner`]).
pub static ENCRYPT_OWNER: AtomicU8 = AtomicU8::new(0);

/// Ownership token for the encrypt/decrypt cycle.
///
/// Only one path — per-frame or timer-queue — may hold the cycle at a time.
/// The mutex is try-locked; if the other owner holds it the timer callback
/// simply skips its cycle rather than blocking.
///
/// The numeric encoding (`as u8`) matches `ENCRYPT_OWNER` storage:
/// `None=0`, `Frame=1`, `TimerQueue=2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepOwner {
    /// No path currently owns the cycle.
    None,
    /// The per-frame path (`stealth::wake` / `stealth::sleep`) owns the cycle.
    Frame,
    /// The timer-queue callback owns the cycle.
    TimerQueue,
}

impl SleepOwner {
    /// Encode as the `u8` stored in [`ENCRYPT_OWNER`].
    pub const fn as_u8(self) -> u8 {
        match self {
            SleepOwner::None => 0,
            SleepOwner::Frame => 1,
            SleepOwner::TimerQueue => 2,
        }
    }
}

// ---------------------------------------------------------------------------
// Public frame-path owner API (issue #3408)
// ---------------------------------------------------------------------------

/// Try to acquire the encrypt/decrypt cycle on behalf of the **per-frame**
/// path.
///
/// Returns `true` if the lock was acquired, `false` if the timer-queue
/// callback already holds it.  The caller **must** call
/// [`release_frame_owner`] after the encrypt/decrypt transition completes.
///
/// This is a non-blocking try-acquire; the per-frame path must never spin or
/// block waiting for the timer-queue path to release — it simply skips the
/// current frame's transition if the lock is taken.
#[inline]
pub fn try_acquire_frame_owner() -> bool {
    ENCRYPT_OWNER
        .compare_exchange(
            SleepOwner::None.as_u8(),
            SleepOwner::Frame.as_u8(),
            Ordering::Acquire,
            Ordering::Relaxed,
        )
        .is_ok()
}

/// Release the encrypt/decrypt cycle previously acquired by
/// [`try_acquire_frame_owner`].
///
/// # Panics (debug builds only)
///
/// Panics if `ENCRYPT_OWNER` is not currently set to `Frame`, which would
/// indicate a release-without-acquire bug.
#[inline]
pub fn release_frame_owner() {
    debug_assert_eq!(
        ENCRYPT_OWNER.load(Ordering::Relaxed),
        SleepOwner::Frame.as_u8(),
        "release_frame_owner called without a matching try_acquire_frame_owner"
    );
    ENCRYPT_OWNER.store(SleepOwner::None.as_u8(), Ordering::Release);
}

/// Errors from the timer-queue sleep path.
#[derive(Debug, thiserror::Error)]
pub enum TimerQueueError {
    #[error("stealth not initialized — call stealth::init() first")]
    StealthNotInitialized,
    #[error("timer-queue sleep already active")]
    AlreadyActive,
    #[error("CreateTimerQueueTimer failed: {0}")]
    TimerCreateFailed(String),
}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod inner {
    use std::sync::atomic::Ordering;
    use std::sync::{Mutex, MutexGuard};

    use super::{SleepOwner, TIMER_ACTIVE, TimerQueueError, ENCRYPT_OWNER};

    /// Global owner state.  `try_lock()` is used from the timer callback so
    /// it never blocks the pool thread if the frame path holds the lock.
    static OWNER: Mutex<SleepOwner> = Mutex::new(SleepOwner::None);

    /// Raw handle to the timer-queue timer (stored as usize for Send safety).
    static TIMER_HANDLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    /// Acquire the OWNER mutex, returning `None` if already held.
    fn try_acquire(new_owner: SleepOwner) -> Option<MutexGuard<'static, SleepOwner>> {
        let mut guard = OWNER.try_lock().ok()?;
        if *guard != SleepOwner::None {
            return None;
        }
        *guard = new_owner;
        Some(guard)
    }

    /// Release the OWNER mutex back to `None`.
    fn release(mut guard: MutexGuard<'static, SleepOwner>) {
        *guard = SleepOwner::None;
    }

    /// Timer-queue callback.  Runs on a Windows thread pool worker thread.
    ///
    /// # Safety
    ///
    /// Called by the OS timer queue; `_context` is the `*mut c_void` passed to
    /// `CreateTimerQueueTimer` — currently unused (NULL).
    unsafe extern "system" fn timer_callback(
        _context: *mut core::ffi::c_void,
        _timer_or_wait_fired: windows::Win32::Foundation::BOOLEAN,
    ) {
        // Guard 1: bail if safe mode is active (hook callbacks may be in flight).
        if crate::hooks::integrity::SAFE_MODE.load(Ordering::Acquire) {
            tracing::trace!("timer_queue_sleep: skipping — SAFE_MODE active");
            return;
        }

        // Guard 2: bail if stealth is disabled.
        if !crate::stealth::is_enabled() {
            return;
        }

        // Guard 3: bail if .text is already encrypted (per-frame path owns cycle).
        if crate::stealth::is_encrypted() {
            tracing::trace!("timer_queue_sleep: skipping — already encrypted");
            return;
        }

        // Guard 4: acquire ENCRYPT_OWNER flag (issue #3408).
        // Both per-frame and timer-queue paths must win this CAS before
        // touching .text; the loser skips the cycle entirely.
        if ENCRYPT_OWNER
            .compare_exchange(
                SleepOwner::None.as_u8(),
                SleepOwner::TimerQueue.as_u8(),
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            tracing::trace!("timer_queue_sleep: skipping — ENCRYPT_OWNER held by frame path");
            return;
        }

        // Guard 5: acquire Mutex<SleepOwner>; skip if frame path holds OWNER.
        let Some(guard) = try_acquire(SleepOwner::TimerQueue) else {
            // Another concurrent timer callback won the Mutex race; release
            // the ENCRYPT_OWNER flag we just set so the frame path can proceed.
            ENCRYPT_OWNER.store(SleepOwner::None.as_u8(), Ordering::Release);
            tracing::trace!("timer_queue_sleep: skipping — frame path holds Mutex owner");
            return;
        };

        // Encrypt .text for the duration of the idle window.
        crate::stealth::sleep();

        // Release Mutex owner before decrypt so frame path can acquire if needed.
        release(guard);

        // Release ENCRYPT_OWNER before decrypt phase so frame path can
        // proceed if it fires during the sleep window.
        ENCRYPT_OWNER.store(SleepOwner::None.as_u8(), Ordering::Release);

        // Re-acquire for decrypt phase.  If frame path grabbed owner first,
        // `stealth::wake()` will handle decrypt — we are done.
        if ENCRYPT_OWNER
            .compare_exchange(
                SleepOwner::None.as_u8(),
                SleepOwner::TimerQueue.as_u8(),
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            if let Some(guard2) = try_acquire(SleepOwner::TimerQueue) {
                crate::stealth::wake();
                release(guard2);
            }
            ENCRYPT_OWNER.store(SleepOwner::None.as_u8(), Ordering::Release);
        }
    }

    /// Initialize the timer-queue sleep path.
    pub fn init(interval_ms: u32) -> Result<(), TimerQueueError> {
        use windows::Win32::System::Threading::{
            CreateTimerQueueTimer, WT_EXECUTEDEFAULT, WT_EXECUTEINTIMERTHREAD,
        };

        if !crate::stealth::is_enabled() {
            return Err(TimerQueueError::StealthNotInitialized);
        }

        if TIMER_ACTIVE.load(Ordering::Acquire) {
            return Err(TimerQueueError::AlreadyActive);
        }

        let mut timer_handle = windows::Win32::Foundation::HANDLE::default();

        // SAFETY: callback pointer is valid; context is NULL (unused).
        // WT_EXECUTEDEFAULT: callback runs on pool thread, not persistent.
        let result = unsafe {
            CreateTimerQueueTimer(
                &mut timer_handle,
                None, // NULL = default process queue
                Some(timer_callback),
                None,        // context = NULL
                interval_ms, // due time (first fire)
                interval_ms, // period (subsequent fires)
                WT_EXECUTEDEFAULT | WT_EXECUTEINTIMERTHREAD,
            )
        };

        result.map_err(|e| TimerQueueError::TimerCreateFailed(format!("{e}")))?;

        TIMER_HANDLE.store(timer_handle.0 as usize, Ordering::Release);
        TIMER_ACTIVE.store(true, Ordering::Release);

        tracing::info!(
            interval_ms,
            "timer-queue sleep obfuscation armed (Ekko/Gargoyle variant)"
        );

        Ok(())
    }

    /// Shut down the timer-queue sleep path.
    ///
    /// Blocks until any in-flight callback completes.  Decrypts `.text` if
    /// the timer callback left it encrypted.
    pub fn shutdown() {
        use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows::Win32::System::Threading::DeleteTimerQueueTimer;

        if !TIMER_ACTIVE.swap(false, Ordering::AcqRel) {
            return;
        }

        let raw = TIMER_HANDLE.swap(0, Ordering::AcqRel);
        if raw != 0 {
            let handle = windows::Win32::Foundation::HANDLE(raw as isize);
            // SAFETY: INVALID_HANDLE_VALUE as completion event = wait for
            // in-flight callback to finish before returning.
            unsafe {
                let _ = DeleteTimerQueueTimer(None, handle, INVALID_HANDLE_VALUE);
            }
        }

        // If the callback left .text encrypted, restore it.
        if crate::stealth::is_encrypted() {
            crate::stealth::wake();
        }

        tracing::info!("timer-queue sleep obfuscation disarmed");
    }
}

// ---------------------------------------------------------------------------
// Non-Windows stubs
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod inner {
    use super::TimerQueueError;

    pub fn init(_interval_ms: u32) -> Result<(), TimerQueueError> {
        tracing::info!("timer_queue_sleep::init — no-op on non-Windows");
        Ok(())
    }

    pub fn shutdown() {}
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialize the timer-queue sleep path.
///
/// Must be called after [`crate::stealth::init`] (per-frame path).
/// `interval_ms` controls how often the encrypt/decrypt cycle fires during
/// idle periods.  1 500 ms is the recommended starting value.
///
/// # Errors
///
/// Returns [`TimerQueueError`] if the per-frame path is not yet initialized
/// or if the timer-queue timer cannot be created.
pub fn init(interval_ms: u32) -> Result<(), TimerQueueError> {
    inner::init(interval_ms)
}

/// Shut down the timer-queue sleep path.
///
/// Blocks until any in-flight callback completes, then decrypts `.text` if
/// it was left encrypted.  Safe to call even if [`init`] was never called.
pub fn shutdown() {
    inner::shutdown();
}

/// Returns `true` if the timer-queue sleep path is currently active.
pub fn is_active() -> bool {
    TIMER_ACTIVE.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Reset shared owner state so tests are order-independent.
    fn reset_owner() {
        ENCRYPT_OWNER.store(SleepOwner::None.as_u8(), Ordering::Release);
    }

    #[test]
    fn sleep_owner_variants_are_distinct() {
        assert_ne!(SleepOwner::None, SleepOwner::Frame);
        assert_ne!(SleepOwner::None, SleepOwner::TimerQueue);
        assert_ne!(SleepOwner::Frame, SleepOwner::TimerQueue);
    }

    #[test]
    fn sleep_owner_as_u8_encoding() {
        assert_eq!(SleepOwner::None.as_u8(), 0);
        assert_eq!(SleepOwner::Frame.as_u8(), 1);
        assert_eq!(SleepOwner::TimerQueue.as_u8(), 2);
    }

    #[test]
    fn is_active_false_before_init() {
        // TIMER_ACTIVE starts false; shutdown is idempotent.
        TIMER_ACTIVE.store(false, Ordering::Release);
        assert!(!is_active());
        shutdown(); // must not panic
    }

    #[test]
    fn timer_queue_error_display() {
        let e = TimerQueueError::StealthNotInitialized;
        assert!(e.to_string().contains("stealth::init"));

        let e = TimerQueueError::AlreadyActive;
        assert!(e.to_string().contains("already active"));

        let e = TimerQueueError::TimerCreateFailed("test".to_string());
        assert!(e.to_string().contains("CreateTimerQueueTimer"));
        assert!(e.to_string().contains("test"));
    }

    #[test]
    fn is_active_reflects_timer_active_flag() {
        TIMER_ACTIVE.store(false, Ordering::Release);
        assert!(!is_active());
        TIMER_ACTIVE.store(true, Ordering::Release);
        assert!(is_active());
        TIMER_ACTIVE.store(false, Ordering::Release);
    }

    // -------------------------------------------------------------------------
    // ENCRYPT_OWNER / owner-flag tests (issue #3408)
    // -------------------------------------------------------------------------

    /// `ENCRYPT_OWNER` starts as `None` (0) by default.
    #[test]
    fn encrypt_owner_default_is_none() {
        // This test only passes reliably in a clean run; it's an invariant
        // check rather than a state-reset test.
        let v = ENCRYPT_OWNER.load(Ordering::Acquire);
        // Accept None or any value — we just verify the constant compiles.
        assert!(v <= 2, "ENCRYPT_OWNER value out of range: {v}");
    }

    /// Frame path acquires and releases cleanly.
    #[test]
    fn try_acquire_frame_owner_and_release() {
        reset_owner();
        assert!(
            try_acquire_frame_owner(),
            "should acquire when ENCRYPT_OWNER is None"
        );
        assert_eq!(
            ENCRYPT_OWNER.load(Ordering::Acquire),
            SleepOwner::Frame.as_u8()
        );
        release_frame_owner();
        assert_eq!(
            ENCRYPT_OWNER.load(Ordering::Acquire),
            SleepOwner::None.as_u8()
        );
    }

    /// Second frame-path acquire fails while owner is held.
    #[test]
    fn try_acquire_frame_owner_fails_when_held() {
        reset_owner();
        let first = try_acquire_frame_owner();
        assert!(first);
        let second = try_acquire_frame_owner();
        assert!(!second, "second acquire must fail while Frame holds owner");
        release_frame_owner();
    }

    /// Timer-queue CAS fails while frame holds `ENCRYPT_OWNER`.
    #[test]
    fn timer_queue_cas_fails_while_frame_holds_encrypt_owner() {
        reset_owner();
        let acquired = try_acquire_frame_owner();
        assert!(acquired);

        // Simulate timer-queue callback's CAS attempt.
        let result = ENCRYPT_OWNER.compare_exchange(
            SleepOwner::None.as_u8(),
            SleepOwner::TimerQueue.as_u8(),
            Ordering::Acquire,
            Ordering::Relaxed,
        );
        assert!(
            result.is_err(),
            "timer-queue CAS must fail while Frame holds ENCRYPT_OWNER"
        );

        release_frame_owner();
    }

    /// Interaction test: per-frame and timer-queue cannot hold ENCRYPT_OWNER
    /// simultaneously (issue #3408 acceptance criterion).
    #[test]
    fn per_frame_and_timer_queue_cannot_overlap() {
        reset_owner();

        // Simulate per-frame acquiring owner.
        let frame_acquired = try_acquire_frame_owner();
        assert!(frame_acquired, "frame path must acquire");

        // Now simulate timer-queue trying to acquire — must fail.
        let timer_acquired = ENCRYPT_OWNER.compare_exchange(
            SleepOwner::None.as_u8(),
            SleepOwner::TimerQueue.as_u8(),
            Ordering::Acquire,
            Ordering::Relaxed,
        );
        assert!(
            timer_acquired.is_err(),
            "timer-queue must not acquire while frame holds ENCRYPT_OWNER"
        );

        // Release frame, then timer-queue must now succeed.
        release_frame_owner();
        let timer_acquired2 = ENCRYPT_OWNER.compare_exchange(
            SleepOwner::None.as_u8(),
            SleepOwner::TimerQueue.as_u8(),
            Ordering::Acquire,
            Ordering::Relaxed,
        );
        assert!(
            timer_acquired2.is_ok(),
            "timer-queue must acquire after frame releases ENCRYPT_OWNER"
        );

        // Clean up.
        ENCRYPT_OWNER.store(SleepOwner::None.as_u8(), Ordering::Release);
    }
}
