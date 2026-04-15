//! Syscall table construction via TartarusGate pattern matching.
//!
//! At DLL load, we map a fresh copy of ntdll.dll from the KnownDlls section,
//! walk its export table to extract System Service Numbers (SSNs), then unmap
//! the fresh copy immediately. This avoids reading from the potentially-hooked
//! in-memory ntdll while leaving no suspicious mapped sections behind.

#[allow(unused_imports)] // Used by walk_exports_and_resolve on Windows
use super::hash::djb2;

/// A resolved syscall: the SSN (system service number) and the address of a
/// `syscall; ret` gadget inside the *real* loaded ntdll.
#[derive(Debug, Clone, Copy)]
pub struct SyscallEntry {
    /// System Service Number — the value loaded into EAX before `syscall`.
    pub ssn: u16,
    /// Address of a `syscall; ret` (0F 05 C3) gadget inside the real ntdll.
    /// Used as the indirect jump target so the return address is inside ntdll.
    pub gadget: usize,
}

/// The resolved syscall table. Populated once during DLL init, then read-only.
#[derive(Debug, Default)]
pub struct SyscallTable {
    entries: Vec<(u32, SyscallEntry)>,
}

impl SyscallTable {
    /// Look up a syscall entry by its DJB2 hash.
    pub fn get(&self, hash: u32) -> Option<&SyscallEntry> {
        self.entries
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, e)| e)
    }

    /// Number of resolved syscalls.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── Clean stub pattern ──────────────────────────────────────────────────
// A clean (unhooked) ntdll Zw/Nt stub on x64 looks like:
//   4C 8B D1        mov r10, rcx
//   B8 XX XX 00 00  mov eax, <SSN>
// A hooked stub typically starts with E9 (JMP).

/// Byte pattern for the start of a clean syscall stub: `mov r10, rcx; mov
/// eax,`.
const CLEAN_STUB_PREFIX: [u8; 4] = [0x4C, 0x8B, 0xD1, 0xB8];

/// Byte pattern for the `syscall; ret` gadget we search for inside ntdll.
#[cfg(windows)]
const SYSCALL_RET: [u8; 3] = [0x0F, 0x05, 0xC3];

/// Check if a stub is clean (not hooked). Returns the SSN if clean.
fn extract_ssn(stub: &[u8]) -> Option<u16> {
    if stub.len() < 8 {
        return None;
    }
    // Check for clean stub: 4C 8B D1 B8 XX XX 00 00
    if stub[0..3] == CLEAN_STUB_PREFIX[0..3] && stub[3] == CLEAN_STUB_PREFIX[3] {
        let ssn = u16::from_le_bytes([stub[4], stub[5]]);
        // Verify the upper two bytes are 00 00 (SSNs are < 0x1000 in practice)
        if stub[6] == 0x00 && stub[7] == 0x00 {
            return Some(ssn);
        }
    }
    None
}

// ── TartarusGate: SSN recovery from hooked stubs ────────────────────────
// If the target stub is hooked (starts with E9/JMP), we can still recover
// the SSN by looking at neighboring stubs. SSNs are sequential — if the
// stub above or below is clean, we can derive ours by +/- 1.

/// Attempt to recover a SSN from a hooked stub by checking neighbors.
///
/// `stubs` is a slice of (export_rva, name_hash) sorted by RVA. The stub
/// at index `idx` is hooked. We scan up and down for clean neighbors.
#[cfg(windows)]
fn recover_ssn_tartarus(ntdll_base: *const u8, stubs: &[(usize, u32)], idx: usize) -> Option<u16> {
    // Search downward (higher SSNs)
    for delta in 1..=5 {
        if idx + delta >= stubs.len() {
            break;
        }
        let neighbor_addr = unsafe { ntdll_base.add(stubs[idx + delta].0) };
        let neighbor_bytes = unsafe { std::slice::from_raw_parts(neighbor_addr, 8) };
        if let Some(neighbor_ssn) = extract_ssn(neighbor_bytes) {
            return neighbor_ssn.checked_sub(delta as u16);
        }
    }
    // Search upward (lower SSNs)
    for delta in 1..=5 {
        if delta > idx {
            break;
        }
        let neighbor_addr = unsafe { ntdll_base.add(stubs[idx - delta].0) };
        let neighbor_bytes = unsafe { std::slice::from_raw_parts(neighbor_addr, 8) };
        if let Some(neighbor_ssn) = extract_ssn(neighbor_bytes) {
            return neighbor_ssn.checked_add(delta as u16);
        }
    }
    None
}

