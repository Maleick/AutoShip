//! Stealth memory allocator — uses NT heap and section APIs instead of
//! `VirtualAlloc`, which is a common detection signature for injected code.
//!
//! - **Heap**: `RtlAllocateHeap` / `RtlFreeHeap` via the process default heap (PEB).
//! - **Section**: `NtCreateSection` + `NtMapViewOfSection` backed by the pagefile.
//!
//! On non-Windows platforms, all functions delegate to `std::alloc` for stub builds.

/// Stealth allocator that avoids `VirtualAlloc` detection signatures.
///
/// All methods are `unsafe` — the caller is responsible for ensuring correct
/// pointer/size pairing and that freed pointers originated from the matching
/// allocation function.
pub struct StealthAllocator;

// ---------------------------------------------------------------------------
// Windows implementation — real NT APIs
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::ptr;

    type NtStatus = i32;
    const STATUS_SUCCESS: NtStatus = 0;

    const SECTION_ALL_ACCESS: u32 = 0x000F;
    const PAGE_READWRITE: u32 = 0x04;
    const SEC_COMMIT: u32 = 0x800_0000;

    /// ViewUnmap — the mapped view can be unmapped independently.
    const VIEW_UNMAP: u32 = 2;

    #[repr(C)]
    struct LargeInteger {
        quad_part: i64,
    }

    unsafe extern "system" {
        // ntdll — not exposed by the `windows` crate at a safe level.
        fn RtlAllocateHeap(heap: *mut c_void, flags: u32, size: usize) -> *mut c_void;
        fn RtlFreeHeap(heap: *mut c_void, flags: u32, ptr: *mut c_void) -> u8;

        fn NtCreateSection(
            handle: *mut *mut c_void,
            access: u32,
            obj_attrs: *mut c_void,
            max_size: *mut LargeInteger,
            protect: u32,
            attrs: u32,
            file: *mut c_void,
        ) -> NtStatus;

        fn NtMapViewOfSection(
            section: *mut c_void,
            process: *mut c_void,
            base: *mut *mut c_void,
            zero_bits: usize,
            commit_size: usize,
            offset: *mut LargeInteger,
            view_size: *mut usize,
            inherit: u32,
            alloc_type: u32,
            protect: u32,
        ) -> NtStatus;

        fn NtUnmapViewOfSection(process: *mut c_void, base: *mut c_void) -> NtStatus;
        fn NtClose(handle: *mut c_void) -> NtStatus;
    }

    /// Read the process default heap handle directly from the PEB.
    ///
    /// On x86-64 Windows the TEB is at `gs:[0x00]` and `gs:[0x60]` points to
    /// the PEB. `ProcessHeap` sits at PEB offset `0x30`.
    /// On x86 Windows the TEB is at `fs:[0x00]` and `fs:[0x30]` points to
    /// the PEB. `ProcessHeap` sits at PEB offset `0x18`.
    pub(super) unsafe fn get_process_heap() -> *mut c_void {
        let peb: *mut u8;
        unsafe {
            #[cfg(target_arch = "x86_64")]
            {
                core::arch::asm!(
                    "mov {}, gs:[0x60]",
                    out(reg) peb,
                    options(nostack, readonly, preserves_flags),
                );
                *(peb.add(0x30) as *const *mut c_void)
            }
            #[cfg(target_arch = "x86")]
            {
                core::arch::asm!(
                    "mov {}, fs:[0x30]",
                    out(reg) peb,
                    options(nostack, readonly, preserves_flags),
                );
                *(peb.add(0x18) as *const *mut c_void)
            }
        }
    }

    pub(super) unsafe fn heap_alloc(size: usize) -> *mut u8 {
        let heap = unsafe { get_process_heap() };
        unsafe { RtlAllocateHeap(heap, 0, size) as *mut u8 }
    }

    pub(super) unsafe fn heap_free(ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }
        let heap = unsafe { get_process_heap() };
        unsafe {
            RtlFreeHeap(heap, 0, ptr as *mut c_void);
        }
    }

    pub(super) unsafe fn section_alloc(size: usize) -> *mut u8 {
        let mut handle: *mut c_void = ptr::null_mut();
        let mut section_size = LargeInteger {
            quad_part: size as i64,
        };

        // Create a pagefile-backed section (file handle = NULL).
        let status = unsafe {
            NtCreateSection(
                &mut handle,
                SECTION_ALL_ACCESS,
                ptr::null_mut(),
                &mut section_size,
                PAGE_READWRITE,
                SEC_COMMIT,
                ptr::null_mut(),
            )
        };
        if status != STATUS_SUCCESS {
            tracing::warn!(status, "NtCreateSection failed");
            return ptr::null_mut();
        }

        // Map into our process. NtCurrentProcess = (HANDLE)-1.
        let current_process = -1_isize as *mut c_void;
        let mut base: *mut c_void = ptr::null_mut();
        let mut view_size: usize = 0; // 0 = map entire section

        let status = unsafe {
            NtMapViewOfSection(
                handle,
                current_process,
                &mut base,
                0,
                0,
                ptr::null_mut(),
                &mut view_size,
                VIEW_UNMAP,
                0,
                PAGE_READWRITE,
            )
        };

        // Close the section handle — the mapping keeps the pages alive.
        unsafe {
            NtClose(handle);
        }

        if status != STATUS_SUCCESS {
            tracing::warn!(status, "NtMapViewOfSection failed");
            return ptr::null_mut();
        }

        base as *mut u8
    }

    pub(super) unsafe fn section_free(ptr: *mut u8, _size: usize) {
        let current_process = -1_isize as *mut c_void;
        unsafe {
            NtUnmapViewOfSection(current_process, ptr as *mut c_void);
        }
    }
}

