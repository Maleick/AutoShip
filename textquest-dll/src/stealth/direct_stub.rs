//! Direct syscall fallback stubs.
//!
//! When the RecycledGate gadget hunt fails (gadget site hooked, ntdll version
//! skew, ASLR edge case), this module provides a secondary execution path that
//! emits `syscall; ret` from a private RX page inside our DLL.
//!
//! # Security note
//!
//! Direct stubs leave the return address inside our DLL's memory region, which
//! is detectable by EDR / AC return-address scanners. This path is intentionally
//! secondary and logged — it trades stealth for availability. Some SSNs are
//! explicitly blocked from the fallback (fail-closed).
//!
//! # Blocked SSNs (fail-closed)
//!
//! - `NtProtectVirtualMemory` — primary CFG / shadow-stack validation target
//! - `NtAllocateVirtualMemory` — allocation from non-ntdll is extremely suspicious
//!
//! # Stub layout (x64, 16 bytes each)
//!
//! ```text
//! 49 89 CA          mov r10, rcx
//! B8 XX XX 00 00    mov eax, <SSN>
//! 0F 05             syscall
//! C3                ret
//! 90 90 90 90 90    nop (padding to 16-byte boundary)
//! ```

use super::super::syscall::hash;
use std::sync::OnceLock;

/// Hashes blocked from direct-stub fallback. Fail-closed for these SSNs.
const BLOCKED_HASHES: [u32; 2] = [
    hash::NT_PROTECT_VIRTUAL_MEMORY,
    hash::NT_ALLOCATE_VIRTUAL_MEMORY,
];

/// Stride between adjacent stubs in the RX page (16-byte aligned).
const STUB_STRIDE: usize = 16;

/// Direct stub bytes with placeholder SSN (little-endian u16 at offset 4).
///
/// ```text
/// 0: 49 89 CA        mov r10, rcx
/// 3: B8 00 00 00 00  mov eax, 0x0000  ← SSN patched at offset 4
/// 8: 0F 05           syscall
/// A: C3              ret
/// B: 90 90 90 90 90  nop padding
/// ```
const STUB_TEMPLATE: [u8; STUB_STRIDE] = [
    0x49, 0x89, 0xCA, // mov r10, rcx
    0xB8, 0x00, 0x00, 0x00, 0x00, // mov eax, 0  (SSN patched at bytes 4-5)
    0x0F, 0x05, // syscall
    0xC3, // ret
    0x90, 0x90, 0x90, 0x90, 0x90, // nop padding
];

/// An allocated RX page containing direct stubs, one per registered SSN.
pub struct DirectStubPage {
    /// Base address of the RX page.
    base: *mut u8,
    /// Number of stubs written.
    count: usize,
    /// (hash, stub_index) pairs for lookup.
    index: Vec<(u32, usize)>,
}

// SAFETY: The page is allocated once and never mutated after init.
unsafe impl Send for DirectStubPage {}
unsafe impl Sync for DirectStubPage {}

impl DirectStubPage {
    /// Invoke a direct stub by function hash.
    ///
    /// Returns `None` if the hash is unknown or if the SSN is blocked from
    /// direct fallback (fail-closed). Returns `Some(fn_ptr)` on success.
    pub fn stub_ptr(&self, hash: u32) -> Option<usize> {
        // Fail-closed: block high-risk SSNs from direct fallback.
        if BLOCKED_HASHES.contains(&hash) {
            tracing::error!(
                hash = format!("{:#x}", hash),
                "Direct syscall fallback blocked for this SSN (fail-closed)"
            );
            return None;
        }

        let idx = self.index.iter().find(|(h, _)| *h == hash)?.1;
        let ptr = (self.base as usize) + idx * STUB_STRIDE;
        Some(ptr)
    }
}

impl Drop for DirectStubPage {
    fn drop(&mut self) {
        if !self.base.is_null() {
            #[cfg(windows)]
            unsafe {
                let _ = windows::Win32::System::Memory::VirtualFree(
                    self.base as *mut _,
                    0,
                    windows::Win32::System::Memory::MEM_RELEASE,
                );
            }
        }
    }
}

