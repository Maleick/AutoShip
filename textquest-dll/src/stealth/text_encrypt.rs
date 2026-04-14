//! SIMD XOR encryption of the DLL's .text section.
//!
//! **Layer 2** of per-frame sleep obfuscation. Between frames, the .text section
//! is XOR-encrypted with a random 16-byte key (refreshed each cycle).
//!
//! Uses SSE2 SIMD intrinsics (128-bit XOR). SSE2 is guaranteed on x86_64.
//!
//! Encrypt/decrypt functions live in `.tq` section so they remain callable
//! when `.text` is encrypted.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Base address of the .text section.
static TEXT_BASE: AtomicUsize = AtomicUsize::new(0);

/// Size of the .text section in bytes.
static TEXT_SIZE: AtomicUsize = AtomicUsize::new(0);

/// Whether .text has been located.
static TEXT_LOCATED: AtomicBool = AtomicBool::new(false);

/// 16-byte XOR key, regenerated each encrypt cycle via OS entropy.
///
/// Only accessed from the game loop thread (encrypt generates, decrypt consumes).
/// They strictly alternate — never concurrent.
static mut XOR_KEY: [u8; 16] = [0u8; 16];

/// Return the .text section base pointer and size.
pub fn text_section_bounds() -> (*mut u8, usize) {
    if !TEXT_LOCATED.load(Ordering::Acquire) {
        return (std::ptr::null_mut(), 0);
    }
    (
        TEXT_BASE.load(Ordering::Acquire) as *mut u8,
        TEXT_SIZE.load(Ordering::Acquire),
    )
}

/// Locate the DLL's .text section from PE headers.
pub fn init() -> Result<(), super::StealthError> {
    #[cfg(windows)]
    {
        let (base, size) = locate_text_section()?;
        TEXT_BASE.store(base as usize, Ordering::Release);
        TEXT_SIZE.store(size, Ordering::Release);
        TEXT_LOCATED.store(true, Ordering::Release);

        tracing::info!(
            base = format!("{:#x}", base as usize),
            size_kb = size / 1024,
            "Located .text section for encryption"
        );
    }

    #[cfg(not(windows))]
    {
        tracing::info!("Text encryption init skipped (non-Windows stub)");
    }

    Ok(())
}

/// Encrypt .text in-place with a fresh random XOR key.
/// Must be called while the section is PAGE_READWRITE.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
pub fn encrypt() {
    if !TEXT_LOCATED.load(Ordering::Acquire) {
        return;
    }

    generate_key();

    let base = TEXT_BASE.load(Ordering::Acquire) as *mut u8;
    let size = TEXT_SIZE.load(Ordering::Acquire);

    // SAFETY: base/size describe our DLL's .text (resolved from PE headers).
    // Section is RW. Single-threaded from game loop.
    #[allow(unused_unsafe)] // xor_region is unsafe on Windows, no-op on macOS
    unsafe {
        xor_region(base, size);
    }
}

/// Decrypt .text in-place (XOR is self-inverse).
/// Must be called while the section is PAGE_READWRITE.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
pub fn decrypt() {
    if !TEXT_LOCATED.load(Ordering::Acquire) {
        return;
    }

    let base = TEXT_BASE.load(Ordering::Acquire) as *mut u8;
    let size = TEXT_SIZE.load(Ordering::Acquire);

    // SAFETY: Same invariants as encrypt. XOR with same key reverses it.
    #[allow(unused_unsafe)]
    unsafe {
        xor_region(base, size);
    }
}

/// Generate a 16-byte random XOR key from OS entropy.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn generate_key() {
    // SAFETY: XOR_KEY is only accessed from the game loop thread.
    // encrypt() generates the key, decrypt() consumes it, strictly alternating.
    // SAFETY: Single-threaded access from game loop. addr_of_mut avoids
    // creating an intermediate reference to the static, satisfying Rust 2024.
    let key_slice = unsafe { std::slice::from_raw_parts_mut(&raw mut XOR_KEY as *mut u8, 16) };
    if getrandom::getrandom(key_slice).is_err() {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for (i, byte) in key_slice.iter_mut().enumerate() {
            *byte = ((seed >> ((i % 16) * 8)) & 0xFF) as u8 ^ (i as u8).wrapping_mul(0x9E);
        }
        tracing::warn!("getrandom failed for XOR key — using PRNG fallback");
    }
}

