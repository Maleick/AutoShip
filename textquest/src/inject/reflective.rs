//! Reflective DLL injection — maps a DLL from raw bytes into a target process
//! without using `LoadLibrary`, avoiding the most common detection vector.
//!
//! On non-Windows platforms this module compiles to a stub that logs a warning.

use std::fmt;

/// Errors that can occur during reflective injection.
#[derive(Debug)]
pub enum InjectError {
    /// The supplied bytes are not a valid PE file.
    InvalidPe(String),
    /// Failed to allocate memory in the target process.
    AllocFailed,
    /// Failed to write memory into the target process.
    WriteFailed(String),
    /// Failed to read memory from the target process.
    ReadFailed(String),
    /// A required import could not be resolved.
    ImportResolveFailed { dll: String, function: String },
    /// Failed to execute the entry point in the target process.
    EntryPointFailed(String),
    /// Failed to open the target process.
    ProcessOpenFailed(String),
}

impl fmt::Display for InjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPe(msg) => write!(f, "invalid PE: {msg}"),
            Self::AllocFailed => write!(f, "failed to allocate memory in target process"),
            Self::WriteFailed(msg) => write!(f, "write failed: {msg}"),
            Self::ReadFailed(msg) => write!(f, "read failed: {msg}"),
            Self::ImportResolveFailed { dll, function } => {
                write!(f, "unresolved import: {dll}!{function}")
            }
            Self::EntryPointFailed(msg) => write!(f, "entry point execution failed: {msg}"),
            Self::ProcessOpenFailed(msg) => write!(f, "failed to open process: {msg}"),
        }
    }
}

impl std::error::Error for InjectError {}

/// Parsed PE section ready for mapping.
#[derive(Debug, Clone)]
pub struct PeSection {
    /// Section name (e.g. ".text", ".rdata").
    pub name: String,
    /// RVA where this section should be mapped relative to the image base.
    pub virtual_address: u32,
    /// Size of the section in memory (may be larger than raw data due to alignment).
    pub virtual_size: u32,
    /// Raw bytes of the section from the PE file.
    pub data: Vec<u8>,
}

/// A single base relocation entry.
#[derive(Debug, Clone, Copy)]
pub struct Relocation {
    /// RVA of the address that needs patching.
    pub rva: u32,
    /// Relocation type (IMAGE_REL_BASED_DIR64 = 10, IMAGE_REL_BASED_HIGHLOW = 3).
    pub rel_type: u8,
}

/// An import that needs resolving.
#[derive(Debug, Clone)]
pub struct ImportEntry {
    /// DLL name (e.g. "kernel32.dll").
    pub dll_name: String,
    /// Function name or ordinal.
    pub function: ImportName,
    /// RVA of the IAT slot to patch with the resolved address.
    pub iat_rva: u32,
}

/// Import by name or ordinal.
#[derive(Debug, Clone)]
pub enum ImportName {
    Name(String),
    Ordinal(u16),
}

/// Parsed PE metadata needed for reflective loading.
#[derive(Debug)]
pub struct ParsedPe {
    /// Preferred image base from the PE optional header.
    pub image_base: u64,
    /// Total size of the image in memory.
    pub size_of_image: u32,
    /// Size of all headers.
    pub size_of_headers: u32,
    /// RVA of the entry point (DllMain).
    pub entry_point_rva: u32,
    /// Sections to map.
    pub sections: Vec<PeSection>,
    /// Base relocations to apply.
    pub relocations: Vec<Relocation>,
    /// Imports to resolve.
    pub imports: Vec<ImportEntry>,
}

