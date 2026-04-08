use anyhow::{bail, Context, Result};
use std::mem;

/// Handle to an opened process for memory reading.
/// Automatically closes the handle on drop (Windows only).
pub struct ProcessHandle {
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
    /// Process ID of the opened process.
    pub pid: u32,
}

impl ProcessHandle {
    /// Open a process by PID with read access.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    #[cfg(windows)]
    pub fn open(pid: u32) -> Result<Self> {
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };

        let handle =
            unsafe { OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid) }
                .context("Failed to open process")?;

        Ok(Self { handle, pid })
    }

    #[cfg(not(windows))]
    pub fn open(pid: u32) -> Result<Self> {
        tracing::warn!(
            pid,
            "ProcessHandle::open called on non-Windows platform (stub)"
        );
        Ok(Self { pid })
    }

    /// Get the base address of the main executable module in the target process.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    #[cfg(windows)]
    pub fn module_base(&self) -> Result<u64> {
        use windows::Win32::Foundation::HMODULE;
        use windows::Win32::System::ProcessStatus::EnumProcessModules;

        let mut module = HMODULE::default();
        let mut bytes_needed: u32 = 0;
        unsafe {
            EnumProcessModules(
                self.handle,
                &mut module,
                std::mem::size_of::<HMODULE>() as u32,
                &mut bytes_needed,
            )
        }
        .context("EnumProcessModules failed")?;

        Ok(module.0 as u64)
    }

    #[cfg(not(windows))]
    pub fn module_base(&self) -> Result<u64> {
        Ok(0x140000000) // Preferred base fallback
    }

    /// Read a value of type T from the process at the given address.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    #[cfg(windows)]
    pub fn read<T: Copy>(&self, address: usize) -> Result<T> {
        use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;

        let mut buffer: T = unsafe { mem::zeroed() };
        let size = mem::size_of::<T>();
        let mut bytes_read: usize = 0;

        let success = unsafe {
            ReadProcessMemory(
                self.handle,
                address as *const _,
                &mut buffer as *mut T as *mut _,
                size,
                Some(&mut bytes_read),
            )
        };

        match success {
            Ok(()) if bytes_read == size => Ok(buffer),
            Ok(()) => bail!("ReadProcessMemory at {address:#x}: read {bytes_read} of {size} bytes"),
            Err(e) => Err(e).context(format!("ReadProcessMemory failed at {address:#x}")),
        }
    }

    #[cfg(not(windows))]
    pub fn read<T: Copy>(&self, address: usize) -> Result<T> {
        bail!(
            "Cannot read process memory on non-Windows platform (pid={}, addr={:#x}, size={})",
            self.pid,
            address,
            mem::size_of::<T>()
        )
    }

    /// Read a pointer (usize) from the process at the given address.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn read_ptr(&self, address: usize) -> Result<usize> {
        self.read::<u64>(address).map(|v| v as usize)
    }

    /// Chase a pointer chain: read base, then follow each offset.
    /// Example: `chase_ptr(base`, &[0x10, 0x08]) reads *(*base + 0x10) + 0x08
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn chase_ptr(&self, base: usize, offsets: &[usize]) -> Result<usize> {
        let mut addr = base;
        for (i, &offset) in offsets.iter().enumerate() {
            addr = self.read_ptr(addr).with_context(|| {
                format!("chase_ptr: failed at step {i} (addr={addr:#x}, offset={offset:#x})")
            })?;
            addr += offset;
        }
        Ok(addr)
    }

    /// Read N bytes from the process at the given address.
    /// Useful for diagnostic hex dumps when debugging offset issues.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    #[cfg(windows)]
    pub fn read_bytes(&self, address: usize, count: usize) -> Result<Vec<u8>> {
        use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;

        let mut buffer = vec![0u8; count];
        let mut bytes_read: usize = 0;
        unsafe {
            ReadProcessMemory(
                self.handle,
                address as *const _,
                buffer.as_mut_ptr() as *mut _,
                count,
                Some(&mut bytes_read),
            )
        }
        .context(format!("ReadProcessMemory (bytes) failed at {address:#x}"))?;
        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    /// Read N bytes — non-Windows stub.
    #[cfg(not(windows))]
    pub fn read_bytes(&self, address: usize, count: usize) -> Result<Vec<u8>> {
        bail!(
            "Cannot read process memory on non-Windows platform (pid={}, addr={:#x}, count={})",
            self.pid,
            address,
            count
        )
    }

    /// Read a null-terminated string from the process at the given address.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    #[allow(unused_variables, unused_mut)]
    pub fn read_string(&self, address: usize, max_len: usize) -> Result<String> {
        let mut buffer = vec![0u8; max_len];

        #[cfg(windows)]
        {
            use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
            let mut bytes_read: usize = 0;
            unsafe {
                ReadProcessMemory(
                    self.handle,
                    address as *const _,
                    buffer.as_mut_ptr() as *mut _,
                    max_len,
                    Some(&mut bytes_read),
                )
            }
            .context(format!("ReadProcessMemory (string) failed at {address:#x}"))?;
        }

        let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
        Ok(String::from_utf8_lossy(&buffer[..end]).into_owned())
    }
}

