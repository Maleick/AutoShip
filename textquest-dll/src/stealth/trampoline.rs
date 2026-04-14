//! Trampoline hardening helpers for return-oriented / detour allocations.
//!
//! Tracks trampoline regions and optionally XOR-obfuscates their bytes when not
//! actively executing. On Windows, trampoline memory is moved to RX after
//! installation to reduce RWX exposure.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// XOR key used for trampoline concealment.
pub const TRAMPOLINE_XOR_KEY: u8 = 0xA5;

/// addr -> (size, is_concealed)
static TRAMPOLINE_REGISTRY: OnceLock<Mutex<HashMap<usize, (usize, bool)>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<usize, (usize, bool)>> {
    TRAMPOLINE_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Hardens retour/trampoline allocations.
pub struct TrampolineHardener;

impl TrampolineHardener {
    pub fn new() -> Self {
        Self
    }

    /// Register a new trampoline region.
    pub fn register(&self, addr: *mut u8, size: usize) {
        if addr.is_null() || size == 0 {
            return;
        }

        if let Ok(mut map) = registry().lock() {
            map.insert(addr as usize, (size, false));
        }
    }

    /// Returns whether an address is currently registered as a trampoline.
    pub fn is_registered(&self, addr: *mut u8) -> bool {
        if addr.is_null() {
            return false;
        }
        registry()
            .lock()
            .ok()
            .is_some_and(|map| map.contains_key(&(addr as usize)))
    }

    /// Returns `Some(is_concealed)` for a registered trampoline.
    pub fn is_concealed(&self, addr: *mut u8) -> Option<bool> {
        if addr.is_null() {
            return None;
        }
        registry()
            .lock()
            .ok()?
            .get(&(addr as usize))
            .map(|(_, concealed)| *concealed)
    }

    /// Mark trampoline bytes as RX-protected memory.
    #[cfg(windows)]
    pub fn protect(&self, addr: *mut u8, size: usize) {
        if addr.is_null() || size == 0 {
            return;
        }

        use windows::Win32::System::Memory::{
            PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS, VirtualProtect,
        };

        let mut old = PAGE_PROTECTION_FLAGS(0);
        // SAFETY: caller-owned pointer+size pair refers to writable trampoline memory.
        if let Err(e) =
            unsafe { VirtualProtect(addr as *const _, size, PAGE_EXECUTE_READ, &mut old) }
        {
            tracing::warn!("VirtualProtect(trampoline RX) failed: {}", e);
        }
    }

    /// Stub implementation on non-Windows platforms.
    #[cfg(not(windows))]
    pub fn protect(&self, _addr: *mut u8, _size: usize) {}

    /// Apply XOR concealment to trampoline bytes.
    pub fn conceal(&self, addr: *mut u8, size: usize) {
        self.xor_region(addr, size);

        if let Ok(mut map) = registry().lock() {
            if let Some((_, concealed)) = map.get_mut(&(addr as usize)) {
                *concealed = true;
            }
        }
    }

    /// Reveal concealed trampoline bytes before execution.
    pub fn reveal(&self, addr: *mut u8, size: usize) {
        self.xor_region(addr, size);

        if let Ok(mut map) = registry().lock() {
            if let Some((_, concealed)) = map.get_mut(&(addr as usize)) {
                *concealed = false;
            }
        }
    }

    fn xor_region(&self, addr: *mut u8, size: usize) {
        if addr.is_null() || size == 0 {
            return;
        }
        // SAFETY: The caller is responsible for passing a valid writable region.
        unsafe {
            let bytes = std::slice::from_raw_parts_mut(addr, size);
            for byte in bytes {
                *byte ^= TRAMPOLINE_XOR_KEY;
            }
        }
    }

    /// Protect all registered trampoline regions as RX.
    pub fn protect_registered(&self) {
        let entries: Vec<(usize, usize)> = registry()
            .lock()
            .map(|map| map.iter().map(|(a, (s, _))| (*a, *s)).collect())
            .unwrap_or_default();
        for (addr, size) in entries {
            self.protect(addr as *mut u8, size);
        }
    }

    #[cfg(test)]
    pub(crate) fn clear_registry(&self) {
        if let Ok(mut map) = registry().lock() {
            map.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    /// Serialize all tests in this module: they share a global `TRAMPOLINE_REGISTRY`
    /// and each test starts by calling `clear_registry()`, which races when tests
    /// run in parallel. Holding `TEST_GUARD` for the duration of each test prevents
    /// concurrent access to the shared registry.
    static TEST_GUARD: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_tests() -> std::sync::MutexGuard<'static, ()> {
        TEST_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn conceal_roundtrip() {
        let _guard = lock_tests();
        let hardener = TrampolineHardener::new();
        hardener.clear_registry();

        let mut data = vec![0x10u8, 0x22, 0x33, 0x44, 0x55, 0xAA];
        let original = data.clone();
        let len = data.len();

        hardener.conceal(data.as_mut_ptr(), len);
        assert_ne!(data.as_slice(), original.as_slice());
        hardener.reveal(data.as_mut_ptr(), len);
        assert_eq!(data.as_slice(), original.as_slice());
    }

    #[test]
    fn protect_does_not_panic_on_stub_platform() {
        let _guard = lock_tests();
        let hardener = TrampolineHardener::new();
        hardener.clear_registry();

        let mut data = vec![0xAAu8, 0xBB, 0xCC];
        hardener.register(data.as_mut_ptr(), data.len());
        hardener.protect(data.as_mut_ptr(), data.len());
    }

    #[test]
    fn registry_tracks_entries() {
        let _guard = lock_tests();
        let hardener = TrampolineHardener::new();
        hardener.clear_registry();

        let mut data = vec![0x01u8, 0x02, 0x03, 0x04];
        hardener.register(data.as_mut_ptr(), data.len());

        assert!(hardener.is_registered(data.as_mut_ptr()));
        assert_eq!(hardener.is_concealed(data.as_mut_ptr()), Some(false));

        hardener.conceal(data.as_mut_ptr(), data.len());
        assert_eq!(hardener.is_concealed(data.as_mut_ptr()), Some(true));

        hardener.reveal(data.as_mut_ptr(), data.len());
        assert_eq!(hardener.is_concealed(data.as_mut_ptr()), Some(false));
    }
}