/// Parse a PE file from raw bytes, extracting sections, relocations, and imports.
pub fn parse_pe(dll_bytes: &[u8]) -> Result<ParsedPe, InjectError> {
    use goblin::pe::PE;

    let pe = PE::parse(dll_bytes).map_err(|e| InjectError::InvalidPe(e.to_string()))?;

    if !pe.is_64 {
        return Err(InjectError::InvalidPe(
            "only 64-bit PE files supported".into(),
        ));
    }

    let optional_header = pe
        .header
        .optional_header
        .ok_or_else(|| InjectError::InvalidPe("missing optional header".into()))?;

    let image_base = optional_header.windows_fields.image_base;
    let size_of_image = optional_header.windows_fields.size_of_image;
    let size_of_headers = optional_header.windows_fields.size_of_headers;
    let entry_point_rva = optional_header.standard_fields.address_of_entry_point as u32;

    // Parse sections
    let sections = pe
        .sections
        .iter()
        .map(|sec| {
            let name = String::from_utf8_lossy(
                &sec.name[..sec
                    .name
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(sec.name.len())],
            )
            .into_owned();

            let ptr_raw = sec.pointer_to_raw_data as usize;
            let size_raw = sec.size_of_raw_data as usize;
            let data = if ptr_raw < dll_bytes.len() {
                let end = (ptr_raw + size_raw).min(dll_bytes.len());
                dll_bytes[ptr_raw..end].to_vec()
            } else {
                Vec::new()
            };

            PeSection {
                name,
                virtual_address: sec.virtual_address,
                virtual_size: sec.virtual_size,
                data,
            }
        })
        .collect();

    // Parse base relocations from the raw data directory
    let relocations = parse_base_relocations(dll_bytes, &pe)?;

    // Parse imports
    let imports = pe
        .imports
        .iter()
        .map(|imp| ImportEntry {
            dll_name: imp.dll.to_string(),
            function: if imp.ordinal != 0 && imp.name.is_empty() {
                ImportName::Ordinal(imp.ordinal)
            } else {
                ImportName::Name(imp.name.to_string())
            },
            iat_rva: imp.offset as u32,
        })
        .collect();

    Ok(ParsedPe {
        image_base,
        size_of_image,
        size_of_headers,
        entry_point_rva,
        sections,
        relocations,
        imports,
    })
}

/// Parse base relocation directory from raw PE bytes.
fn parse_base_relocations(
    dll_bytes: &[u8],
    pe: &goblin::pe::PE<'_>,
) -> Result<Vec<Relocation>, InjectError> {
    let optional_header = pe.header.optional_header.as_ref().unwrap();
    let dirs = &optional_header.data_directories;

    // Base relocation is directory index 5
    let reloc_dir = match dirs.get_base_relocation_table() {
        Some(dir) if dir.size > 0 => dir,
        _ => return Ok(Vec::new()),
    };

    // Convert RVA to file offset using section table
    let reloc_rva = reloc_dir.virtual_address as usize;
    let reloc_size = reloc_dir.size as usize;

    let file_offset = rva_to_offset(reloc_rva, &pe.sections).ok_or_else(|| {
        InjectError::InvalidPe("relocation directory RVA not in any section".into())
    })?;

    if file_offset + reloc_size > dll_bytes.len() {
        return Err(InjectError::InvalidPe(
            "relocation directory extends past EOF".into(),
        ));
    }

    let reloc_data = &dll_bytes[file_offset..file_offset + reloc_size];
    let mut relocations = Vec::new();
    let mut cursor = 0;

    while cursor + 8 <= reloc_data.len() {
        let block_rva = u32::from_le_bytes(reloc_data[cursor..cursor + 4].try_into().unwrap());
        let block_size =
            u32::from_le_bytes(reloc_data[cursor + 4..cursor + 8].try_into().unwrap()) as usize;

        if block_size < 8 || cursor + block_size > reloc_data.len() {
            break;
        }

        let entry_count = (block_size - 8) / 2;
        for i in 0..entry_count {
            let entry_offset = cursor + 8 + i * 2;
            let entry = u16::from_le_bytes(
                reloc_data[entry_offset..entry_offset + 2]
                    .try_into()
                    .unwrap(),
            );

            let rel_type = (entry >> 12) as u8;
            let offset = entry & 0x0FFF;

            // Type 0 = padding, skip
            if rel_type != 0 {
                relocations.push(Relocation {
                    rva: block_rva + u32::from(offset),
                    rel_type,
                });
            }
        }

        cursor += block_size;
    }

    Ok(relocations)
}

