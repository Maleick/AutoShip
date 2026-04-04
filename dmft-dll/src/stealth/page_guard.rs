//! VirtualProtect toggle — flips .text between RW and RX.
//! Layer 1 of per-frame sleep obfuscation. Functions in .dmft section.

/// Set .text to PAGE_EXECUTE_READ (executable). Called after decryption.
#[cfg(windows)]
#[unsafe(link_section = ".dmft")]
pub fn set_executable() {
    use windows::Win32::System::Memory::{
        PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS, VirtualProtect,
    };
    let (base, size) = super::text_encrypt::text_section_bounds();
    if base.is_null() || size == 0 {
        return;
    }
    let mut old = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: base/size are our own DLL .text section. Transition: RW->RX.
    if let Err(e) = unsafe { VirtualProtect(base as *const _, size, PAGE_EXECUTE_READ, &mut old) } {
        tracing::error!("VirtualProtect(RX) failed: {}", e);
    }
}

/// Set .text to PAGE_READWRITE (writable). Called before encryption.
#[cfg(windows)]
#[unsafe(link_section = ".dmft")]
pub fn set_writable() {
    use windows::Win32::System::Memory::{PAGE_PROTECTION_FLAGS, PAGE_READWRITE, VirtualProtect};
    let (base, size) = super::text_encrypt::text_section_bounds();
    if base.is_null() || size == 0 {
        return;
    }
    let mut old = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Same as set_executable — our own .text section. Transition: RX->RW.
    if let Err(e) = unsafe { VirtualProtect(base as *const _, size, PAGE_READWRITE, &mut old) } {
        tracing::error!("VirtualProtect(RW) failed: {}", e);
    }
}

#[cfg(not(windows))]
pub fn set_executable() {}
#[cfg(not(windows))]
pub fn set_writable() {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stubs_do_not_panic() {
        set_executable();
        set_writable();
    }
}