/// Find a `syscall; ret` (0F 05 C3) gadget in the .text section of the real
/// loaded ntdll. Returns the virtual address of the gadget.
#[cfg(windows)]
fn find_syscall_ret_gadget(ntdll_base: *const u8) -> Option<usize> {
    // SAFETY: We've validated ntdll_base is a valid PE. We read the PE headers
    // to find the .text section bounds, then search within those bounds.
    unsafe {
        let dos_header = ntdll_base as *const ImageDosHeader;
        let e_lfanew = (*dos_header).e_lfanew;
        if e_lfanew <= 0 {
            return None;
        }

        let nt_headers = ntdll_base.offset(e_lfanew as isize) as *const ImageNtHeaders64;
        let section_count = (*nt_headers).file_header.number_of_sections as usize;
        let first_section = (nt_headers as *const u8).add(std::mem::size_of::<ImageNtHeaders64>())
            as *const ImageSectionHeader;

        for i in 0..section_count {
            let section = &*first_section.add(i);
            // Look for .text section (executable code)
            if section.name[0..5] == *b".text" {
                let text_start = ntdll_base.add(section.virtual_address as usize);
                let text_size = section.virtual_size as usize;

                // Scan for 0F 05 C3 (syscall; ret)
                for offset in 0..text_size.saturating_sub(2) {
                    let ptr = text_start.add(offset);
                    if *ptr == SYSCALL_RET[0]
                        && *ptr.add(1) == SYSCALL_RET[1]
                        && *ptr.add(2) == SYSCALL_RET[2]
                    {
                        return Some(ptr as usize);
                    }
                }
            }
        }
    }
    None
}

// ── PE parsing structures (minimal, no external dep) ────────────────────

#[cfg(windows)]
#[repr(C)]
#[allow(dead_code)]
struct ImageDosHeader {
    e_magic: u16,
    e_cblp: u16,
    e_cp: u16,
    e_crlc: u16,
    e_cparhdr: u16,
    e_minalloc: u16,
    e_maxalloc: u16,
    e_ss: u16,
    e_sp: u16,
    e_csum: u16,
    e_ip: u16,
    e_cs: u16,
    e_lfarlc: u16,
    e_ovno: u16,
    e_res: [u16; 4],
    e_oemid: u16,
    e_oeminfo: u16,
    e_res2: [u16; 10],
    e_lfanew: i32,
}

#[cfg(windows)]
#[repr(C)]
#[allow(dead_code)]
struct ImageNtHeaders64 {
    signature: u32,
    file_header: ImageFileHeader,
    optional_header: ImageOptionalHeader64,
}

#[cfg(windows)]
#[repr(C)]
#[allow(dead_code)]
struct ImageFileHeader {
    machine: u16,
    number_of_sections: u16,
    time_date_stamp: u32,
    pointer_to_symbol_table: u32,
    number_of_symbols: u32,
    size_of_optional_header: u16,
    characteristics: u16,
}

