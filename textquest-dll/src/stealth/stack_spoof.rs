//! Per-API call stack spoofing.
//!
//! Overwrites return addresses on the stack with legitimate addresses from
//! ntdll.dll and kernel32.dll before calling sensitive APIs, so that stack
//! walks by anti-cheat see a plausible call chain rather than addresses inside
//! our injected DLL.

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by stack-spoof operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackSpoofError {
    /// The supplied module name contains an interior NUL byte and cannot be
    /// passed safely to `GetModuleHandleA`. Silently falling back to an empty
    /// `CString` would cause the Windows API to return a handle to the
    /// **calling process** instead of the intended target module, producing
    /// incorrect spoofed frames without any error signal.
    InvalidModuleName,
    /// The module was not found in the process's loaded module list.
    ModuleNotFound,
    /// `GetModuleInformation` failed for the given module handle.
    ModuleInfoFailed,
}

impl std::fmt::Display for StackSpoofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidModuleName => f.write_str("module name contains an interior NUL byte"),
            Self::ModuleNotFound => f.write_str("module not found in process module list"),
            Self::ModuleInfoFailed => f.write_str("GetModuleInformation failed"),
        }
    }
}

impl std::error::Error for StackSpoofError {}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod inner {
    use std::sync::OnceLock;

    use tracing::{debug, warn};
    use windows::{
        Win32::{
            Foundation::HMODULE,
            System::{
                LibraryLoader::GetModuleHandleA,
                ProcessStatus::{GetModuleInformation, MODULEINFO},
                Threading::GetCurrentProcess,
            },
        },
        core::PCSTR,
    };

    use super::StackSpoofError;

    /// Cached gadgets from ntdll + kernel32.
    static GADGETS: OnceLock<Vec<usize>> = OnceLock::new();

    /// Locate `ret` (0xC3) gadgets at plausible function-boundary offsets
    /// within a loaded module. We only keep addresses that sit right after
    /// a sequence of non-zero bytes (heuristic for "end of a real
    /// function").
    ///
    /// # Errors
    /// Returns [`StackSpoofError::InvalidModuleName`] if `module_name`
    /// contains an interior NUL byte. Passing an empty `CString` to
    /// `GetModuleHandleA` would silently return a handle to the calling
    /// process rather than the intended target — this function rejects that
    /// case explicitly so the caller receives an unambiguous error.
    pub fn find_gadgets(module_name: &str) -> Result<Vec<usize>, StackSpoofError> {
        let c_name = std::ffi::CString::new(module_name).map_err(|_| {
            warn!(
                "find_gadgets rejected module name containing interior NUL: {:?}",
                module_name
            );
            StackSpoofError::InvalidModuleName
        })?;

        let (base, size) = unsafe {
            let handle: HMODULE = GetModuleHandleA(PCSTR::from_raw(c_name.as_ptr().cast()))
                .map_err(|e| {
                    warn!("GetModuleHandleA({module_name}) failed: {e}");
                    StackSpoofError::ModuleNotFound
                })?;

            let mut info = MODULEINFO::default();
            if GetModuleInformation(
                GetCurrentProcess(),
                handle,
                &mut info,
                std::mem::size_of::<MODULEINFO>() as u32,
            )
            .is_err()
            {
                warn!("GetModuleInformation({module_name}) failed");
                return Err(StackSpoofError::ModuleInfoFailed);
            }
            (info.lpBaseOfDll as usize, info.SizeOfImage as usize)
        };

        if base == 0 || size == 0 {
            return Ok(Vec::new());
        }

        let mut gadgets = Vec::new();
        let slice = unsafe { std::slice::from_raw_parts(base as *const u8, size) };

        // Walk the module looking for 0xC3 bytes that are likely function
        // epilogues (preceded by at least 4 non-zero bytes to skip padding).
        for i in 4..slice.len() {
            if slice[i] == 0xC3
                && slice[i - 1] != 0x00
                && slice[i - 2] != 0x00
                && slice[i - 3] != 0x00
                && slice[i - 4] != 0x00
            {
                gadgets.push(base + i);
            }
        }

        debug!(
            "found {} ret gadgets in {module_name} (base=0x{base:X}, size=0x{size:X})",
            gadgets.len()
        );
        Ok(gadgets)
    }

