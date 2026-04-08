//! Stealth module — per-frame sleep obfuscation (Gargoyle-style).
//!
//! Three-layer memory stealth that keeps the DLL's code encrypted and
//! non-executable ~97% of the time:
//!
//! 1. **VirtualProtect toggle** (page_guard): Flips .text between RW and RX.
//! 2. **SIMD XOR encryption** (text_encrypt): Encrypts .text with random key.
//! 3. **Call stack spoofing** (stack_spoof): Stub for hypnus-style masking (#345).
//!
//! Functions in wake/sleep transitions use `#[link_section = ".tq"]` to stay
//! executable when `.text` is encrypted.

pub mod alloc;
pub mod etw_blind;
pub mod page_encrypt;
pub mod page_guard;
pub mod stack_spoof;
pub mod text_encrypt;
pub mod thread_pool;

use std::sync::atomic::{AtomicBool, Ordering};

static SLEEP_ENABLED: AtomicBool = AtomicBool::new(false);
static SLEEP_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CODE_ENCRYPTED: AtomicBool = AtomicBool::new(false);

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
    fn wake_sleep_noop_when_disabled() {
        SLEEP_ENABLED.store(false, Ordering::Release);
        wake();
        sleep();
    }

    #[test]
    fn enable_disable_lifecycle() {
        // Combined test to avoid races on shared global atomics.
        // Phase 1: enable is a no-op when not initialized.
        SLEEP_INITIALIZED.store(false, Ordering::Release);
        SLEEP_ENABLED.store(false, Ordering::Release);
        CODE_ENCRYPTED.store(false, Ordering::Release);
        enable();
        assert!(!is_enabled());

        // Phase 2: enable/disable work when initialized.
        SLEEP_INITIALIZED.store(true, Ordering::Release);
        enable();
        assert!(is_enabled());
        disable();
        assert!(!is_enabled());
    }
}