#[cfg(windows)]
#[repr(C)]
#[allow(dead_code)]
struct ImageOptionalHeader64 {
    magic: u16,
    major_linker_version: u8,
    minor_linker_version: u8,
    size_of_code: u32,
    size_of_initialized_data: u32,
    size_of_uninitialized_data: u32,
    address_of_entry_point: u32,
    base_of_code: u32,
    image_base: u64,
    section_alignment: u32,
    file_alignment: u32,
    major_operating_system_version: u16,
    minor_operating_system_version: u16,
    major_image_version: u16,
    minor_image_version: u16,
    major_subsystem_version: u16,
    minor_subsystem_version: u16,
    win32_version_value: u32,
    size_of_image: u32,
    size_of_headers: u32,
    check_sum: u32,
    subsystem: u16,
    dll_characteristics: u16,
    size_of_stack_reserve: u64,
    size_of_stack_commit: u64,
    size_of_heap_reserve: u64,
    size_of_heap_commit: u64,
    loader_flags: u32,
    number_of_rva_and_sizes: u32,
    data_directory: [ImageDataDirectory; 16],
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct ImageDataDirectory {
    virtual_address: u32,
    size: u32,
}

#[cfg(windows)]
#[repr(C)]
#[allow(dead_code)]
struct ImageExportDirectory {
    characteristics: u32,
    time_date_stamp: u32,
    major_version: u16,
    minor_version: u16,
    name: u32,
    base: u32,
    number_of_functions: u32,
    number_of_names: u32,
    address_of_functions: u32,
    address_of_names: u32,
    address_of_name_ordinals: u32,
}

#[cfg(windows)]
#[repr(C)]
#[allow(dead_code)]
struct ImageSectionHeader {
    name: [u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    size_of_raw_data: u32,
    pointer_to_raw_data: u32,
    pointer_to_relocations: u32,
    pointer_to_linenumbers: u32,
    number_of_relocations: u16,
    number_of_linenumbers: u16,
    characteristics: u32,
}

// ── Fresh ntdll mapping + export walk ───────────────────────────────────

/// Build the syscall table by:
/// 1. Mapping a fresh ntdll from KnownDlls (guaranteed unhooked)
/// 2. Walking exports to extract SSNs via clean stub pattern matching
/// 3. Finding a `syscall;ret` gadget in the *real* loaded ntdll
/// 4. Unmapping the fresh copy
///
/// Only resolves functions whose DJB2 hash matches `target_hashes`.
#[cfg(windows)]
pub fn build_syscall_table(target_hashes: &[u32]) -> Result<SyscallTable, SyscallError> {
    use windows::{
        Win32::System::{
            LibraryLoader::GetModuleHandleW,
            Memory::{MEMORY_MAPPED_VIEW_ADDRESS, UnmapViewOfFile},
        },
        core::PCWSTR,
    };

    // 1. Get the real loaded ntdll base (for gadget search).
    let real_ntdll_base = unsafe {
        let name: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
        GetModuleHandleW(PCWSTR(name.as_ptr()))
            .map_err(|_| SyscallError::NtdllNotFound)?
            .0 as *const u8
    };

    // 2. Find syscall;ret gadget in real ntdll.
    let gadget = find_syscall_ret_gadget(real_ntdll_base).ok_or(SyscallError::GadgetNotFound)?;
    tracing::debug!(
        gadget = format!("{:#x}", gadget),
        "Found syscall;ret gadget in ntdll"
    );

    // 3. Map fresh ntdll from KnownDlls via NtOpenSection + MapViewOfFile.
    let fresh_base = map_fresh_ntdll()?;

    // 4. Walk exports and extract SSNs.
    let result = walk_exports_and_resolve(fresh_base, gadget, target_hashes);

    // 5. Always unmap the fresh copy, even on error.
    unsafe {
        let addr = MEMORY_MAPPED_VIEW_ADDRESS {
            Value: fresh_base as *mut _,
        };
        let _ = UnmapViewOfFile(addr);
    }

    result
}

/// Map a fresh, unhooked copy of ntdll.dll from the KnownDlls section.
#[cfg(windows)]
fn map_fresh_ntdll() -> Result<*const u8, SyscallError> {
    use windows::Win32::System::Memory::{FILE_MAP_READ, MapViewOfFile};

    let section_handle = open_knowndlls_section()?;

    let base = unsafe { MapViewOfFile(section_handle, FILE_MAP_READ, 0, 0, 0) };

    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(section_handle);
    }

    let ptr = base.Value as *const u8;
    if ptr.is_null() {
        return Err(SyscallError::MappingFailed);
    }

    // Validate MZ signature.
    if unsafe { *(ptr as *const u16) } != 0x5A4D {
        return Err(SyscallError::InvalidPe);
    }