/// Convert an RVA to a file offset using the section table.
fn rva_to_offset(
    rva: usize,
    sections: &[goblin::pe::section_table::SectionTable],
) -> Option<usize> {
    for sec in sections {
        let sec_rva = sec.virtual_address as usize;
        let sec_size = sec.virtual_size.max(sec.size_of_raw_data) as usize;
        if rva >= sec_rva && rva < sec_rva + sec_size {
            return Some(rva - sec_rva + sec.pointer_to_raw_data as usize);
        }
    }
    None
}

/// Apply a single base relocation: compute the delta between actual and preferred base,
/// then patch the address at the given RVA in the mapped image.
pub fn apply_relocation(
    image: &mut [u8],
    reloc: &Relocation,
    delta: i64,
) -> Result<(), InjectError> {
    let offset = reloc.rva as usize;

    match reloc.rel_type {
        // IMAGE_REL_BASED_DIR64
        10 => {
            if offset + 8 > image.len() {
                return Err(InjectError::InvalidPe(format!(
                    "DIR64 relocation at RVA {:#x} out of bounds",
                    reloc.rva
                )));
            }
            let val = i64::from_le_bytes(image[offset..offset + 8].try_into().unwrap());
            let patched = val.wrapping_add(delta);
            image[offset..offset + 8].copy_from_slice(&patched.to_le_bytes());
        }
        // IMAGE_REL_BASED_HIGHLOW
        3 => {
            if offset + 4 > image.len() {
                return Err(InjectError::InvalidPe(format!(
                    "HIGHLOW relocation at RVA {:#x} out of bounds",
                    reloc.rva
                )));
            }
            let val = i32::from_le_bytes(image[offset..offset + 4].try_into().unwrap());
            #[allow(clippy::cast_possible_truncation)]
            let patched = (i64::from(val).wrapping_add(delta)) as i32;
            image[offset..offset + 4].copy_from_slice(&patched.to_le_bytes());
        }
        // IMAGE_REL_BASED_ABSOLUTE (padding, no-op)
        0 => {}
        other => {
            return Err(InjectError::InvalidPe(format!(
                "unsupported relocation type {other} at RVA {:#x}",
                reloc.rva
            )));
        }
    }
    Ok(())
}