#[cfg(windows)]
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        let _ = unsafe { CloseHandle(self.handle) };
    }
}

/// Find all PIDs for processes matching the given name (e.g., "eqgame.exe").
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
pub fn find_processes_by_name(name: &str) -> Result<Vec<u32>> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::ProcessStatus::{EnumProcesses, GetModuleBaseNameW};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    let mut pids = [0u32; 4096];
    let mut bytes_returned: u32 = 0;

    unsafe {
        EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * 4) as u32,
            &mut bytes_returned,
        )
    }
    .context("EnumProcesses failed")?;

    let count = bytes_returned as usize / 4;
    let mut matches = Vec::new();

    for &pid in &pids[..count] {
        if pid == 0 {
            continue;
        }

        let handle =
            unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) };

        if let Ok(handle) = handle {
            let mut buf = [0u16; 260];
            let len = unsafe { GetModuleBaseNameW(handle, None, &mut buf) };

            let _ = unsafe { CloseHandle(handle) };

            if len > 0 {
                let proc_name = String::from_utf16_lossy(&buf[..len as usize]);
                if proc_name.eq_ignore_ascii_case(name) {
                    matches.push(pid);
                }
            }
        }
    }

    Ok(matches)
}

#[cfg(not(windows))]
pub fn find_processes_by_name(name: &str) -> Result<Vec<u32>> {
    tracing::warn!(
        name,
        "find_processes_by_name called on non-Windows platform (stub)"
    );
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    mod non_windows {
        use super::super::*;

        const STUB_TEST_ADDR: usize = 0xDEADBEEF;

        #[test]
        fn non_windows_stubs_return_safe_defaults() {
            let handle = ProcessHandle::open(1234).unwrap();

            assert_eq!(handle.pid, 1234);
            assert_eq!(handle.module_base().unwrap(), 0x140000000);
            assert_eq!(handle.read_string(STUB_TEST_ADDR, 32).unwrap(), "");
            assert!(find_processes_by_name("eqgame.exe").unwrap().is_empty());
        }

        #[test]
        fn non_windows_read_reports_pid_address_and_size() {
            let handle = ProcessHandle::open(42).unwrap();

            let err = handle.read::<u32>(0x1234).unwrap_err();
            let msg = format!("{err:#}");

            assert!(msg.contains("Cannot read process memory on non-Windows platform"));
            assert!(msg.contains("pid=42"));
            assert!(msg.contains("addr=0x1234"));
            assert!(msg.contains("size=4"));
        }

        #[test]
        fn non_windows_read_bytes_reports_pid_address_and_count() {
            let handle = ProcessHandle::open(77).unwrap();

            let err = handle.read_bytes(0xBEEF, 16).unwrap_err();
            let msg = format!("{err:#}");

            assert!(msg.contains("Cannot read process memory on non-Windows platform"));
            assert!(msg.contains("pid=77"));
            assert!(msg.contains("addr=0xbeef"));
            assert!(msg.contains("count=16"));
        }

        #[test]
        fn chase_ptr_adds_step_context_to_read_failures() {
            let handle = ProcessHandle::open(99).unwrap();

            let err = handle.chase_ptr(0x1000, &[0x10, 0x20]).unwrap_err();
            let msg = format!("{err:#}");

            assert!(msg.contains("chase_ptr: failed at step 0"));
            assert!(msg.contains("addr=0x1000"));
            assert!(msg.contains("offset=0x10"));
            assert!(msg.contains("pid=99"));
        }
    }
}