    tracing::debug!(
        base = format!("{:#x}", ptr as usize),
        "Mapped fresh ntdll from KnownDlls"
    );
    Ok(ptr)
}

/// Open the KnownDlls ntdll.dll section object using NtOpenSection.
///
/// We call NtOpenSection from the *real* loaded ntdll (even if hooked, this
/// particular function is rarely hooked by EDR). This bootstraps our access
/// to the clean copy.
#[cfg(windows)]
fn open_knowndlls_section() -> Result<windows::Win32::Foundation::HANDLE, SyscallError> {
    use windows::{
        Win32::{
            Foundation::HANDLE,
            System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
        },
        core::{PCWSTR, s},
    };

    let ntdll = unsafe {
        let name: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
        GetModuleHandleW(PCWSTR(name.as_ptr())).map_err(|_| SyscallError::NtdllNotFound)?
    };

    let proc = unsafe {
        GetProcAddress(ntdll, s!("NtOpenSection"))
            .ok_or(SyscallError::ProcNotFound("NtOpenSection"))?
    };

    // NtOpenSection signature:
    // NTSTATUS NtOpenSection(PHANDLE, ACCESS_MASK, POBJECT_ATTRIBUTES)
    type NtOpenSectionFn = unsafe extern "system" fn(
        section_handle: *mut HANDLE,
        desired_access: u32,
        object_attributes: *const ObjectAttributes,
    ) -> i32;

    let nt_open_section: NtOpenSectionFn = unsafe { std::mem::transmute(proc) };

    // Build the object path: \KnownDlls\ntdll.dll
    let path: Vec<u16> = "\\KnownDlls\\ntdll.dll".encode_utf16().collect();

    let unicode_str = UnicodeString {
        length: (path.len() * 2) as u16,
        maximum_length: (path.len() * 2) as u16,
        buffer: path.as_ptr(),
    };

    let obj_attrs = ObjectAttributes {
        length: std::mem::size_of::<ObjectAttributes>() as u32,
        root_directory: HANDLE::default(),
        object_name: &unicode_str as *const UnicodeString,
        attributes: 0x40, // OBJ_CASE_INSENSITIVE
        security_descriptor: std::ptr::null(),
        security_quality_of_service: std::ptr::null(),
    };

    let mut handle = HANDLE::default();
    // SECTION_MAP_READ | SECTION_QUERY = 0x4 | 0x1 = 0x5
    let status = unsafe { nt_open_section(&mut handle, 0x5, &obj_attrs) };

    if status < 0 {
        tracing::warn!(
            status = format!("{:#x}", status as u32),
            "NtOpenSection failed"
        );
        return Err(SyscallError::SectionOpenFailed(status));
    }

    Ok(handle)
}

/// Minimal UNICODE_STRING for NT API calls.
#[cfg(windows)]
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

/// Minimal OBJECT_ATTRIBUTES for NT API calls.
#[cfg(windows)]
#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: windows::Win32::Foundation::HANDLE,
    object_name: *const UnicodeString,
    attributes: u32,
    security_descriptor: *const std::ffi::c_void,
    security_quality_of_service: *const std::ffi::c_void,
}

/// Walk the export table of the fresh ntdll and resolve target syscalls.
#[cfg(windows)]
fn walk_exports_and_resolve(
    fresh_base: *const u8,
    gadget: usize,
    target_hashes: &[u32],
) -> Result<SyscallTable, SyscallError> {
    let dos = fresh_base as *const ImageDosHeader;
    let e_lfanew = unsafe { (*dos).e_lfanew };
    if e_lfanew <= 0 {
        return Err(SyscallError::InvalidPe);
    }

    let nt_headers = unsafe { fresh_base.offset(e_lfanew as isize) as *const ImageNtHeaders64 };

    // Export directory is data_directory[0].
    let export_dir_rva = unsafe { (*nt_headers).optional_header.data_directory[0].virtual_address };
    if export_dir_rva == 0 {
        return Err(SyscallError::NoExports);
    }

    let export_dir =
        unsafe { fresh_base.add(export_dir_rva as usize) as *const ImageExportDirectory };

    let num_names = unsafe { (*export_dir).number_of_names } as usize;
    let names_rva = unsafe { (*export_dir).address_of_names } as usize;
    let ordinals_rva = unsafe { (*export_dir).address_of_name_ordinals } as usize;
    let functions_rva = unsafe { (*export_dir).address_of_functions } as usize;

    let name_table = unsafe { fresh_base.add(names_rva) as *const u32 };
    let ordinal_table = unsafe { fresh_base.add(ordinals_rva) as *const u16 };
    let function_table = unsafe { fresh_base.add(functions_rva) as *const u32 };

    // Collect all Nt* / Zw* stubs with their RVAs and hashes, sorted by RVA.
    // SSNs are assigned in RVA order, which is critical for TartarusGate recovery.
    let mut nt_stubs: Vec<(usize, u32)> = Vec::with_capacity(512);

    for i in 0..num_names {
        let name_rva = unsafe { *name_table.add(i) } as usize;
        let name_ptr = unsafe { fresh_base.add(name_rva) };

        // Read the function name as a C string (null-terminated).
        let name_bytes = unsafe {
            let mut len = 0;
            while *name_ptr.add(len) != 0 && len < 256 {
                len += 1;
            }
            std::slice::from_raw_parts(name_ptr, len)
        };

        // Only process Nt* and Zw* functions (syscall stubs).
        if name_bytes.len() >= 2
            && ((name_bytes[0] == b'N' && name_bytes[1] == b't')
                || (name_bytes[0] == b'Z' && name_bytes[1] == b'w'))
        {
            let ordinal = unsafe { *ordinal_table.add(i) } as usize;
            let func_rva = unsafe { *function_table.add(ordinal) } as usize;
            let hash = djb2(name_bytes);
            nt_stubs.push((func_rva, hash));
        }
    }

    // Sort by RVA — SSNs are sequential in this order.
    nt_stubs.sort_by_key(|(rva, _)| *rva);

    // Extract SSNs for our target functions.
    let mut table = SyscallTable {
        entries: Vec::with_capacity(target_hashes.len()),
    };

    for (idx, &(func_rva, hash)) in nt_stubs.iter().enumerate() {
        if !target_hashes.contains(&hash) {
            continue;
        }

        let stub_addr = unsafe { fresh_base.add(func_rva) };
        let stub_bytes = unsafe { std::slice::from_raw_parts(stub_addr, 8) };

        let ssn = if let Some(ssn) = extract_ssn(stub_bytes) {
            ssn
        } else {
            // Hooked stub — use TartarusGate neighbor recovery.
            match recover_ssn_tartarus(fresh_base, &nt_stubs, idx) {
                Some(ssn) => {
                    tracing::warn!(
                        hash = format!("{:#x}", hash),
                        ssn,
                        "Recovered SSN via TartarusGate (stub was hooked)"
                    );
                    ssn
                }
                None => {
                    tracing::error!(
                        hash = format!("{:#x}", hash),
                        "Failed to recover SSN — stub hooked and no clean neighbors"
                    );
                    continue;
                }
            }
        };

        table.entries.push((hash, SyscallEntry { ssn, gadget }));
        tracing::debug!(hash = format!("{:#x}", hash), ssn, "Resolved syscall");
    }

    if table.entries.len() < target_hashes.len() {
        tracing::warn!(
            resolved = table.entries.len(),
            requested = target_hashes.len(),
            "Not all target syscalls were resolved"
        );
    }

    Ok(table)
}