// ---------------------------------------------------------------------------
// Non-Windows stubs — delegate to std::alloc
// ---------------------------------------------------------------------------
#[cfg(not(windows))]
mod platform {
    use std::alloc::{self, Layout};
    use std::ffi::c_void;
    use std::ptr;

    /// Header size prepended to heap allocations so `heap_free` can recover the
    /// original layout without the caller passing the size.
    const HEADER: usize = 16; // keep 16-byte alignment

    pub(super) unsafe fn get_process_heap() -> *mut c_void {
        // Stub: return a non-null sentinel.
        std::ptr::dangling_mut::<c_void>()
    }

    pub(super) unsafe fn heap_alloc(size: usize) -> *mut u8 {
        let total = match size.checked_add(HEADER) {
            Some(t) => t,
            None => return ptr::null_mut(),
        };
        let layout = match Layout::from_size_align(total, HEADER) {
            Ok(l) => l,
            Err(_) => return ptr::null_mut(),
        };
        let raw = unsafe { alloc::alloc(layout) };
        if raw.is_null() {
            return ptr::null_mut();
        }
        // Store size in the header region.
        unsafe {
            *(raw as *mut usize) = size;
            raw.add(HEADER)
        }
    }

    pub(super) unsafe fn heap_free(ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }
        unsafe {
            let raw = ptr.sub(HEADER);
            let size = *(raw as *mut usize);
            let total = size + HEADER;
            let layout = Layout::from_size_align_unchecked(total, HEADER);
            alloc::dealloc(raw, layout);
        }
    }

    pub(super) unsafe fn section_alloc(size: usize) -> *mut u8 {
        let layout = match Layout::from_size_align(size, 4096) {
            Ok(l) => l,
            Err(_) => return ptr::null_mut(),
        };
        unsafe { alloc::alloc(layout) }
    }

    pub(super) unsafe fn section_free(ptr: *mut u8, size: usize) {
        if ptr.is_null() {
            return;
        }
        unsafe {
            let layout = Layout::from_size_align_unchecked(size, 4096);
            alloc::dealloc(ptr, layout);
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------
impl StealthAllocator {
    /// Allocate memory from the process default heap via `RtlAllocateHeap`.
    ///
    /// Returns a null pointer on failure.
    ///
    /// # Safety
    /// The returned pointer must be freed with [`heap_free`](Self::heap_free).
    pub unsafe fn heap_alloc(size: usize) -> *mut u8 {
        let ptr = unsafe { platform::heap_alloc(size) };
        tracing::trace!(addr = ?ptr, size, "stealth heap_alloc");
        ptr
    }

    /// Free memory previously allocated with [`heap_alloc`](Self::heap_alloc).
    ///
    /// # Safety
    /// `ptr` must have been returned by `heap_alloc` and must not be freed twice.
    pub unsafe fn heap_free(ptr: *mut u8) {
        tracing::trace!(addr = ?ptr, "stealth heap_free");
        unsafe { platform::heap_free(ptr) }
    }

    /// Allocate memory via `NtCreateSection` + `NtMapViewOfSection` (pagefile-backed).
    ///
    /// Returns a null pointer on failure.
    ///
    /// # Safety
    /// The returned pointer must be freed with [`section_free`](Self::section_free)
    /// using the same `size`.
    pub unsafe fn section_alloc(size: usize) -> *mut u8 {
        let ptr = unsafe { platform::section_alloc(size) };
        tracing::trace!(addr = ?ptr, size, "stealth section_alloc");
        ptr
    }

    /// Free memory previously allocated with [`section_alloc`](Self::section_alloc).
    ///
    /// # Safety
    /// `ptr` must have been returned by `section_alloc` with the matching `size`,
    /// and must not be freed twice.
    pub unsafe fn section_free(ptr: *mut u8, size: usize) {
        tracing::trace!(addr = ?ptr, size, "stealth section_free");
        unsafe { platform::section_free(ptr, size) }
    }

    /// Get the process default heap handle from the PEB.
    ///
    /// # Safety
    /// On Windows this reads from the PEB via the GS segment register.
    /// On non-Windows this returns a non-null stub sentinel.
    pub unsafe fn process_heap() -> *mut std::ffi::c_void {
        unsafe { platform::get_process_heap() }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn heap_alloc_free_does_not_panic() {
        unsafe {
            let p = StealthAllocator::heap_alloc(256);
            assert!(!p.is_null(), "heap_alloc returned null");
            ptr::write_bytes(p, 0xAB, 256);
            StealthAllocator::heap_free(p);
        }
    }

    #[test]
    fn section_alloc_free_does_not_panic() {
        unsafe {
            let size = 4096;
            let p = StealthAllocator::section_alloc(size);
            assert!(!p.is_null(), "section_alloc returned null");
            ptr::write_bytes(p, 0xCD, size);
            StealthAllocator::section_free(p, size);
        }
    }

    #[test]
    fn process_heap_is_non_null() {
        unsafe {
            let heap = StealthAllocator::process_heap();
            assert!(!heap.is_null(), "process_heap returned null");
        }
    }

    #[test]
    fn heap_alloc_zero_size() {
        unsafe {
            let p = StealthAllocator::heap_alloc(0);
            if !p.is_null() {
                StealthAllocator::heap_free(p);
            }
        }
    }

    #[test]
    fn multiple_heap_allocs_return_distinct_pointers() {
        unsafe {
            let a = StealthAllocator::heap_alloc(64);
            let b = StealthAllocator::heap_alloc(64);
            assert!(!a.is_null());
            assert!(!b.is_null());
            assert_ne!(a, b, "two allocations should not overlap");
            StealthAllocator::heap_free(a);
            StealthAllocator::heap_free(b);
        }
    }
}