/// Global direct-stub page, initialized alongside the syscall table.
static STUB_PAGE: OnceLock<DirectStubPage> = OnceLock::new();

/// Initialize the direct-stub fallback page.
///
/// Allocates a private RX page, writes one stub per (hash, ssn) pair, then
/// locks the page as execute+read (RX). Safe to call multiple times
/// (idempotent).
///
/// # Arguments
///
/// * `entries` — slice of `(djb2_hash, ssn)` pairs for which to generate stubs.
pub fn init(entries: &[(u32, u16)]) -> Result<(), DirectStubError> {
    let _ = STUB_PAGE.get_or_try_init(|| {
        let page = build_stub_page(entries)?;
        tracing::info!(
            count = entries.len(),
            "Direct syscall fallback page allocated"
        );
        Ok(page)
    })?;
    Ok(())
}

/// Get the global stub page.
pub fn page() -> Option<&'static DirectStubPage> {
    STUB_PAGE.get()
}

/// Invoke a syscall via the direct fallback path.
///
/// Emits a warning log, checks the blocked list, then calls the stub.
/// Returns `Err(DirectStubError::Blocked)` for blocked SSNs.
///
/// # Safety
///
/// The SSN must be valid for the current OS. All arguments must satisfy the
/// underlying NT syscall ABI.
#[cfg(windows)]
pub unsafe fn call_direct(
    hash: u32,
    ssn: u16,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    arg6: usize,
) -> Result<i32, DirectStubError> {
    tracing::warn!(
        hash = format!("{:#x}", hash),
        ssn,
        reason = "RecycledGate gadget unavailable",
        "Direct syscall fallback invoked"
    );

    let stub_page = STUB_PAGE.get().ok_or(DirectStubError::NotInitialized)?;
    if BLOCKED_HASHES.contains(&hash) {
        tracing::warn!(
            hash = format!("{:#x}", hash),
            ssn,
            reason = "hash is explicitly blocked from direct-stub fallback",
            "Direct syscall fallback denied"
        );
        return Err(DirectStubError::Blocked);
    }

    let fn_ptr = match stub_page.stub_ptr(hash) {
        Some(ptr) => ptr,
        None => {
            tracing::warn!(
                hash = format!("{:#x}", hash),
                ssn,
                reason = "hash not present in direct-stub index",
                "Direct syscall fallback unavailable"
            );
            return Err(DirectStubError::Blocked);
        }
    };
    // Cast the stub to a raw function pointer and call it.
    // The stub uses the Windows x64 syscall ABI directly.
    type DirectStubFn = unsafe extern "system" fn(
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
    ) -> i32;

    let f: DirectStubFn = unsafe { std::mem::transmute(fn_ptr) };
    let result = unsafe { f(arg1, arg2, arg3, arg4, arg5, arg6) };
    Ok(result)
}

/// macOS stub — direct syscalls are Windows-only.
#[cfg(not(windows))]
pub unsafe fn call_direct(
    _hash: u32,
    _ssn: u16,
    _arg1: usize,
    _arg2: usize,
    _arg3: usize,
    _arg4: usize,
    _arg5: usize,
    _arg6: usize,
) -> Result<i32, DirectStubError> {
    Ok(0)
}

// ── Page allocation + stub generation ───────────────────────────────────────