/// macOS stub: returns an empty table (syscalls are Windows-only).
#[cfg(not(windows))]
pub fn build_syscall_table(_target_hashes: &[u32]) -> Result<SyscallTable, SyscallError> {
    Ok(SyscallTable::default())
}

/// Errors that can occur during syscall table construction.
#[derive(Debug, thiserror::Error)]
pub enum SyscallError {
    #[error("ntdll.dll not found in process")]
    NtdllNotFound,

    #[error("failed to find syscall;ret gadget in ntdll")]
    GadgetNotFound,

    #[error("failed to map fresh ntdll from KnownDlls")]
    MappingFailed,

    #[error("NtOpenSection failed with status {0:#x}")]
    SectionOpenFailed(i32),

    #[error("invalid PE header")]
    InvalidPe,

    #[error("no export directory in ntdll")]
    NoExports,

    #[error("GetProcAddress failed for {0}")]
    ProcNotFound(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_ssn_clean_stub() {
        let stub = [0x4C, 0x8B, 0xD1, 0xB8, 0x50, 0x00, 0x00, 0x00];
        assert_eq!(extract_ssn(&stub), Some(0x0050));
    }

    #[test]
    fn extract_ssn_hooked_stub() {
        let stub = [0xE9, 0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00];
        assert_eq!(extract_ssn(&stub), None);
    }

    #[test]
    fn extract_ssn_too_short() {
        let stub = [0x4C, 0x8B, 0xD1];
        assert_eq!(extract_ssn(&stub), None);
    }

    #[test]
    fn extract_ssn_large_ssn() {
        let stub = [0x4C, 0x8B, 0xD1, 0xB8, 0xFF, 0x01, 0x00, 0x00];
        assert_eq!(extract_ssn(&stub), Some(0x01FF));
    }

    #[test]
    fn extract_ssn_invalid_upper_bytes() {
        let stub = [0x4C, 0x8B, 0xD1, 0xB8, 0x50, 0x00, 0x01, 0x00];
        assert_eq!(extract_ssn(&stub), None);
    }

    #[test]
    fn syscall_table_empty() {
        let table = SyscallTable::default();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
        assert!(table.get(0x1234).is_none());
    }

    #[test]
    fn syscall_table_lookup() {
        let table = SyscallTable {
            entries: vec![(
                0xDEAD,
                SyscallEntry {
                    ssn: 0x50,
                    gadget: 0x7FFE0000,
                },
            )],
        };
        assert_eq!(table.len(), 1);
        assert!(!table.is_empty());
        let entry = table.get(0xDEAD).unwrap();
        assert_eq!(entry.ssn, 0x50);
        assert_eq!(entry.gadget, 0x7FFE0000);
        assert!(table.get(0xBEEF).is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn build_stub_table_on_macos() {
        let table = build_syscall_table(&[0x1234]).unwrap();
        assert!(table.is_empty());
    }
}