/// XOR a memory region in-place using SSE2 SIMD (128-bit lanes).
///
/// # Safety
/// - `base` must be writable for `size` bytes.
/// - Single-threaded access to XOR_KEY.
#[cfg(windows)]
#[unsafe(link_section = ".tq")]
unsafe fn xor_region(base: *mut u8, size: usize) {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::{__m128i, _mm_loadu_si128, _mm_storeu_si128, _mm_xor_si128};

        // SAFETY: Caller guarantees base is writable for size bytes and
        // single-threaded access to XOR_KEY.
        unsafe {
            let key_vec: __m128i = _mm_loadu_si128(&raw const XOR_KEY as *const __m128i);
            let chunks = size / 16;
            let remainder = size % 16;

            for i in 0..chunks {
                let ptr = base.add(i * 16) as *mut __m128i;
                let data = _mm_loadu_si128(ptr as *const __m128i);
                let xored = _mm_xor_si128(data, key_vec);
                _mm_storeu_si128(ptr, xored);
            }

            let tail_start = chunks * 16;
            let key_ptr = &raw const XOR_KEY as *const u8;
            for i in 0..remainder {
                let ptr = base.add(tail_start + i);
                *ptr ^= *key_ptr.add(i % 16);
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        // SAFETY: Caller guarantees base is writable for size bytes.
        unsafe {
            let key_ptr = &raw const XOR_KEY as *const u8;
            for i in 0..size {
                *base.add(i) ^= *key_ptr.add(i % 16);
            }
        }
    }
}

/// Scalar XOR fallback for non-Windows (functional for tests).
#[cfg(not(windows))]
unsafe fn xor_region(base: *mut u8, size: usize) {
    // SAFETY: Caller guarantees base is writable for size bytes.
    unsafe {
        let key_ptr = &raw const XOR_KEY as *const u8;
        for i in 0..size {
            *base.add(i) ^= *key_ptr.add(i % 16);
        }
    }
}

/// Locate .text section by parsing in-memory PE headers.
#[cfg(windows)]
fn locate_text_section() -> Result<(*mut u8, usize), super::StealthError> {
    let dll_base = find_own_dll_base()?;

    // SAFETY: dll_base is our module's base (validated via VirtualQuery).
    // PE headers are valid mapped memory. DOS/PE sigs validated before use.
    unsafe {
        let dos = dll_base;

        if *(dos as *const u16) != 0x5A4D {
            return Err(super::StealthError::TextSectionNotFound);
        }

        let pe_offset = *(dos.add(0x3C) as *const u32) as usize;
        let pe = dos.add(pe_offset);

        if *(pe as *const u32) != 0x0000_4550 {
            return Err(super::StealthError::TextSectionNotFound);
        }

        let coff = pe.add(4);
        let num_sections = *(coff.add(2) as *const u16) as usize;
        let opt_header_size = *(coff.add(16) as *const u16) as usize;
        let sections = coff.add(20 + opt_header_size);

        for i in 0..num_sections {
            let section = sections.add(i * 40);
            let name = std::slice::from_raw_parts(section, 8);

            if name.starts_with(b".text") {
                let virt_size = *(section.add(8) as *const u32) as usize;
                let virt_addr = *(section.add(12) as *const u32) as usize;
                return Ok(((dll_base as *mut u8).add(virt_addr), virt_size));
            }
        }
    }

    Err(super::StealthError::TextSectionNotFound)
}

/// Find our DLL base via VirtualQuery on a known in-module address.
#[cfg(windows)]
fn find_own_dll_base() -> Result<*const u8, super::StealthError> {
    use windows::Win32::System::Memory::{MEMORY_BASIC_INFORMATION, VirtualQuery};

    let self_addr = init as *const u8 as *const core::ffi::c_void;
    let mut mbi = MEMORY_BASIC_INFORMATION::default();

    // SAFETY: VirtualQuery on our own code address with valid output struct.
    let result = unsafe {
        VirtualQuery(
            Some(self_addr),
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };

    if result == 0 {
        return Err(super::StealthError::TextSectionNotFound);
    }

    let base = mbi.AllocationBase as *const u8;
    if base.is_null() {
        return Err(super::StealthError::TextSectionNotFound);
    }

    Ok(base)
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    /// Serialize all tests that write to `XOR_KEY` or call `xor_region`.
    /// `XOR_KEY` is a shared global mutable static; concurrent writes from
    /// multiple tests racing on the key produce non-deterministic results.
    static XOR_GUARD: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_xor() -> std::sync::MutexGuard<'static, ()> {
        XOR_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn bounds_before_init() {
        let (base, size) = text_section_bounds();
        let _ = (base, size);
    }

    #[test]
    fn encrypt_decrypt_noop_before_init() {
        TEXT_LOCATED.store(false, Ordering::Release);
        encrypt();
        decrypt();
    }

    #[test]
    fn xor_is_self_inverse() {
        let _guard = lock_xor();
        let original: Vec<u8> = (0..35).collect();
        let mut buffer = original.clone();

        unsafe {
            (&raw mut XOR_KEY).write([
                0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA,
                0xBB, 0xCC,
            ]);
            xor_region(buffer.as_mut_ptr(), buffer.len());
        }
        assert_ne!(buffer, original);

        unsafe {
            xor_region(buffer.as_mut_ptr(), buffer.len());
        }
        assert_eq!(buffer, original);
    }

    #[test]
    fn generate_key_nonzero() {
        let _guard = lock_xor();
        generate_key();
        let key = unsafe { (&raw const XOR_KEY).read() };
        assert!(key.iter().any(|&b| b != 0));
    }

    #[test]
    fn xor_empty_region() {
        unsafe {
            xor_region(std::ptr::null_mut(), 0);
        }
    }

    #[test]
    fn xor_single_byte() {
        let _guard = lock_xor();
        let mut buf = [0x42u8];
        unsafe {
            (&raw mut XOR_KEY).write([0xFF; 16]);
            xor_region(buf.as_mut_ptr(), 1);
        }
        assert_eq!(buf[0], 0x42 ^ 0xFF);
        unsafe {
            xor_region(buf.as_mut_ptr(), 1);
        }
        assert_eq!(buf[0], 0x42);
    }

    #[test]
    fn xor_exact_simd_boundary() {
        let _guard = lock_xor();
        let original: Vec<u8> = (0..16).collect();
        let mut buffer = original.clone();

        unsafe {
            (&raw mut XOR_KEY).write([0xDE; 16]);
            xor_region(buffer.as_mut_ptr(), buffer.len());
        }
        assert_ne!(buffer, original);
        unsafe {
            xor_region(buffer.as_mut_ptr(), buffer.len());
        }
        assert_eq!(buffer, original);
    }
}