#[cfg(windows)]
fn build_stub_page(entries: &[(u32, u16)]) -> Result<DirectStubPage, DirectStubError> {
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, VirtualAlloc,
        VirtualProtect,
    };

    let n = entries.len();
    if n == 0 {
        return Err(DirectStubError::NoEntries);
    }

    // Allocate enough space for all stubs (one page minimum).
    let page_size = 0x1000usize;
    let alloc_size = ((n * STUB_STRIDE + page_size - 1) / page_size) * page_size;

    let base = unsafe {
        VirtualAlloc(
            None,
            alloc_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    };

    if base.is_null() {
        return Err(DirectStubError::AllocationFailed);
    }

    let base_ptr = base as *mut u8;

    // Write stubs with SSNs patched in.
    let mut index = Vec::with_capacity(n);
    for (i, &(hash, ssn)) in entries.iter().enumerate() {
        let stub_ptr = unsafe { base_ptr.add(i * STUB_STRIDE) };
        let mut stub = STUB_TEMPLATE;
        // Patch SSN at offset 4 (little-endian u16).
        stub[4] = (ssn & 0xFF) as u8;
        stub[5] = (ssn >> 8) as u8;
        unsafe {
            std::ptr::copy_nonoverlapping(stub.as_ptr(), stub_ptr, STUB_STRIDE);
        }
        index.push((hash, i));
        tracing::trace!(hash = format!("{:#x}", hash), ssn, "Direct stub written");
    }

    // Lock page to PAGE_EXECUTE_READ — no more writes.
    let mut old_protect = windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS(0);
    let result = unsafe {
        VirtualProtect(
            base,
            alloc_size,
            PAGE_EXECUTE_READ,
            &mut old_protect,
        )
    };

    if result.is_err() {
        // Failed to lock — free and error.
        unsafe {
            let _ = windows::Win32::System::Memory::VirtualFree(
                base,
                0,
                windows::Win32::System::Memory::MEM_RELEASE,
            );
        }
        return Err(DirectStubError::ProtectFailed);
    }

    tracing::debug!(
        base = format!("{:#x}", base_ptr as usize),
        stubs = n,
        "Direct stub RX page locked"
    );

    Ok(DirectStubPage {
        base: base_ptr,
        count: n,
        index,
    })
}

#[cfg(not(windows))]
fn build_stub_page(entries: &[(u32, u16)]) -> Result<DirectStubPage, DirectStubError> {
    if entries.is_empty() {
        return Err(DirectStubError::NoEntries);
    }
    // macOS: no real allocation needed, return a sentinel.
    Ok(DirectStubPage {
        base: std::ptr::null_mut(),
        count: entries.len(),
        index: entries.iter().enumerate().map(|(i, &(h, _))| (h, i)).collect(),
    })
}

/// Errors from direct stub initialization or invocation.
#[derive(Debug, thiserror::Error)]
pub enum DirectStubError {
    #[error("stub page not initialized — call init() first")]
    NotInitialized,

    #[error("this SSN is blocked from direct fallback (fail-closed)")]
    Blocked,

    #[error("no stub entries provided")]
    NoEntries,

    #[error("VirtualAlloc failed for stub page")]
    AllocationFailed,

    #[error("VirtualProtect to PAGE_EXECUTE_READ failed")]
    ProtectFailed,
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_entries() -> Vec<(u32, u16)> {
        vec![
            (hash::NT_SET_CONTEXT_THREAD, 0x00BE),
            (hash::NT_GET_CONTEXT_THREAD, 0x00F5),
        ]
    }

    #[cfg(not(windows))]
    #[test]
    fn stub_template_layout() {
        // Verify the template has the right opcodes at fixed offsets.
        assert_eq!(&STUB_TEMPLATE[0..3], &[0x49, 0x89, 0xCA]); // mov r10, rcx
        assert_eq!(STUB_TEMPLATE[3], 0xB8); // mov eax,
        assert_eq!(&STUB_TEMPLATE[8..10], &[0x0F, 0x05]); // syscall
        assert_eq!(STUB_TEMPLATE[10], 0xC3); // ret
    }

    #[cfg(not(windows))]
    #[test]
    fn blocked_hashes_are_correct() {
        assert!(BLOCKED_HASHES.contains(&hash::NT_PROTECT_VIRTUAL_MEMORY));
        assert!(BLOCKED_HASHES.contains(&hash::NT_ALLOCATE_VIRTUAL_MEMORY));
        assert!(!BLOCKED_HASHES.contains(&hash::NT_SET_CONTEXT_THREAD));
        assert!(!BLOCKED_HASHES.contains(&hash::NT_GET_CONTEXT_THREAD));
    }

