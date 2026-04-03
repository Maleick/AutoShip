use std::path::Path;

use anyhow::Result;
#[cfg(windows)]
use anyhow::{Context, bail};

#[cfg(windows)]
const INJECTION_TIMEOUT_MS: u32 = 10_000;
#[cfg(windows)]
const DLL_PATH_LEN_LIMIT: usize = 8192;

/// Inject a DLL into a target process by PID.
/// Uses `CreateRemoteThread` + `LoadLibraryW` (classic injection technique).
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
pub fn inject_dll(pid: u32, dll_path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Foundation::WAIT_EVENT;
    use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx,
    };
    use windows::Win32::System::Threading::{
        CreateRemoteThread, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
        PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject,
    };
    const WAIT_OBJECT_0: WAIT_EVENT = WAIT_EVENT(0);
    use windows::core::w;

    use anyhow::Context;

    validate_dll_path(dll_path)?;

    let dll_path_wide: Vec<u16> = dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let dll_path_bytes = dll_path_wide
        .len()
        .checked_mul(2)
        .filter(|&n| n <= DLL_PATH_LEN_LIMIT)
        .ok_or_else(|| anyhow::anyhow!("DLL path too long: {}", dll_path.display()))?;

    // Open target process
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
    .context("Failed to open target process")?;

    let result = (|| -> Result<()> {
        let remote_buf = unsafe {
            VirtualAllocEx(
                process,
                Some(std::ptr::null()),
                dll_path_bytes,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };

        if remote_buf.is_null() {
            bail!("VirtualAllocEx failed");
        }

        // Write DLL path to target process memory
        let write_result = unsafe {
            WriteProcessMemory(
                process,
                remote_buf,
                dll_path_wide.as_ptr() as *const _,
                dll_path_bytes,
                None,
            )
        };
        if let Err(err) = write_result {
            unsafe {
                let _ = VirtualFreeEx(process, remote_buf, 0, MEM_RELEASE);
            }
            return Err(err).context("WriteProcessMemory failed");
        }

        // Get address of LoadLibraryW in kernel32.dll
        let kernel32 = unsafe { GetModuleHandleW(w!("kernel32.dll")) }
            .context("Failed to get kernel32 handle")?;

        // GetProcAddress for LoadLibraryW
        let load_library_addr = unsafe {
            windows::Win32::System::LibraryLoader::GetProcAddress(
                kernel32,
                windows::core::s!("LoadLibraryW"),
            )
        }
        .context("Failed to get LoadLibraryW address")?;

        let load_library_fn: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 =
            unsafe { std::mem::transmute(load_library_addr) };

        // Create remote thread calling LoadLibraryW with our DLL path
        let thread = unsafe {
            CreateRemoteThread(
                process,
                None, // default security
                0,    // default stack size
                Some(load_library_fn),
                Some(remote_buf),
                0,    // run immediately
                None, // don't need thread ID
            )
        }
        .context("CreateRemoteThread failed")?;

        // Wait for the remote thread to complete
        unsafe {
            let wait_result = WaitForSingleObject(thread, INJECTION_TIMEOUT_MS);
            if wait_result != WAIT_OBJECT_0 {
                CloseHandle(thread)?;
                let _ = VirtualFreeEx(process, remote_buf, 0, MEM_RELEASE);
                anyhow::bail!(
                    "DLL injection timed out — LoadLibrary did not complete within the timeout period"
                );
            }
            CloseHandle(thread)?;
        }

        // Free the remote buffer
        unsafe {
            let _ = VirtualFreeEx(process, remote_buf, 0, MEM_RELEASE);
        }

        tracing::info!(pid, dll = %dll_path.display(), "DLL injected successfully");
        Ok(())
    })();

    unsafe {
        let _ = CloseHandle(process);
    }
    result
}

#[cfg(windows)]
fn validate_dll_path(path: &Path) -> Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    if path.as_os_str().is_empty() {
        anyhow::bail!("DLL path is empty");
    }

    let Some(dll_ext) = path.extension().and_then(OsStr::to_str) else {
        anyhow::bail!("DLL path has invalid characters: {}", path.display());
    };
    if !dll_ext.eq_ignore_ascii_case("dll") {
        anyhow::bail!("DLL path must point to a .dll file: {}", path.display());
    }

    if path.as_os_str().encode_wide().any(|wchar| wchar == 0u16) {
        anyhow::bail!(
            "DLL path contains embedded NUL character: {}",
            path.display()
        );
    }

    let meta = std::fs::metadata(path).context("Failed to inspect DLL path")?;
    if !meta.is_file() {
        anyhow::bail!("DLL path is not a regular file: {}", path.display());
    }

    Ok(())
}

