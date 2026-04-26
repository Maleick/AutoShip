//! Stealth module — per-frame sleep obfuscation (Gargoyle-style).
//!
//! Three-layer memory stealth that keeps the DLL's code encrypted and
//! non-executable ~97% of the time:
//!
//! 1. **VirtualProtect toggle** (page_guard): Flips .text between RW and RX.
//! 2. **SIMD XOR encryption** (text_encrypt): Encrypts .text with random key.
//! 3. **Call stack spoofing** (stack_spoof): Stub for hypnus-style masking
//!    (#345).
//!
//! Functions in wake/sleep transitions use `#[link_section = ".tq"]` to stay
//! executable when `.text` is encrypted.

pub mod alloc;
pub mod direct_stub;
pub mod etw_blind;
pub mod page_encrypt;
pub mod page_guard;
#[cfg(windows)]
pub mod pe_erase;
#[cfg(windows)]
pub mod peb_unlink;
#[cfg(windows)]
pub mod section_remap;
pub mod stack_spoof;
pub mod text_encrypt;
pub mod thread_pool;
pub mod timer_queue_sleep;
pub mod trampoline;

use std::sync::atomic::{AtomicBool, Ordering};

static SLEEP_ENABLED: AtomicBool = AtomicBool::new(false);
static SLEEP_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CODE_ENCRYPTED: AtomicBool = AtomicBool::new(false);
static SLEEP_CYCLE_LOCKED: AtomicBool = AtomicBool::new(false);

/// RAII guard for the sleep-cycle spin lock.
///
/// Acquires the lock on construction and releases it on drop, so unwinding
/// panics in the critical section cannot leave `SLEEP_CYCLE_LOCKED` stuck at
/// `true` (which would deadlock all later `wake()`/`sleep()` calls).
struct SleepCycleGuard;

impl SleepCycleGuard {
    #[cfg_attr(windows, unsafe(link_section = ".tq"))]
    fn acquire() -> Self {
        // Acquire on success is enough to establish ordering with the previous
        // holder's Release on unlock. Relaxed on failure avoids a pointless
        // Acquire barrier on every spin iteration.
        while SLEEP_CYCLE_LOCKED
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Hint to the CPU we're in a spin loop so it can reduce power /
            // yield SMT resources instead of burning a full core.
            std::hint::spin_loop();
        }
        SleepCycleGuard
    }
}