    /// Initialise the gadget cache from ntdll.dll and kernel32.dll.
    pub fn init() {
        GADGETS.get_or_init(|| {
            let mut g = find_gadgets("ntdll.dll").unwrap_or_default();
            g.extend(find_gadgets("kernel32.dll").unwrap_or_default());
            debug!("stack_spoof: cached {} total gadgets", g.len());
            g
        });
    }

    /// Return a reference to the cached gadget list.
    fn gadgets() -> &'static [usize] {
        GADGETS.get().map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Saved frame state for stack restoration.
    pub struct SpoofedCallContext {
        /// Original return addresses we overwrote, together with their stack
        /// slot addresses so we can restore them.
        saved: Vec<(usize, usize)>,
    }

    impl SpoofedCallContext {
        /// Walk the RBP chain starting from the current frame and overwrite
        /// up to `depth` return addresses with gadget addresses.
        ///
        /// # Safety
        /// Caller must ensure this runs on a thread whose stack is writable and
        /// that `restore` is called before the spoofed frames unwind naturally.
        pub unsafe fn spoof(depth: usize) -> Self {
            let gs = gadgets();
            if gs.is_empty() {
                return Self { saved: Vec::new() };
            }

            let mut saved = Vec::with_capacity(depth);
            let mut rbp: usize;

            // Read current RBP.
            unsafe {
                std::arch::asm!("mov {}, rbp", out(reg) rbp, options(nostack, nomem));
            }

            for i in 0..depth {
                if rbp == 0 {
                    break;
                }
                // Return address sits at rbp + 8 on x86-64.
                let ret_slot = rbp + 8;
                let original = unsafe { std::ptr::read(ret_slot as *const usize) };
                if original == 0 {
                    break;
                }
                // Pick a gadget (round-robin).
                let gadget = gs[i % gs.len()];
                unsafe {
                    std::ptr::write(ret_slot as *mut usize, gadget);
                }
                saved.push((ret_slot, original));
                // Walk to the next frame.
                rbp = unsafe { std::ptr::read(rbp as *const usize) };
            }

            Self { saved }
        }
    }

    /// Panic-safe restoration: Drop restores original return addresses
    /// even if `f` panics inside `with_spoofed_stack`.
    impl Drop for SpoofedCallContext {
        fn drop(&mut self) {
            for (slot, original) in self.saved.iter().rev() {
                unsafe {
                    std::ptr::write(*slot as *mut usize, *original);
                }
            }
        }
    }

    /// Return cached gadgets, initialising on first call.
    pub fn cached_gadgets() -> &'static [usize] {
        init();
        GADGETS.get().map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Execute `f` with the top stack frames spoofed to point into
    /// ntdll/kernel32.
    ///
    /// The return addresses are restored via RAII (Drop) even if `f` panics.
    pub fn with_spoofed_stack<F: FnOnce() -> R, R>(f: F) -> R {
        const SPOOF_DEPTH: usize = 4;
        let _ctx = unsafe { SpoofedCallContext::spoof(SPOOF_DEPTH) };
        f()
    }
}

// ---------------------------------------------------------------------------
// Non-Windows stub
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod inner {
    /// No-op on non-Windows.
    pub fn init() {}

    /// Pass-through on non-Windows — no stack spoofing needed.
    pub fn with_spoofed_stack<F: FnOnce() -> R, R>(f: F) -> R {
        f()
    }

    /// Returns an empty vec on non-Windows, but still validates the module name
    /// so that callers receive `Err(InvalidModuleName)` for NUL-containing
    /// names on all platforms (consistent API contract, cross-platform tests).
    pub fn find_gadgets(module_name: &str) -> Result<Vec<usize>, super::StackSpoofError> {
        // Validate even on non-Windows so callers get consistent error
        // behaviour and regression tests run on macOS/Linux CI.
        std::ffi::CString::new(module_name)
            .map_err(|_| super::StackSpoofError::InvalidModuleName)?;
        Ok(Vec::new())
    }

    /// Returns an empty slice on non-Windows.
    pub fn cached_gadgets() -> &'static [usize] {
        &[]
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialise the stack-spoof gadget cache. Call once during DLL startup.
pub fn init() {
    inner::init();
}

/// Execute `f` with spoofed return addresses on the call stack.
///
/// On Windows this overwrites the top N return addresses with legitimate
/// ntdll/kernel32 addresses, calls `f`, then restores the originals.
/// On other platforms this is a zero-cost pass-through.
pub fn with_spoofed_stack<F: FnOnce() -> R, R>(f: F) -> R {
    inner::with_spoofed_stack(f)
}

/// Find `ret` instruction gadgets in a loaded module.
///
/// # Errors
/// Returns [`StackSpoofError::InvalidModuleName`] if `module_name` contains
/// an interior NUL byte. Passing an empty `CString` to `GetModuleHandleA`
/// silently returns a handle to the **calling process**, producing incorrect
/// spoofed frames — this function rejects that case with an explicit error.
pub fn find_gadgets(module_name: &str) -> Result<Vec<usize>, StackSpoofError> {
    inner::find_gadgets(module_name)
}

/// Get or initialise the shared gadget cache (ntdll + kernel32).
pub fn cached_gadgets() -> &'static [usize] {
    inner::cached_gadgets()
}