// ── Windows implementation ──────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::*;

    use windows::Win32::Foundation::{CloseHandle, WAIT_EVENT};
    use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE,
        VirtualAllocEx, VirtualFreeEx, VirtualProtectEx,
    };
    use windows::Win32::System::Threading::{
        CreateRemoteThread, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
        PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject,
    };

    const WAIT_OBJECT_0: WAIT_EVENT = WAIT_EVENT(0);
    const ENTRY_TIMEOUT_MS: u32 = 15_000;

    /// Reflective DLL loader — maps a PE from raw bytes into a target process.
    pub struct ReflectiveLoader;

    impl ReflectiveLoader {
        /// Load a DLL from raw bytes into a target process.
        ///
        /// Returns the base address where the image was mapped.
        pub fn load(
            process_handle: windows::Win32::Foundation::HANDLE,
            dll_bytes: &[u8],
        ) -> Result<usize, InjectError> {
            let pe = parse_pe(dll_bytes)?;

            // 1. Allocate memory in target for the full image
            let remote_base = unsafe {
                VirtualAllocEx(
                    process_handle,
                    Some(std::ptr::null()),
                    pe.size_of_image as usize,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                )
            };
            if remote_base.is_null() {
                return Err(InjectError::AllocFailed);
            }

            let remote_base_addr = remote_base as usize;
            let result = Self::map_and_execute(
                &pe,
                dll_bytes,
                process_handle,
                remote_base,
                remote_base_addr,
            );

            if result.is_err() {
                unsafe {
                    let _ = VirtualFreeEx(process_handle, remote_base, 0, MEM_RELEASE);
                }
            }

            result
        }

        fn map_and_execute(
            pe: &ParsedPe,
            dll_bytes: &[u8],
            process_handle: windows::Win32::Foundation::HANDLE,
            remote_base: *mut core::ffi::c_void,
            remote_base_addr: usize,
        ) -> Result<usize, InjectError> {
            // 2. Build the mapped image locally
            let mut image = vec![0u8; pe.size_of_image as usize];

            // Copy headers
            let header_size = pe.size_of_headers as usize;
            if header_size <= dll_bytes.len() && header_size <= image.len() {
                image[..header_size].copy_from_slice(&dll_bytes[..header_size]);
            }

            // Copy sections at their virtual addresses
            for section in &pe.sections {
                let dst_start = section.virtual_address as usize;
                let dst_end = dst_start + section.data.len();
                if dst_end <= image.len() {
                    image[dst_start..dst_end].copy_from_slice(&section.data);
                }
            }

            // 3. Process base relocations
            let delta = remote_base_addr as i64 - pe.image_base as i64;
            for reloc in &pe.relocations {
                apply_relocation(&mut image, reloc, delta)?;
            }

            // 4. Resolve imports — walk the IAT and patch with addresses from
            //    already-loaded modules in the target (kernel32, ntdll, etc.)
            Self::resolve_imports(&mut image, &pe.imports)?;

            // 5. Write the fully prepared image to the target process
            unsafe {
                WriteProcessMemory(
                    process_handle,
                    remote_base,
                    image.as_ptr() as *const _,
                    image.len(),
                    None,
                )
            }
            .map_err(|e| InjectError::WriteFailed(e.to_string()))?;

            // 6. Set executable permissions
            let mut old_protect = windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS(0);
            unsafe {
                VirtualProtectEx(
                    process_handle,
                    remote_base,
                    pe.size_of_image as usize,
                    PAGE_EXECUTE_READWRITE,
                    &mut old_protect,
                )
            }
            .map_err(|e| InjectError::WriteFailed(format!("VirtualProtectEx: {e}")))?;

            // 7. Execute DllMain via CreateRemoteThread
            let entry_addr = remote_base_addr + pe.entry_point_rva as usize;
            Self::execute_entry(process_handle, entry_addr, remote_base_addr)?;

            tracing::info!(
                base = format_args!("{remote_base_addr:#x}"),
                entry = format_args!("{entry_addr:#x}"),
                "reflective DLL injection succeeded"
            );

            Ok(remote_base_addr)
        }

        /// Resolve imports by looking up export addresses from our own process.
        ///
        /// kernel32/ntdll are at the same address in all processes on x64 Windows,
        /// so resolving locally gives correct addresses for the target.
        fn resolve_imports(image: &mut [u8], imports: &[ImportEntry]) -> Result<(), InjectError> {
            use windows::Win32::System::LibraryLoader::{
                GetModuleHandleA, GetProcAddress, LoadLibraryA,
            };
            use windows::core::PCSTR;

            for imp in imports {
                let dll_cstr = std::ffi::CString::new(imp.dll_name.as_str()).map_err(|_| {
                    InjectError::ImportResolveFailed {
                        dll: imp.dll_name.clone(),
                        function: String::new(),
                    }
                })?;

                let dll_pcstr = PCSTR(dll_cstr.as_ptr() as *const u8);

                // Try GetModuleHandle first (already loaded), fall back to
                // LoadLibrary for lazily-loaded DLLs (e.g., d3d11.dll).
                let module = unsafe { GetModuleHandleA(dll_pcstr) }.or_else(|_| {
                    tracing::debug!(dll = %imp.dll_name, "Module not loaded, loading via LoadLibraryA");
                    unsafe { LoadLibraryA(dll_pcstr) }.map(|m| m.into())
                })
                .map_err(|_| InjectError::ImportResolveFailed {
                    dll: imp.dll_name.clone(),
                    function: String::new(),
                })?;

                let addr = match &imp.function {
                    ImportName::Name(name) => {
                        let func_cstr = std::ffi::CString::new(name.as_str()).map_err(|_| {
                            InjectError::ImportResolveFailed {
                                dll: imp.dll_name.clone(),
                                function: name.clone(),
                            }
                        })?;
                        unsafe { GetProcAddress(module, PCSTR(func_cstr.as_ptr() as *const u8)) }
                            .ok_or_else(|| InjectError::ImportResolveFailed {
                                dll: imp.dll_name.clone(),
                                function: name.clone(),
                            })?
                    }
                    ImportName::Ordinal(ord) => {
                        unsafe { GetProcAddress(module, PCSTR(*ord as usize as *const u8)) }
                            .ok_or_else(|| InjectError::ImportResolveFailed {
                                dll: imp.dll_name.clone(),
                                function: format!("#{ord}"),
                            })?
                    }
                };

                // Write the resolved address into the IAT slot
                let iat_offset = imp.iat_rva as usize;
                if iat_offset + 8 <= image.len() {
                    let addr_val = addr as usize;
                    image[iat_offset..iat_offset + 8].copy_from_slice(&addr_val.to_le_bytes());
                }
            }

            Ok(())
        }

        /// Execute the DLL's entry point via a small shellcode stub.
        ///
        /// DllMain expects `(HINSTANCE, DWORD fdwReason, LPVOID)` but
        /// `CreateRemoteThread` only passes one parameter. We write a tiny
        /// x64 stub that sets up the three arguments and calls the entry:
        ///
        /// ```asm
        /// mov  rcx, <base_addr>     ; hinstDLL
        /// mov  edx, 1               ; DLL_PROCESS_ATTACH
        /// xor  r8, r8               ; lpvReserved = NULL
        /// mov  rax, <entry_addr>
        /// call rax
        /// ret
        /// ```
        fn execute_entry(
            process_handle: windows::Win32::Foundation::HANDLE,
            entry_addr: usize,
            base_addr: usize,
        ) -> Result<(), InjectError> {
            // Build x64 shellcode stub for DllMain(base, DLL_PROCESS_ATTACH, NULL)
            let mut stub = Vec::with_capacity(64);
            // mov rcx, imm64 (base_addr = hinstDLL)
            stub.extend_from_slice(&[0x48, 0xB9]);
            stub.extend_from_slice(&(base_addr as u64).to_le_bytes());
            // mov edx, 1 (DLL_PROCESS_ATTACH)
            stub.extend_from_slice(&[0xBA, 0x01, 0x00, 0x00, 0x00]);
            // xor r8, r8 (lpvReserved = NULL)
            stub.extend_from_slice(&[0x4D, 0x31, 0xC0]);
            // mov rax, imm64 (entry_addr)
            stub.extend_from_slice(&[0x48, 0xB8]);
            stub.extend_from_slice(&(entry_addr as u64).to_le_bytes());
            // call rax
            stub.extend_from_slice(&[0xFF, 0xD0]);
            // xor eax, eax (return 0)
            stub.extend_from_slice(&[0x31, 0xC0]);
            // ret
            stub.push(0xC3);

            // Allocate RWX memory in target for the stub
            let stub_mem = unsafe {
                VirtualAllocEx(
                    process_handle,
                    Some(std::ptr::null()),
                    stub.len(),
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_EXECUTE_READWRITE,
                )
            };
            if stub_mem.is_null() {
                return Err(InjectError::EntryPointFailed(
                    "VirtualAllocEx for stub failed".into(),
                ));
            }

            // Write stub to target
            unsafe {
                WriteProcessMemory(
                    process_handle,
                    stub_mem,
                    stub.as_ptr() as *const _,
                    stub.len(),
                    None,
                )
            }
            .map_err(|e| InjectError::EntryPointFailed(format!("WriteProcessMemory stub: {e}")))?;

            // Execute stub via CreateRemoteThread
            let stub_fn: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 =
                unsafe { std::mem::transmute(stub_mem) };

            let thread = unsafe {
                CreateRemoteThread(process_handle, None, 0, Some(stub_fn), None, 0, None)
            }
            .map_err(|e| InjectError::EntryPointFailed(format!("CreateRemoteThread: {e}")))?;

            unsafe {
                let wait = WaitForSingleObject(thread, ENTRY_TIMEOUT_MS);
                let _ = CloseHandle(thread);

                // Clean up stub memory
                let _ = VirtualFreeEx(process_handle, stub_mem, 0, MEM_RELEASE);

                if wait != WAIT_OBJECT_0 {
                    return Err(InjectError::EntryPointFailed(
                        "DllMain did not complete within timeout".into(),
                    ));
                }
            }

            Ok(())
        }
    }

    /// Convenience: open a process and inject from bytes.
    pub fn inject_reflective(pid: u32, dll_bytes: &[u8]) -> Result<usize, InjectError> {
        let process = unsafe {
            OpenProcess(
                PROCESS_CREATE_THREAD
                    | PROCESS_VM_OPERATION
                    | PROCESS_VM_WRITE
                    | PROCESS_VM_READ
                    | PROCESS_QUERY_INFORMATION,
                false,
                pid,
            )
        }
        .map_err(|e| InjectError::ProcessOpenFailed(e.to_string()))?;

        let result = ReflectiveLoader::load(process, dll_bytes);

        unsafe {
            let _ = CloseHandle(process);
        }

        result
    }
}

