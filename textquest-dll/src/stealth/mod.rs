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
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
pub fn wake() {
    let _guard = SleepCycleGuard::acquire();
    if !SLEEP_ENABLED.load(Ordering::Acquire) {
        return;
    }
    if !CODE_ENCRYPTED.load(Ordering::Acquire) {
        return;
    }
    stack_spoof::restore_real_frame();
    text_encrypt::decrypt();
    page_guard::set_executable();
    CODE_ENCRYPTED.store(false, Ordering::Release);
}

/// Sleep: set RW + encrypt .text. Called at frame end.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
pub fn sleep() {
    let _guard = SleepCycleGuard::acquire();
    if !SLEEP_ENABLED.load(Ordering::Acquire) {
        return;
    }
    if CODE_ENCRYPTED.load(Ordering::Acquire) {
        return;
    }
    page_guard::set_writable();
    text_encrypt::encrypt();
    stack_spoof::prepare_spoofed_frame();
    CODE_ENCRYPTED.store(true, Ordering::Release);
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

    #[test]
    fn defaults() {
        let _ = (is_encrypted(), is_enabled());
    }

    #[test]
    fn enable_disable_lifecycle() {
        // Single test to avoid races on shared global atomics.
        // Phase 1: wake/sleep are no-ops when disabled.
        SLEEP_INITIALIZED.store(false, Ordering::Release);
        SLEEP_ENABLED.store(false, Ordering::Release);
        CODE_ENCRYPTED.store(false, Ordering::Release);
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
        SLEEP_ENABLED.store(false, Ordering::Release);
        CODE_ENCRYPTED.store(false, Ordering::Release);
        wake(); // should not panic
        assert!(!is_encrypted());
    }

    #[test]
    fn sleep_noop_when_not_enabled() {
        SLEEP_ENABLED.store(false, Ordering::Release);
        CODE_ENCRYPTED.store(false, Ordering::Release);
        sleep(); // should not panic
        assert!(!is_encrypted());
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