// ---------------------------------------------------------------------------
// Sleep-cycle frame spoofing stubs (Layer 3)
// ---------------------------------------------------------------------------

/// Prepare spoofed stack frame after encryption.
/// Called by stealth::sleep() during the sleep transition.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
pub fn prepare_spoofed_frame() {
    // Stub — wired up after #345 HWBP hooking lands.
}

/// Restore real stack frame before decryption.
/// Called by stealth::wake() during the wake transition.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
pub fn restore_real_frame() {
    // Stub — wired up after #345 HWBP hooking lands.
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_does_not_panic() {
        init();
    }

    #[test]
    fn passthrough_returns_value() {
        let val = with_spoofed_stack(|| 42);
        assert_eq!(val, 42);
    }

    #[test]
    fn passthrough_returns_string() {
        let val = with_spoofed_stack(|| String::from("hello"));
        assert_eq!(val, "hello");
    }

    #[test]
    fn find_gadgets_nonexistent_module_returns_empty_or_not_found() {
        // On non-Windows this always returns Ok([]); on Windows it returns
        // Err(ModuleNotFound) or Ok([]) depending on the loaded module list.
        let result = find_gadgets("nonexistent_module_12345.dll");
        match result {
            Ok(gadgets) => assert!(gadgets.is_empty()),
            Err(StackSpoofError::ModuleNotFound) => {} // expected on Windows
            Err(e) => panic!("unexpected error for missing module: {e}"),
        }
    }

    /// Regression test: a module name with an embedded NUL byte must return
    /// `Err(StackSpoofError::InvalidModuleName)` rather than silently passing
    /// an empty `CString` to `GetModuleHandleA`, which would return a handle
    /// to the calling process and fabricate wrong spoofed frames.
    #[test]
    fn find_gadgets_nul_in_name_returns_invalid_module_name_error() {
        let result = find_gadgets("kernel32.dll\0ntdll.dll");
        assert_eq!(
            result,
            Err(StackSpoofError::InvalidModuleName),
            "embedded NUL must be rejected with InvalidModuleName, not silently truncated"
        );
    }

    #[test]
    fn gadget_finder_with_mock_buffer() {
        // Simulate scanning a buffer for ret gadgets (0xC3 after non-zero bytes).
        let buffer: Vec<u8> = vec![
            0x55, 0x48, 0x89, 0xE5, 0xC3, // push rbp; mov rbp,rsp; ret
            0x00, 0x00, 0x00, 0x00, 0x00, // padding
            0x48, 0x83, 0xC4, 0x20, 0xC3, // add rsp,0x20; ret
        ];

        let base = buffer.as_ptr() as usize;
        let mut found = Vec::new();
        for i in 4..buffer.len() {
            if buffer[i] == 0xC3
                && buffer[i - 1] != 0x00
                && buffer[i - 2] != 0x00
                && buffer[i - 3] != 0x00
                && buffer[i - 4] != 0x00
            {
                found.push(base + i);
            }
        }

        // First ret at offset 4, second at offset 14.
        assert_eq!(found.len(), 2);
        assert_eq!(found[0], base + 4);
        assert_eq!(found[1], base + 14);
    }

    #[test]
    fn cached_gadgets_returns_slice() {
        // On non-Windows this returns empty; on Windows it would scan real modules.
        let g = cached_gadgets();
        // Just verify it doesn't panic and returns a valid slice.
        let _ = g.len();
    }

    #[test]
    fn sleep_stubs_do_not_panic() {
        prepare_spoofed_frame();
        restore_real_frame();
    }
}