/// Eject a DLL from a target process via `CreateRemoteThread(FreeLibrary, module_base)`.
///
/// Finds the DLL's module base address in the target process using a Toolhelp snapshot,
/// then spawns a remote thread calling `FreeLibrary` on that address.
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
#[allow(dead_code)] // Will be used by graceful eject command path
pub fn eject_dll(pid: u32, dll_name: &str) -> Result<()> {
    use anyhow::Context;
    use windows::Win32::Foundation::WAIT_EVENT;
    use windows::Win32::Foundation::{CloseHandle, HMODULE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE,
        TH32CS_SNAPMODULE32,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Threading::{
        CreateRemoteThread, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
        PROCESS_VM_READ, WaitForSingleObject,
    };
    use windows::core::w;
    const WAIT_OBJECT_0: WAIT_EVENT = WAIT_EVENT(0);

    // Snapshot all modules loaded in the target process.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) }
        .context("CreateToolhelp32Snapshot failed")?;

    let dll_name_lower = dll_name.to_ascii_lowercase();
    let mut module_base = HMODULE::default();

    let mut entry = MODULEENTRY32W {
        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };

    // Walk the module list looking for a name that contains dll_name (case-insensitive).
    let found = unsafe {
        if Module32FirstW(snap, &mut entry).is_ok() {
            loop {
                let name: String = entry
                    .szModule
                    .iter()
                    .take_while(|&&c| c != 0)
                    .map(|&c| char::from_u32(u32::from(c)).unwrap_or('?'))
                    .collect::<String>()
                    .to_ascii_lowercase();

                if name.contains(&dll_name_lower) {
                    module_base = HMODULE(entry.modBaseAddr as isize);
                    break true;
                }
                if Module32NextW(snap, &mut entry).is_err() {
                    break false;
                }
            }
        } else {
            false
        }
    };

    unsafe {
        let _ = CloseHandle(snap);
    }

    if !found || module_base.is_invalid() {
        bail!("DLL '{dll_name}' not found in modules of process {pid}");
    }

    // Open target process with thread-creation rights.
    let process = unsafe {
        OpenProcess(
            PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            pid,
        )
    }
    .context("OpenProcess failed for eject")?;

    let result = (|| -> Result<()> {
        // Get FreeLibrary address from kernel32 — identical in all processes on x64 Windows.
        let kernel32 = unsafe { GetModuleHandleW(w!("kernel32.dll")) }
            .context("Failed to get kernel32 handle")?;

        let free_library_addr =
            unsafe { GetProcAddress(kernel32, windows::core::s!("FreeLibrary")) }
                .context("GetProcAddress(FreeLibrary) failed")?;

        let free_library_fn: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 =
            unsafe { std::mem::transmute(free_library_addr) };

        // Spawn a remote thread executing FreeLibrary(module_base).
        let thread = unsafe {
            CreateRemoteThread(
                process,
                None,
                0,
                Some(free_library_fn),
                Some(module_base.0 as *const _),
                0,
                None,
            )
        }
        .context("CreateRemoteThread(FreeLibrary) failed")?;

        unsafe {
            let wait_result = WaitForSingleObject(thread, 5000); // 5s timeout
            let _ = CloseHandle(thread);
            if wait_result != WAIT_OBJECT_0 {
                bail!("DLL ejection timed out waiting for FreeLibrary remote thread");
            }
        }

        tracing::info!(pid, dll = dll_name, "DLL ejected successfully");
        Ok(())
    })();

    unsafe {
        let _ = CloseHandle(process);
    }
    result
}

#[cfg(not(windows))]
pub fn inject_dll(pid: u32, dll_path: &Path) -> Result<()> {
    tracing::warn!(
        pid,
        dll = %dll_path.display(),
        "DLL injection not available on this platform (stub)"
    );
    Ok(())
}

#[cfg(not(windows))]
pub fn eject_dll(pid: u32, dll_name: &str) -> Result<()> {
    tracing::warn!(
        pid,
        dll = dll_name,
        "DLL ejection not available on this platform (stub)"
    );
    Ok(())
}