// ── Non-Windows stub ────────────────────────────────────────────────────────

#[cfg(not(windows))]
mod platform {
    use super::*;

    /// Reflective DLL loader — stub on non-Windows platforms.
    pub struct ReflectiveLoader;

    impl ReflectiveLoader {
        /// Stub: returns `Ok(0)` on non-Windows platforms.
        pub fn load(_process_handle: usize, _dll_bytes: &[u8]) -> Result<usize, InjectError> {
            tracing::warn!("reflective DLL injection not available on this platform (stub)");
            Ok(0)
        }
    }

    /// Stub: returns `Ok(0)` on non-Windows platforms.
    pub fn inject_reflective(_pid: u32, _dll_bytes: &[u8]) -> Result<usize, InjectError> {
        tracing::warn!("reflective DLL injection not available on this platform (stub)");
        Ok(0)
    }
}

pub use platform::{ReflectiveLoader, inject_reflective};

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PE64 in memory for testing PE parsing.
    fn make_minimal_pe64() -> Vec<u8> {
        let mut buf = vec![0u8; 4096];

        // DOS header
        buf[0] = b'M';
        buf[1] = b'Z';
        // e_lfanew at offset 0x3C → points to PE signature
        let pe_offset: u32 = 0x80;
        buf[0x3C..0x40].copy_from_slice(&pe_offset.to_le_bytes());

        let p = pe_offset as usize;

        // PE signature "PE\0\0"
        buf[p] = b'P';
        buf[p + 1] = b'E';
        buf[p + 2] = 0;
        buf[p + 3] = 0;

        // COFF header (20 bytes)
        let coff = p + 4;
        // Machine: AMD64 (0x8664)
        buf[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes());
        // NumberOfSections: 1
        buf[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        // SizeOfOptionalHeader: 240 (PE64)
        buf[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());
        // Characteristics: DLL | EXECUTABLE_IMAGE
        buf[coff + 18..coff + 20].copy_from_slice(&0x2022u16.to_le_bytes());

        // Optional header
        let opt = coff + 20;
        // Magic: PE32+ (0x20B)
        buf[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes());
        // AddressOfEntryPoint at opt+16
        buf[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes());
        // ImageBase at opt+24 (8 bytes for PE64)
        buf[opt + 24..opt + 32].copy_from_slice(&0x0000000140000000u64.to_le_bytes());
        // SectionAlignment at opt+32
        buf[opt + 32..opt + 36].copy_from_slice(&0x1000u32.to_le_bytes());
        // FileAlignment at opt+36
        buf[opt + 36..opt + 40].copy_from_slice(&0x200u32.to_le_bytes());
        // SizeOfImage at opt+56
        buf[opt + 56..opt + 60].copy_from_slice(&0x3000u32.to_le_bytes());
        // SizeOfHeaders at opt+60
        buf[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes());
        // NumberOfRvaAndSizes at opt+108
        buf[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes());

        // Section header (40 bytes each), starts at opt + 240
        let sec = opt + 240;
        // Name: ".text\0\0\0"
        buf[sec..sec + 5].copy_from_slice(b".text");
        // VirtualSize
        buf[sec + 8..sec + 12].copy_from_slice(&0x100u32.to_le_bytes());
        // VirtualAddress
        buf[sec + 12..sec + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        // SizeOfRawData
        buf[sec + 16..sec + 20].copy_from_slice(&0x200u32.to_le_bytes());
        // PointerToRawData
        buf[sec + 20..sec + 24].copy_from_slice(&0x200u32.to_le_bytes());

        // Write some section data at file offset 0x200
        buf[0x200] = 0xCC; // INT3 as placeholder code

        buf
    }

    #[test]
    fn test_parse_minimal_pe() {
        let pe_bytes = make_minimal_pe64();
        let parsed = parse_pe(&pe_bytes).expect("should parse minimal PE64");

        assert_eq!(parsed.image_base, 0x140000000);
        assert_eq!(parsed.entry_point_rva, 0x1000);
        assert_eq!(parsed.size_of_image, 0x3000);
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].name, ".text");
        assert_eq!(parsed.sections[0].virtual_address, 0x1000);
    }

    #[test]
    fn test_parse_rejects_invalid_bytes() {
        let garbage = vec![0u8; 64];
        let result = parse_pe(&garbage);
        assert!(result.is_err());
    }

    #[test]
    fn test_relocation_dir64() {
        let mut image = vec![0u8; 0x2000];
        // Plant a 64-bit address at RVA 0x1000
        let original: u64 = 0x0000000140001234;
        image[0x1000..0x1008].copy_from_slice(&original.to_le_bytes());

        let reloc = Relocation {
            rva: 0x1000,
            rel_type: 10, // IMAGE_REL_BASED_DIR64
        };

        // Actual base is 0x7FF000000 → delta from preferred 0x140000000
        let actual_base: i64 = 0x7FF000000;
        let preferred_base: i64 = 0x140000000;
        let delta = actual_base - preferred_base;

        apply_relocation(&mut image, &reloc, delta).unwrap();

        let patched = u64::from_le_bytes(image[0x1000..0x1008].try_into().unwrap());
        let expected = (original as i64 + delta) as u64;
        assert_eq!(patched, expected);
    }

    #[test]
    fn test_relocation_highlow() {
        let mut image = vec![0u8; 0x2000];
        let original: u32 = 0x40001234;
        image[0x1000..0x1004].copy_from_slice(&original.to_le_bytes());

        let reloc = Relocation {
            rva: 0x1000,
            rel_type: 3, // IMAGE_REL_BASED_HIGHLOW
        };

        let delta: i64 = 0x1000;
        apply_relocation(&mut image, &reloc, delta).unwrap();

        let patched = u32::from_le_bytes(image[0x1000..0x1004].try_into().unwrap());
        assert_eq!(patched, 0x40002234);
    }

    #[test]
    fn test_relocation_absolute_is_noop() {
        let mut image = vec![0u8; 0x100];
        image[0x10] = 0xAB;

        let reloc = Relocation {
            rva: 0x10,
            rel_type: 0, // IMAGE_REL_BASED_ABSOLUTE (padding)
        };

        apply_relocation(&mut image, &reloc, 0x5000).unwrap();
        assert_eq!(image[0x10], 0xAB); // unchanged
    }

    #[test]
    fn test_relocation_out_of_bounds() {
        let mut image = vec![0u8; 0x10];
        let reloc = Relocation {
            rva: 0x0F,
            rel_type: 10, // needs 8 bytes but only 1 available
        };

        let result = apply_relocation(&mut image, &reloc, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_relocation_type() {
        let mut image = vec![0u8; 0x100];
        let reloc = Relocation {
            rva: 0x10,
            rel_type: 7, // unsupported
        };

        let result = apply_relocation(&mut image, &reloc, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_inject_error_display() {
        let err = InjectError::InvalidPe("bad magic".into());
        assert_eq!(err.to_string(), "invalid PE: bad magic");

        let err = InjectError::ImportResolveFailed {
            dll: "kernel32.dll".into(),
            function: "CreateFileW".into(),
        };
        assert_eq!(
            err.to_string(),
            "unresolved import: kernel32.dll!CreateFileW"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn test_stub_returns_ok() {
        let result = inject_reflective(1234, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn test_reflective_loader_stub() {
        let result = ReflectiveLoader::load(0, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pe_section_data_extraction() {
        let pe_bytes = make_minimal_pe64();
        let parsed = parse_pe(&pe_bytes).unwrap();

        // The .text section should contain our 0xCC byte
        assert!(!parsed.sections[0].data.is_empty());
        assert_eq!(parsed.sections[0].data[0], 0xCC);
    }
}
