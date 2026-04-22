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

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the timer-queue sleep path has been initialised and is running.
static TIMER_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Ownership token for the encrypt/decrypt cycle.
///
/// Only one path — per-frame or timer-queue — may hold the cycle at a time.
/// The mutex is try-locked; if the other owner holds it the timer callback
/// simply skips its cycle rather than blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepOwner {
    /// No path currently owns the cycle.
    None,
    /// The per-frame path (`stealth::wake` / `stealth::sleep`) owns the cycle.
    Frame,
    /// The timer-queue callback owns the cycle.
    TimerQueue,
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

    use super::{SleepOwner, TIMER_ACTIVE, TimerQueueError};

    /// Global owner state.  `try_lock()` is used from the timer callback so
    /// it never blocks the pool thread if the frame path holds the lock.
    static OWNER: Mutex<SleepOwner> = Mutex::new(SleepOwner::None);

    /// Raw handle to the timer-queue timer (stored as usize for Send safety).
    static TIMER_HANDLE: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

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

        // Guard 4: acquire ownership; skip if frame path holds OWNER.
        let Some(guard) = try_acquire(SleepOwner::TimerQueue) else {
            tracing::trace!("timer_queue_sleep: skipping — frame path holds owner");
            return;
        };

        // Encrypt .text for the duration of the idle window.
        crate::stealth::sleep();

        // Release owner before decrypt so frame path can acquire if needed.
        release(guard);

        // Re-acquire for decrypt phase.  If frame path grabbed owner first,
        // `stealth::wake()` will handle decrypt — we are done.
        if let Some(guard2) = try_acquire(SleepOwner::TimerQueue) {
            crate::stealth::wake();
            release(guard2);
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
                None,                       // NULL = default process queue
                Some(timer_callback),
                None,                       // context = NULL
                interval_ms,                // due time (first fire)
                interval_ms,                // period (subsequent fires)
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

    #[test]
    fn sleep_owner_variants_are_distinct() {
        assert_ne!(SleepOwner::None, SleepOwner::Frame);
        assert_ne!(SleepOwner::None, SleepOwner::TimerQueue);
        assert_ne!(SleepOwner::Frame, SleepOwner::TimerQueue);
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
}