impl Drop for SleepCycleGuard {
    #[cfg_attr(windows, unsafe(link_section = ".tq"))]
    fn drop(&mut self) {
        SLEEP_CYCLE_LOCKED.store(false, Ordering::Release);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StealthError {
    #[error("failed to locate DLL .text section")]
    TextSectionNotFound,
    #[error("VirtualProtect failed: {0}")]
    VirtualProtect(String),
    #[error("failed to generate encryption key")]
    KeyGeneration,
    #[error("sleep obfuscation is incompatible with active IPC background threads")]
    IncompatibleWithIpc,
}

/// Initialize per-frame sleep obfuscation.
pub fn init() -> Result<(), StealthError> {
    if SLEEP_INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }
    if crate::ipc::is_running() {
        return Err(StealthError::IncompatibleWithIpc);
    }
    text_encrypt::init()?;
    SLEEP_INITIALIZED.store(true, Ordering::Release);
    SLEEP_ENABLED.store(true, Ordering::Release);
    tracing::info!("Sleep obfuscation initialized");
    Ok(())
}

/// Enable sleep obfuscation (must be initialized first).
pub fn enable() {
    if SLEEP_INITIALIZED.load(Ordering::Acquire) {
        SLEEP_ENABLED.store(true, Ordering::Release);
        tracing::info!("Sleep obfuscation enabled");
    }
}

/// Disable sleep obfuscation. Decrypts if currently encrypted.
pub fn disable() {
    if CODE_ENCRYPTED.load(Ordering::Acquire) {
        wake();
    }
    SLEEP_ENABLED.store(false, Ordering::Release);
    tracing::info!("Sleep obfuscation disabled");
}

/// Wake: decrypt .text + set RX. Called at frame start.
///
/// Acquires [`timer_queue_sleep::ENCRYPT_OWNER`] before the transition (issue
/// #3408) so the per-frame and timer-queue paths cannot run simultaneously.
/// If the timer-queue callback currently holds the owner flag this call
/// returns without blocking — the frame will wake on the next tick.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
pub fn wake() {
    // Try to claim ENCRYPT_OWNER for the frame path.  Non-blocking: if the
    // timer-queue callback already holds it we skip this frame's decrypt.
    if !timer_queue_sleep::try_acquire_frame_owner() {
        tracing::trace!("stealth::wake: skipping — timer-queue path holds ENCRYPT_OWNER");
        return;
    }
    let _guard = SleepCycleGuard::acquire();
    if !SLEEP_ENABLED.load(Ordering::Acquire) {
        timer_queue_sleep::release_frame_owner();
        return;
    }
    if !CODE_ENCRYPTED.load(Ordering::Acquire) {
        timer_queue_sleep::release_frame_owner();
        return;
    }
    stack_spoof::restore_real_frame();
    text_encrypt::decrypt();
    page_guard::set_executable();
    CODE_ENCRYPTED.store(false, Ordering::Release);
    timer_queue_sleep::release_frame_owner();
}

/// Sleep: set RW + encrypt .text. Called at frame end.
///
/// Acquires [`timer_queue_sleep::ENCRYPT_OWNER`] before the transition (issue
/// #3408) so the per-frame and timer-queue paths cannot run simultaneously.
/// If the timer-queue callback currently holds the owner flag this call
/// returns without blocking — the frame will sleep on the next tick.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
pub fn sleep() {
    // Try to claim ENCRYPT_OWNER for the frame path.  Non-blocking: if the
    // timer-queue callback already holds it we skip this frame's encrypt.
    if !timer_queue_sleep::try_acquire_frame_owner() {
        tracing::trace!("stealth::sleep: skipping — timer-queue path holds ENCRYPT_OWNER");
        return;
    }
    let _guard = SleepCycleGuard::acquire();
    if !SLEEP_ENABLED.load(Ordering::Acquire) {
        timer_queue_sleep::release_frame_owner();
        return;
    }
    if CODE_ENCRYPTED.load(Ordering::Acquire) {
        timer_queue_sleep::release_frame_owner();
        return;
    }
    page_guard::set_writable();
    text_encrypt::encrypt();
    stack_spoof::prepare_spoofed_frame();
    CODE_ENCRYPTED.store(true, Ordering::Release);
    timer_queue_sleep::release_frame_owner();
}

pub fn is_encrypted() -> bool {
    CODE_ENCRYPTED.load(Ordering::Acquire)
}
pub fn is_enabled() -> bool {
    SLEEP_ENABLED.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::timer_queue_sleep::{ENCRYPT_OWNER, SleepOwner};

    /// Reset all shared atomics to a known state before each test so tests are
    /// order-independent.  Must be called at the top of every test that calls
    /// `wake()` or `sleep()`.
    fn reset_state() {
        SLEEP_INITIALIZED.store(false, Ordering::Release);
        SLEEP_ENABLED.store(false, Ordering::Release);
        CODE_ENCRYPTED.store(false, Ordering::Release);
        SLEEP_CYCLE_LOCKED.store(false, Ordering::Release);
        ENCRYPT_OWNER.store(SleepOwner::None.as_u8(), Ordering::Release);
    }

    #[test]
    fn defaults() {
        let _ = (is_encrypted(), is_enabled());
    }

    #[test]
    fn enable_disable_lifecycle() {
        // Single test to avoid races on shared global atomics.
        // Phase 1: wake/sleep are no-ops when disabled.
        reset_state();
        wake();
        sleep();

        // Phase 2: enable is a no-op when not initialized.
        enable();
        assert!(!is_enabled());

        // Phase 3: enable/disable work when initialized.
        SLEEP_INITIALIZED.store(true, Ordering::Release);
        enable();
        assert!(is_enabled());
        disable();
        assert!(!is_enabled());
    }

    #[test]
    fn is_encrypted_reflects_code_encrypted_flag() {
        CODE_ENCRYPTED.store(false, Ordering::Release);
        assert!(!is_encrypted());
        CODE_ENCRYPTED.store(true, Ordering::Release);
        assert!(is_encrypted());
        CODE_ENCRYPTED.store(false, Ordering::Release);
    }

    #[test]
    fn is_enabled_reflects_sleep_enabled_flag() {
        let prev = SLEEP_ENABLED.load(Ordering::Acquire);
        SLEEP_ENABLED.store(false, Ordering::Release);
        assert!(!is_enabled());
        SLEEP_ENABLED.store(true, Ordering::Release);
        assert!(is_enabled());
        SLEEP_ENABLED.store(prev, Ordering::Release);
    }

    #[test]
    fn wake_noop_when_not_enabled() {
        reset_state();
        wake(); // should not panic
        assert!(!is_encrypted());
        // ENCRYPT_OWNER must be released even on early-exit paths.
        assert_eq!(
            ENCRYPT_OWNER.load(Ordering::Acquire),
            SleepOwner::None.as_u8(),
            "wake() leaked ENCRYPT_OWNER"
        );
    }

    #[test]
    fn sleep_noop_when_not_enabled() {
        reset_state();
        sleep(); // should not panic
        assert!(!is_encrypted());
        assert_eq!(
            ENCRYPT_OWNER.load(Ordering::Acquire),
            SleepOwner::None.as_u8(),
            "sleep() leaked ENCRYPT_OWNER"
        );
    }

    /// Interaction test (issue #3408): per-frame path must not proceed when
    /// the timer-queue path holds `ENCRYPT_OWNER`.
    ///
    /// Simulates a timer-queue callback holding the owner flag while a frame
    /// tick fires wake() simultaneously.  The frame path must skip without
    /// panicking and must leave `CODE_ENCRYPTED` unchanged.
    #[test]
    fn per_frame_skips_when_timer_queue_holds_encrypt_owner() {
        reset_state();
        SLEEP_ENABLED.store(true, Ordering::Release);
        SLEEP_INITIALIZED.store(true, Ordering::Release);
        // Simulate timer-queue callback holding the owner.
        CODE_ENCRYPTED.store(true, Ordering::Release);
        ENCRYPT_OWNER.store(SleepOwner::TimerQueue.as_u8(), Ordering::Release);

        // Per-frame wake() must skip — timer-queue holds owner.
        wake();

        // .text encryption state must be unchanged.
        assert!(
            is_encrypted(),
            "wake() should not have decrypted while timer-queue held ENCRYPT_OWNER"
        );
        // Owner flag must still be TimerQueue (we did not modify it).
        assert_eq!(
            ENCRYPT_OWNER.load(Ordering::Acquire),
            SleepOwner::TimerQueue.as_u8(),
            "wake() must not steal ENCRYPT_OWNER from timer-queue path"
        );

        // Release the simulated timer-queue lock so subsequent tests are clean.
        ENCRYPT_OWNER.store(SleepOwner::None.as_u8(), Ordering::Release);
        CODE_ENCRYPTED.store(false, Ordering::Release);
    }

    /// Interaction test (issue #3408): timer-queue path must not proceed when
    /// the per-frame path holds `ENCRYPT_OWNER`.
    ///
    /// Verifies that `try_acquire_frame_owner()` actually blocks a concurrent
    /// timer-queue `compare_exchange` attempt.
    #[test]
    fn timer_queue_skips_when_frame_holds_encrypt_owner() {
        reset_state();
        // Simulate per-frame path holding the owner.
        let acquired = crate::stealth::timer_queue_sleep::try_acquire_frame_owner();
        assert!(acquired, "frame path should have acquired ENCRYPT_OWNER");
        assert_eq!(
            ENCRYPT_OWNER.load(Ordering::Acquire),
            SleepOwner::Frame.as_u8()
        );

        // A second try_acquire_frame_owner must fail (owner already taken).
        let second = crate::stealth::timer_queue_sleep::try_acquire_frame_owner();
        assert!(!second, "second acquire should fail while Frame holds owner");

        // A simulated timer-queue CAS must also fail.
        let timer_cas = ENCRYPT_OWNER.compare_exchange(
            SleepOwner::None.as_u8(),
            SleepOwner::TimerQueue.as_u8(),
            Ordering::Acquire,
            Ordering::Relaxed,
        );
        assert!(timer_cas.is_err(), "timer-queue CAS must fail while Frame holds owner");

        // Release and verify.
        crate::stealth::timer_queue_sleep::release_frame_owner();
        assert_eq!(
            ENCRYPT_OWNER.load(Ordering::Acquire),
            SleepOwner::None.as_u8(),
            "release_frame_owner must restore ENCRYPT_OWNER to None"
        );
    }

    /// No-deadlock smoke test (issue #3408): rapid alternating acquire/release
    /// on both paths must not deadlock or panic.
    #[test]
    fn no_deadlock_rapid_frame_acquire_release() {
        reset_state();
        for _ in 0..1_000 {
            if crate::stealth::timer_queue_sleep::try_acquire_frame_owner() {
                crate::stealth::timer_queue_sleep::release_frame_owner();
            }
        }
        assert_eq!(
            ENCRYPT_OWNER.load(Ordering::Acquire),
            SleepOwner::None.as_u8(),
            "ENCRYPT_OWNER must be None after stress loop"
        );
    }

    #[test]
    fn stealth_error_display() {
        let err = StealthError::TextSectionNotFound;
        assert!(format!("{err}").contains(".text"));

        let err = StealthError::VirtualProtect("access denied".to_string());
        assert!(format!("{err}").contains("access denied"));

        let err = StealthError::KeyGeneration;
        assert!(format!("{err}").contains("key"));

        let err = StealthError::IncompatibleWithIpc;
        assert!(format!("{err}").contains("IPC"));
    }
}
