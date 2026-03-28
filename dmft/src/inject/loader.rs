use std::path::Path;

use anyhow::Result;
#[cfg(windows)]
use anyhow::bail;

/// Inject a DLL into a target process by PID.
/// Uses CreateRemoteThread + LoadLibraryW (classic injection technique).
#[cfg(windows)]
pub fn inject_dll(pid: u32, dll_path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx,
    };
    use windows::Win32::Foundation::WAIT_EVENT;
    use windows::Win32::System::Threading::{
        CreateRemoteThread, OpenProcess, WaitForSingleObject,
        PROCESS_CREATE_THREAD, PROCESS_VM_OPERATION, PROCESS_VM_WRITE, PROCESS_VM_READ,
        PROCESS_QUERY_INFORMATION,
    };
    const WAIT_OBJECT_0: WAIT_EVENT = WAIT_EVENT(0);
    use windows::core::w;

    use anyhow::Context;

    let dll_path_wide: Vec<u16> = dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let dll_path_bytes = dll_path_wide.len() * 2; // UTF-16 byte count

    // Open target process
    let process = unsafe { OpenProcess(PROCESS_CREATE_THREAD | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid) }
        .context("Failed to open target process")?;

    let result = (|| -> Result<()> {
        // Allocate memory in target process for the DLL path
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
        unsafe {
            WriteProcessMemory(
                process,
                remote_buf,
                dll_path_wide.as_ptr() as *const _,
                dll_path_bytes,
                None,
            )
        }
        .context("WriteProcessMemory failed")?;

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
                None,  // default security
                0,     // default stack size
                Some(std::mem::transmute(load_library_fn)),
                Some(remote_buf),
                0,     // run immediately
                None,  // don't need thread ID
            )
        }
        .context("CreateRemoteThread failed")?;

        // Wait for the remote thread to complete
        unsafe {
            let wait_result = WaitForSingleObject(thread, 10000); // 10s timeout
            if wait_result != WAIT_OBJECT_0 {
                // Don't free remote_buf — safer to leak than crash the target
                CloseHandle(thread)?;
                anyhow::bail!("DLL injection timed out — LoadLibrary did not complete within the timeout period");
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

/// Eject a DLL from a target process (FreeLibrary via CreateRemoteThread).
#[cfg(windows)]
pub fn eject_dll(pid: u32, dll_name: &str) -> Result<()> {
    // TODO: Find module handle in remote process, call FreeLibrary
    tracing::info!(pid, dll = dll_name, "DLL ejection not yet implemented");
    Ok(())
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
