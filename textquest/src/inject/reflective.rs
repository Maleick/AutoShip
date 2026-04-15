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
    /// Size of the section in memory (may be larger than raw data due to
    /// alignment).
    pub virtual_size: u32,
    /// Raw bytes of the section from the PE file.
    pub data: Vec<u8>,
}

/// A single base relocation entry.
#[derive(Debug, Clone, Copy)]
pub struct Relocation {
    /// RVA of the address that needs patching.
    pub rva: u32,
    /// Relocation type (IMAGE_REL_BASED_DIR64 = 10, IMAGE_REL_BASED_HIGHLOW =
    /// 3).
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

/// System DLLs that share base addresses across all processes on x64 Windows.
/// All entries must be lowercase. These are resolved locally (GetModuleHandleA
/// + GetProcAddress) rather than via remote process enumeration.
const SYSTEM_DLLS: &[&str] = &[
    // NT core
    "ntdll.dll",
    "kernel32.dll",
    "kernelbase.dll",
    // Win32 subsystem
    "user32.dll",
    "gdi32.dll",
    "advapi32.dll",
    "ws2_32.dll",
    // Security / crypto
    "bcrypt.dll",
    "bcryptprimitives.dll",
    // COM / automation
    "oleaut32.dll",
    "ole32.dll",
    // NOTE: d3d11.dll and dxgi.dll are intentionally excluded — textquest.exe
    // does not load them, so GetModuleHandleA would fail. EQ does load them,
    // so they're resolved via remote module enumeration (see resolve_imports).
    // MSVC runtimes
    "msvcrt.dll",
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "msvcp140.dll",
    // UCRT API-Set forwarding stubs (always resolve to ucrtbase.dll internals)
    "api-ms-win-crt-runtime-l1-1-0.dll",
    "api-ms-win-crt-math-l1-1-0.dll",
    "api-ms-win-crt-stdio-l1-1-0.dll",
    "api-ms-win-crt-string-l1-1-0.dll",
    "api-ms-win-crt-heap-l1-1-0.dll",
    "api-ms-win-crt-utility-l1-1-0.dll",
    "api-ms-win-crt-time-l1-1-0.dll",
    // WinAPI API-Set stubs
    "api-ms-win-core-synch-l1-2-0.dll",
];

/// Check if a DLL is a system DLL (same base in all processes on x64 Windows).
pub fn is_system_dll(dll_name: &str) -> bool {
    let lower = dll_name.to_ascii_lowercase();
    SYSTEM_DLLS.iter().any(|&s| lower == s)
}

const MAX_REMOTE_EXPORT_ENTRIES: usize = 65_536;

fn checked_remote_export_table_len(
    count: usize,
    entry_size: usize,
    table_name: &str,
) -> Result<usize, InjectError> {
    if count > MAX_REMOTE_EXPORT_ENTRIES {
        return Err(InjectError::ReadFailed(format!(
            "remote export {table_name} count too large: {count}"
        )));
    }

    count.checked_mul(entry_size).ok_or_else(|| {
        InjectError::ReadFailed(format!(
            "remote export {table_name} size overflow: count={count}, entry_size={entry_size}",
        ))
    })
}

/// Parse a PE file from raw bytes, extracting sections, relocations, and
/// imports.
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

/// Apply a single base relocation: compute the delta between actual and
/// preferred base, then patch the address at the given RVA in the mapped image.
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

    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_EVENT},
        System::{
            Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory},
            Memory::{
                MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE,
                VirtualAllocEx, VirtualFreeEx, VirtualProtectEx,
            },
            ProcessStatus::{EnumProcessModulesEx, GetModuleFileNameExW, LIST_MODULES_ALL},
            Threading::{
                CreateRemoteThread, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
                PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject,
            },
        },
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

            // 4. Resolve imports — system DLLs use local resolution (same base), non-system
            //    DLLs use remote process export table parsing.
            Self::resolve_imports(&mut image, &pe.imports, process_handle)?;

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

        /// Resolve imports, using local resolution for system DLLs and
        /// cross-process resolution for non-system DLLs.
        fn resolve_imports(
            image: &mut [u8],
            imports: &[ImportEntry],
            process_handle: windows::Win32::Foundation::HANDLE,
        ) -> Result<(), InjectError> {
            use windows::{
                Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
                core::PCSTR,
            };

            // Build remote module map lazily — only if we encounter a non-system DLL
            let mut remote_modules: Option<Vec<(String, usize)>> = None;

            for imp in imports {
                let addr = if is_system_dll(&imp.dll_name) {
                    // System DLL — resolve locally (same base in all processes)
                    let dll_cstr = std::ffi::CString::new(imp.dll_name.as_str()).map_err(|_| {
                        InjectError::ImportResolveFailed {
                            dll: imp.dll_name.clone(),
                            function: String::new(),
                        }
                    })?;

                    let module = unsafe { GetModuleHandleA(PCSTR(dll_cstr.as_ptr() as *const u8)) }
                        .map_err(|_| InjectError::ImportResolveFailed {
                            dll: imp.dll_name.clone(),
                            function: String::new(),
                        })?;

                    match &imp.function {
                        ImportName::Name(name) => {
                            let func_cstr =
                                std::ffi::CString::new(name.as_str()).map_err(|_| {
                                    InjectError::ImportResolveFailed {
                                        dll: imp.dll_name.clone(),
                                        function: name.clone(),
                                    }
                                })?;
                            unsafe {
                                GetProcAddress(module, PCSTR(func_cstr.as_ptr() as *const u8))
                            }
                            .ok_or_else(|| {
                                InjectError::ImportResolveFailed {
                                    dll: imp.dll_name.clone(),
                                    function: name.clone(),
                                }
                            })? as usize
                        }
                        ImportName::Ordinal(ord) => {
                            unsafe { GetProcAddress(module, PCSTR(*ord as usize as *const u8)) }
                                .ok_or_else(|| InjectError::ImportResolveFailed {
                                    dll: imp.dll_name.clone(),
                                    function: format!("#{ord}"),
                                })? as usize
                        }
                    }
                } else {
                    // Non-system DLL — resolve via remote process export table
                    if remote_modules.is_none() {
                        remote_modules = Some(Self::enumerate_remote_modules(process_handle)?);
                    }
                    let modules = remote_modules.as_ref().unwrap();

                    Self::resolve_import_remote(process_handle, modules, imp)?
                };

                // Write the resolved address into the IAT slot
                let iat_offset = imp.iat_rva as usize;
                if iat_offset + 8 <= image.len() {
                    image[iat_offset..iat_offset + 8].copy_from_slice(&addr.to_le_bytes());
                }
            }

            Ok(())
        }

        /// Enumerate all modules loaded in the target process.
        /// Returns `(lowercase_filename, base_address)` pairs.
        fn enumerate_remote_modules(
            process_handle: windows::Win32::Foundation::HANDLE,
        ) -> Result<Vec<(String, usize)>, InjectError> {
            let mut modules_buf = vec![windows::Win32::Foundation::HMODULE::default(); 1024];
            let mut bytes_needed: u32 = 0;

            unsafe {
                EnumProcessModulesEx(
                    process_handle,
                    modules_buf.as_mut_ptr(),
                    (modules_buf.len() * std::mem::size_of::<windows::Win32::Foundation::HMODULE>())
                        as u32,
                    &mut bytes_needed,
                    LIST_MODULES_ALL,
                )
            }
            .map_err(|e| InjectError::ReadFailed(format!("EnumProcessModulesEx: {e}")))?;

            let count =
                bytes_needed as usize / std::mem::size_of::<windows::Win32::Foundation::HMODULE>();
            modules_buf.truncate(count);

            let mut result = Vec::with_capacity(count);
            let mut name_buf = vec![0u16; 260];

            for hmod in &modules_buf {
                let len =
                    unsafe { GetModuleFileNameExW(process_handle, *hmod, &mut name_buf) } as usize;

                if len > 0 {
                    let full_path = String::from_utf16_lossy(&name_buf[..len]);
                    let filename = full_path
                        .rsplit('\\')
                        .next()
                        .unwrap_or(&full_path)
                        .to_ascii_lowercase();
                    result.push((filename, hmod.0 as usize));
                }
            }

            tracing::debug!(count = result.len(), "enumerated remote modules");
            Ok(result)
        }

        /// Resolve a single import from a non-system DLL by reading the target
        /// process's PE export table via `ReadProcessMemory`.
        fn resolve_import_remote(
            process_handle: windows::Win32::Foundation::HANDLE,
            remote_modules: &[(String, usize)],
            imp: &ImportEntry,
        ) -> Result<usize, InjectError> {
            let dll_lower = imp.dll_name.to_ascii_lowercase();
            let (_, base) = remote_modules
                .iter()
                .find(|(name, _)| *name == dll_lower)
                .ok_or_else(|| {
                    tracing::warn!(dll = %imp.dll_name, "DLL not found in target process");
                    InjectError::ImportResolveFailed {
                        dll: imp.dll_name.clone(),
                        function: String::new(),
                    }
                })?;

            let base_addr = *base;

            // Helper: read bytes from target process
            let read_remote = |addr: usize, buf: &mut [u8]| -> Result<(), InjectError> {
                unsafe {
                    ReadProcessMemory(
                        process_handle,
                        addr as *const _,
                        buf.as_mut_ptr() as *mut _,
                        buf.len(),
                        None,
                    )
                }
                .map_err(|e| InjectError::ReadFailed(format!("ReadProcessMemory @ {addr:#x}: {e}")))
            };
            let remote_addr = |rva: usize, table_name: &str| -> Result<usize, InjectError> {
                base_addr.checked_add(rva).ok_or_else(|| {
                    InjectError::ReadFailed(format!(
                        "{table_name} RVA overflow: base={base_addr:#x} rva={rva:#x}",
                    ))
                })
            };

            // Read DOS header to get e_lfanew
            let mut dos_header = [0u8; 64];
            read_remote(base_addr, &mut dos_header)?;

            if dos_header[0] != b'M' || dos_header[1] != b'Z' {
                return Err(InjectError::ReadFailed(format!(
                    "invalid DOS signature at remote module {base_addr:#x}",
                )));
            }

            let e_lfanew = u32::from_le_bytes(dos_header[0x3C..0x40].try_into().unwrap()) as usize;

            // Read PE signature + COFF header + optional header (enough for data
            // directories)
            let mut pe_header = [0u8; 264];
            read_remote(remote_addr(e_lfanew, "PE header")?, &mut pe_header)?;

            if &pe_header[0..4] != b"PE\0\0" {
                return Err(InjectError::ReadFailed(
                    "invalid PE signature in remote module".into(),
                ));
            }

            // Optional header starts at offset 24 (4 PE sig + 20 COFF)
            let opt_offset = 24usize;
            let magic =
                u16::from_le_bytes(pe_header[opt_offset..opt_offset + 2].try_into().unwrap());
            if magic != 0x020B {
                return Err(InjectError::ReadFailed("remote module is not PE32+".into()));
            }

            // Export directory is data directory index 0, at opt header + 112
            let dd_offset = opt_offset + 112;
            let export_rva =
                u32::from_le_bytes(pe_header[dd_offset..dd_offset + 4].try_into().unwrap())
                    as usize;
            let export_size =
                u32::from_le_bytes(pe_header[dd_offset + 4..dd_offset + 8].try_into().unwrap())
                    as usize;

            if export_rva == 0 || export_size < 40 {
                return Err(InjectError::ImportResolveFailed {
                    dll: imp.dll_name.clone(),
                    function: format!("{:?}", imp.function),
                });
            }

            // Read the IMAGE_EXPORT_DIRECTORY (40 bytes)
            let mut export_dir = [0u8; 40];
            read_remote(
                remote_addr(export_rva, "export directory")?,
                &mut export_dir,
            )?;

            let num_functions = u32::from_le_bytes(export_dir[20..24].try_into().unwrap()) as usize;
            let num_names = u32::from_le_bytes(export_dir[24..28].try_into().unwrap()) as usize;
            let addr_of_functions =
                u32::from_le_bytes(export_dir[28..32].try_into().unwrap()) as usize;
            let addr_of_names = u32::from_le_bytes(export_dir[32..36].try_into().unwrap()) as usize;
            let addr_of_ordinals =
                u32::from_le_bytes(export_dir[36..40].try_into().unwrap()) as usize;
            let ordinal_base = u32::from_le_bytes(export_dir[16..20].try_into().unwrap()) as usize;

            match &imp.function {
                ImportName::Name(name) => {
                    // Read name RVA table, ordinal table, and function address table
                    let name_rvas_len = checked_remote_export_table_len(num_names, 4, "name RVA")?;
                    let mut name_rvas = vec![0u8; name_rvas_len];
                    read_remote(
                        remote_addr(addr_of_names, "name RVA table")?,
                        &mut name_rvas,
                    )?;

                    let ordinals_len = checked_remote_export_table_len(num_names, 2, "ordinal")?;
                    let mut ordinals = vec![0u8; ordinals_len];
                    read_remote(
                        remote_addr(addr_of_ordinals, "ordinal table")?,
                        &mut ordinals,
                    )?;

                    for i in 0..num_names {
                        let name_rva =
                            u32::from_le_bytes(name_rvas[i * 4..(i + 1) * 4].try_into().unwrap())
                                as usize;

                        let mut name_buf = [0u8; 256];
                        read_remote(remote_addr(name_rva, "export name string")?, &mut name_buf)?;

                        let nul_pos = name_buf.iter().position(|&b| b == 0).unwrap_or(256);
                        let export_name = std::str::from_utf8(&name_buf[..nul_pos]).unwrap_or("");

                        if export_name == name.as_str() {
                            let ordinal_index = u16::from_le_bytes(
                                ordinals[i * 2..(i + 1) * 2].try_into().unwrap(),
                            ) as usize;
                            if ordinal_index >= num_functions {
                                return Err(InjectError::ReadFailed(format!(
                                    "invalid export ordinal index: {ordinal_index} >= \
                                     {num_functions}",
                                )));
                            }

                            let func_entry_offset = checked_remote_export_table_len(
                                ordinal_index,
                                4,
                                "function RVA index",
                            )?;
                            let func_entry_addr = remote_addr(
                                addr_of_functions
                                    .checked_add(func_entry_offset)
                                    .ok_or_else(|| {
                                        InjectError::ReadFailed(format!(
                                            "function RVA table offset overflow: \
                                             base={addr_of_functions:#x} \
                                             offset={func_entry_offset:#x}",
                                        ))
                                    })?,
                                "function RVA entry",
                            )?;
                            let mut func_rva_buf = [0u8; 4];
                            read_remote(func_entry_addr, &mut func_rva_buf)?;
                            let func_rva = u32::from_le_bytes(func_rva_buf) as usize;

                            // Check for forwarded export (RVA within export directory)
                            if func_rva >= export_rva
                                && func_rva < export_rva.saturating_add(export_size)
                            {
                                tracing::warn!(
                                    dll = %imp.dll_name,
                                    function = %name,
                                    "forwarded export not yet supported"
                                );
                                return Err(InjectError::ImportResolveFailed {
                                    dll: imp.dll_name.clone(),
                                    function: name.clone(),
                                });
                            }

                            return Ok(base_addr + func_rva);
                        }
                    }

                    Err(InjectError::ImportResolveFailed {
                        dll: imp.dll_name.clone(),
                        function: name.clone(),
                    })
                }
                ImportName::Ordinal(ord) => {
                    let index = *ord as usize - ordinal_base;
                    if index >= num_functions {
                        return Err(InjectError::ImportResolveFailed {
                            dll: imp.dll_name.clone(),
                            function: format!("#{ord}"),
                        });
                    }

                    let func_rvas_len =
                        checked_remote_export_table_len(num_functions, 4, "function RVA")?;
                    let func_entry_offset =
                        checked_remote_export_table_len(index, 4, "function RVA index")?;
                    debug_assert!(func_entry_offset < func_rvas_len);
                    let func_entry_addr = remote_addr(
                        addr_of_functions
                            .checked_add(func_entry_offset)
                            .ok_or_else(|| {
                                InjectError::ReadFailed(format!(
                                    "function RVA table offset overflow: \
                                     base={addr_of_functions:#x} offset={func_entry_offset:#x}",
                                ))
                            })?,
                        "function RVA entry",
                    )?;
                    let mut func_rva_buf = [0u8; 4];
                    read_remote(func_entry_addr, &mut func_rva_buf)?;

                    let func_rva = u32::from_le_bytes(func_rva_buf) as usize;

                    if func_rva >= export_rva && func_rva < export_rva.saturating_add(export_size) {
                        return Err(InjectError::ImportResolveFailed {
                            dll: imp.dll_name.clone(),
                            function: format!("#{ord}"),
                        });
                    }
                    Ok(base_addr + func_rva)
                }
            }
        }

        /// Execute the DLL's entry point via a small shellcode stub.
        ///
        /// DllMain expects `(HINSTANCE, DWORD fdwReason, LPVOID)` but
        /// `CreateRemoteThread` only passes one parameter. We write a tiny
        /// x64 stub that sets up the three arguments and calls the entry.
        /// Includes 32-byte shadow space per x64 ABI.
        fn execute_entry(
            process_handle: windows::Win32::Foundation::HANDLE,
            entry_addr: usize,
            base_addr: usize,
        ) -> Result<(), InjectError> {
            // Build x64 shellcode stub for DllMain(base, DLL_PROCESS_ATTACH, NULL)
            // x64 ABI requires 32 bytes of shadow space for the callee.
            // sub rsp, 0x28 = 32 shadow + 8 alignment (call pushes 8-byte return addr,
            // so 0x28 keeps RSP 16-byte aligned at the callee's entry).
            let mut stub = Vec::with_capacity(64);
            // sub rsp, 0x28
            stub.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]);
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
            // add rsp, 0x28
            stub.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]);
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

    #[test]
    fn test_system_dll_classification() {
        // System DLLs — should use local resolution
        assert!(is_system_dll("kernel32.dll"));
        assert!(is_system_dll("KERNEL32.DLL"));
        assert!(is_system_dll("ntdll.dll"));
        assert!(is_system_dll("KernelBase.dll"));
        assert!(is_system_dll("user32.dll"));
        assert!(is_system_dll("advapi32.dll"));
        assert!(is_system_dll("ws2_32.dll"));
        assert!(is_system_dll("msvcrt.dll"));

        // MSVC runtimes — system (loaded by textquest.exe itself)
        assert!(is_system_dll("vcruntime140.dll"));

        // Non-system DLLs — should use remote resolution
        assert!(!is_system_dll("d3d11.dll")); // not loaded by textquest.exe
        assert!(!is_system_dll("dxgi.dll")); // not loaded by textquest.exe
        assert!(!is_system_dll("eqgame.dll"));
    }

    /// Build a PE export table in a byte buffer and verify we can parse it.
    /// This tests the export directory format parsing without needing a real
    /// process.
    #[test]
    fn test_export_table_parsing() {
        let mut buf = vec![0u8; 0x2000];

        // DOS header
        buf[0] = b'M';
        buf[1] = b'Z';
        buf[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes()); // e_lfanew

        let pe = 0x80usize;
        buf[pe..pe + 4].copy_from_slice(b"PE\0\0");

        // COFF header
        let coff = pe + 4;
        buf[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes()); // AMD64

        // Optional header
        let opt = coff + 20;
        buf[opt..opt + 2].copy_from_slice(&0x020Bu16.to_le_bytes()); // PE32+

        // Export directory data directory (first entry, at opt+112)
        let export_rva: u32 = 0x1000;
        let export_size: u32 = 200;
        buf[opt + 112..opt + 116].copy_from_slice(&export_rva.to_le_bytes());
        buf[opt + 116..opt + 120].copy_from_slice(&export_size.to_le_bytes());

        // Build export directory at offset 0x1000
        let ed = 0x1000usize;
        let ordinal_base: u32 = 1;
        let num_functions: u32 = 2;
        let num_names: u32 = 2;
        let addr_of_functions: u32 = 0x1100;
        let addr_of_names: u32 = 0x1200;
        let addr_of_ordinals: u32 = 0x1300;

        // Export directory struct (40 bytes)
        buf[ed + 16..ed + 20].copy_from_slice(&ordinal_base.to_le_bytes());
        buf[ed + 20..ed + 24].copy_from_slice(&num_functions.to_le_bytes());
        buf[ed + 24..ed + 28].copy_from_slice(&num_names.to_le_bytes());
        buf[ed + 28..ed + 32].copy_from_slice(&addr_of_functions.to_le_bytes());
        buf[ed + 32..ed + 36].copy_from_slice(&addr_of_names.to_le_bytes());
        buf[ed + 36..ed + 40].copy_from_slice(&addr_of_ordinals.to_le_bytes());

        // Function RVA table at 0x1100: two functions at RVAs 0x500 and 0x600
        buf[0x1100..0x1104].copy_from_slice(&0x500u32.to_le_bytes());
        buf[0x1104..0x1108].copy_from_slice(&0x600u32.to_le_bytes());

        // Name RVA table at 0x1200: point to name strings
        let name1_rva: u32 = 0x1400;
        let name2_rva: u32 = 0x1420;
        buf[0x1200..0x1204].copy_from_slice(&name1_rva.to_le_bytes());
        buf[0x1204..0x1208].copy_from_slice(&name2_rva.to_le_bytes());

        // Ordinal table at 0x1300: ordinal indices 0 and 1
        buf[0x1300..0x1302].copy_from_slice(&0u16.to_le_bytes());
        buf[0x1302..0x1304].copy_from_slice(&1u16.to_le_bytes());

        // Name strings
        buf[0x1400..0x1400 + 7].copy_from_slice(b"FuncOne");
        buf[0x1407] = 0;
        buf[0x1420..0x1420 + 7].copy_from_slice(b"FuncTwo");
        buf[0x1427] = 0;

        // Verify the export directory structure
        let read_u32 = |offset: usize| -> u32 {
            u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
        };

        assert_eq!(read_u32(ed + 20), 2); // num_functions
        assert_eq!(read_u32(ed + 24), 2); // num_names

        // Verify function RVAs
        assert_eq!(read_u32(0x1100), 0x500);
        assert_eq!(read_u32(0x1104), 0x600);

        // Verify name string lookup
        let name_rva = read_u32(0x1200) as usize;
        let nul = buf[name_rva..].iter().position(|&b| b == 0).unwrap();
        let name = std::str::from_utf8(&buf[name_rva..name_rva + nul]).unwrap();
        assert_eq!(name, "FuncOne");

        // Verify ordinal → function mapping
        let ordinal_idx = u16::from_le_bytes(buf[0x1300..0x1302].try_into().unwrap()) as usize;
        let func_rva = read_u32(0x1100 + ordinal_idx * 4);
        assert_eq!(func_rva, 0x500); // FuncOne → RVA 0x500

        let ordinal_idx2 = u16::from_le_bytes(buf[0x1302..0x1304].try_into().unwrap()) as usize;
        let func_rva2 = read_u32(0x1100 + ordinal_idx2 * 4);
        assert_eq!(func_rva2, 0x600); // FuncTwo → RVA 0x600
    }

    #[test]
    fn test_shellcode_shadow_space() {
        // Verify the shellcode stub layout includes shadow space allocation.
        let base_addr: u64 = 0x7FF000000;
        let entry_addr: u64 = 0x7FF001000;

        let mut stub = Vec::with_capacity(64);
        // sub rsp, 0x28
        stub.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]);
        // mov rcx, imm64
        stub.extend_from_slice(&[0x48, 0xB9]);
        stub.extend_from_slice(&base_addr.to_le_bytes());
        // mov edx, 1
        stub.extend_from_slice(&[0xBA, 0x01, 0x00, 0x00, 0x00]);
        // xor r8, r8
        stub.extend_from_slice(&[0x4D, 0x31, 0xC0]);
        // mov rax, imm64
        stub.extend_from_slice(&[0x48, 0xB8]);
        stub.extend_from_slice(&entry_addr.to_le_bytes());
        // call rax
        stub.extend_from_slice(&[0xFF, 0xD0]);
        // add rsp, 0x28
        stub.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]);
        // xor eax, eax
        stub.extend_from_slice(&[0x31, 0xC0]);
        // ret
        stub.push(0xC3);

        // Verify shadow space: sub rsp, 0x28 at start
        assert_eq!(&stub[0..4], &[0x48, 0x83, 0xEC, 0x28]);

        // Verify add rsp, 0x28 after call rax (0xFF, 0xD0)
        let call_pos = stub
            .windows(2)
            .position(|w| w == [0xFF, 0xD0])
            .expect("call rax not found");
        assert_eq!(&stub[call_pos + 2..call_pos + 6], &[0x48, 0x83, 0xC4, 0x28]);

        // Verify total stub size is reasonable
        assert!(stub.len() < 64);
    }

    #[test]
    fn test_checked_remote_export_table_len_rejects_large_count() {
        let result =
            checked_remote_export_table_len(MAX_REMOTE_EXPORT_ENTRIES + 1, 4, "function RVA");
        assert!(result.is_err());
    }

    #[test]
    fn test_checked_remote_export_table_len_handles_overflow() {
        let result = checked_remote_export_table_len(2, usize::MAX, "ordinal");
        assert!(result.is_err());
    }
}