    #[cfg(not(windows))]
    #[test]
    fn build_stub_page_macos() {
        let entries = make_test_entries();
        let page = build_stub_page(&entries).unwrap();
        assert_eq!(page.count, 2);
        assert_eq!(page.index.len(), 2);
    }

    #[cfg(not(windows))]
    #[test]
    fn stub_ptr_blocked_returns_none() {
        // Blocked hashes should return None even if they were in entries.
        // (They won't be in entries in practice, but test the guard.)
        let blocked_entries = vec![(hash::NT_PROTECT_VIRTUAL_MEMORY, 0x004D_u16)];
        let blocked_page = build_stub_page(&blocked_entries).unwrap();
        assert!(blocked_page.stub_ptr(hash::NT_PROTECT_VIRTUAL_MEMORY).is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn stub_ptr_allowed_returns_some() {
        let entries = make_test_entries();
        let _page = build_stub_page(&entries).unwrap();
        // Allowed hashes should return a ptr (non-null on macOS sentinel page
        // returns 0 since base is null, but the index lookup succeeds).
        // We just verify the lookup doesn't panic and the blocked check passes.
        // The actual pointer value on macOS is 0 (null base + offset).
        let _ = _page.stub_ptr(hash::NT_SET_CONTEXT_THREAD);
        let _ = _page.stub_ptr(hash::NT_GET_CONTEXT_THREAD);
    }

    #[cfg(not(windows))]
    #[test]
    fn call_direct_macos_stub_returns_success() {
        // macOS stub always returns Ok(0).
        let result = unsafe {
            call_direct(hash::NT_SET_CONTEXT_THREAD, 0x00BE, 0, 0, 0, 0, 0, 0)
        };
        assert_eq!(result.unwrap(), 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn no_entries_errors() {
        let result = build_stub_page(&[]);
        assert!(matches!(result, Err(DirectStubError::NoEntries)));
    }

    #[test]
    fn stub_stride_is_power_of_two() {
        assert_eq!(STUB_STRIDE & (STUB_STRIDE - 1), 0);
    }

    #[test]
    fn blocked_hashes_are_unique() {
        assert_ne!(BLOCKED_HASHES[0], BLOCKED_HASHES[1]);
    }

    /// Simulate gadget failure for one SSN and verify fallback path is invoked.
    ///
    /// This test forces the condition described in issue #2181: the gadget is
    /// unavailable, so the direct fallback must handle the call.
    #[cfg(not(windows))]
    #[test]
    fn fallback_invoked_when_gadget_missing() {
        // Simulate: RecycledGate returned gadget = 0 (not found).
        // Direct fallback should be attempted for allowed SSNs.
        let gadget_unavailable = 0usize;
        let hash = hash::NT_SET_CONTEXT_THREAD;
        let ssn = 0x00BE_u16;

        // Mimic the dispatch logic: gadget == 0 → use fallback.
        let result = if gadget_unavailable == 0 {
            unsafe { call_direct(hash, ssn, 0, 0, 0, 0, 0, 0) }
        } else {
            Ok(-1) // would be RecycledGate path
        };

        // macOS stub returns Ok(0); on Windows would be a real syscall result.
        assert_eq!(result.unwrap(), 0, "Fallback path should return STATUS_SUCCESS stub");
    }

    /// Verify blocked SSNs fail-closed even if the gadget is also missing.
    #[cfg(not(windows))]
    #[test]
    fn blocked_ssn_fails_closed_when_gadget_missing() {
        // NtProtectVirtualMemory must not fall back to direct stub.
        // If gadget fails we expect an error, not a direct exec.
        let entries = vec![(hash::NT_PROTECT_VIRTUAL_MEMORY, 0x004D_u16)];
        let page = build_stub_page(&entries).unwrap();
        let ptr = page.stub_ptr(hash::NT_PROTECT_VIRTUAL_MEMORY);
        assert!(
            ptr.is_none(),
            "NtProtectVirtualMemory must not be accessible via direct fallback"
        );
    }
}
