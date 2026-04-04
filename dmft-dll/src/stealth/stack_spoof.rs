//! Per-API call stack spoofing.
//!
//! Overwrites return addresses on the stack with legitimate addresses from
//! ntdll.dll and kernel32.dll before calling sensitive APIs, so that stack
//! walks by anti-cheat see a plausible call chain rather than addresses inside
//! our injected DLL.

use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod inner {
    use std::sync::OnceLock;

    use tracing::{debug, warn};
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::GetModuleHandleA;
    use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::core::PCSTR;

    /// Cached gadgets from ntdll + kernel32.
    static GADGETS: OnceLock<Vec<usize>> = OnceLock::new();

    /// Locate `ret` (0xC3) gadgets at plausible function-boundary offsets within
    /// a loaded module. We only keep addresses that sit right after a sequence of
    /// non-zero bytes (heuristic for "end of a real function").
    pub fn find_gadgets(module_name: &str) -> Vec<usize> {
        let c_name = std::ffi::CString::new(module_name).unwrap_or_default();

        let (base, size) = unsafe {
            let handle: HMODULE =
                match GetModuleHandleA(PCSTR::from_raw(c_name.as_ptr().cast())) {
                    Ok(h) => h,
                    Err(e) => {
                        warn!("GetModuleHandleA({module_name}) failed: {e}");
                        return Vec::new();
                    }
                };

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
                return Vec::new();
            }
            (info.lpBaseOfDll as usize, info.SizeOfImage as usize)
        };

        if base == 0 || size == 0 {
            return Vec::new();
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
        gadgets
    }

    /// Initialise the gadget cache from ntdll.dll and kernel32.dll.
    pub fn init() {
        GADGETS.get_or_init(|| {
            let mut g = find_gadgets("ntdll.dll");
            g.extend(find_gadgets("kernel32.dll"));
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

        /// Restore original return addresses.
        ///
        /// # Safety
        /// The saved slot addresses must still be valid (i.e., the frames have
        /// not been unwound yet).
        pub unsafe fn restore(self) {
            for (slot, original) in self.saved.iter().rev() {
                unsafe {
                    std::ptr::write(*slot as *mut usize, *original);
                }
            }
        }
    }

    /// Execute `f` with the top stack frames spoofed to point into ntdll/kernel32.
    ///
    /// The return addresses are restored immediately after `f` completes.
    pub fn with_spoofed_stack<F: FnOnce() -> R, R>(f: F) -> R {
        const SPOOF_DEPTH: usize = 4;
        let ctx = unsafe { SpoofedCallContext::spoof(SPOOF_DEPTH) };
        let result = f();
        unsafe {
            ctx.restore();
        }
        result
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

    /// Returns an empty vec on non-Windows.
    pub fn find_gadgets(_module_name: &str) -> Vec<usize> {
        Vec::new()
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
pub fn find_gadgets(module_name: &str) -> Vec<usize> {
    inner::find_gadgets(module_name)
}

/// Cached gadgets from ntdll + kernel32 (re-exported for external use).
static GADGET_CACHE: OnceLock<Vec<usize>> = OnceLock::new();

/// Get or initialise the shared gadget cache.
pub fn cached_gadgets() -> &'static [usize] {
    GADGET_CACHE
        .get_or_init(|| {
            let mut g = find_gadgets("ntdll.dll");
            g.extend(find_gadgets("kernel32.dll"));
            g
        })
        .as_slice()
}

// ---------------------------------------------------------------------------
// Sleep-cycle frame spoofing stubs (Layer 3)
// ---------------------------------------------------------------------------

/// Prepare spoofed stack frame after encryption.
/// Called by stealth::sleep() during the sleep transition.
#[cfg_attr(windows, unsafe(link_section = ".dmft"))]
pub fn prepare_spoofed_frame() {
    // Stub — wired up after #345 HWBP hooking lands.
}

/// Restore real stack frame before decryption.
/// Called by stealth::wake() during the wake transition.
#[cfg_attr(windows, unsafe(link_section = ".dmft"))]
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
    fn find_gadgets_nonexistent_module_returns_empty() {
        let gadgets = find_gadgets("nonexistent_module_12345.dll");
        assert!(gadgets.is_empty());
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
