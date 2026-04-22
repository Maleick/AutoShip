//! Early Bird APC injection — B8 variant.
//!
//! Queues an Asynchronous Procedure Call (APC) to the main thread of a newly
//! created, suspended EQ process before any user-mode code runs. The APC
//! drains during `ntdll!LdrInitializeThunk` — before `WinMain`, before any
//! anti-cheat initialization, before EQ's CRT runs.
//!
//! # Technique summary
//!
//! 1. Spawn `eqgame.exe` suspended (`CREATE_SUSPENDED`).
//! 2. Allocate RW memory in the target, write reflective loader shellcode.
//! 3. Flip allocation to `PAGE_EXECUTE_READ`.
//! 4. Queue an APC pointing at the shellcode on the suspended main thread.
//! 5. Resume the thread — kernel drains APC queue, our loader fires first.
//!
//! See `docs/research/B8-early-bird-apc-injection.md` for full design.
//!
//! # Safety contract
//!
//! All public functions in this module are `unsafe`. Callers must ensure:
//! - `eq_path` is a valid path to `eqgame.exe`.
//! - `shellcode` is position-independent and safe to execute on the target
//!   thread's initial stack before the CRT is initialized.
//! - The caller owns the returned `EarlyBirdHandles` and must call
//!   [`EarlyBirdHandles::close`] when done to avoid handle leaks.

#[cfg(windows)]
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        Diagnostics::Debug::WriteProcessMemory,
        Memory::{
            MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
            VirtualAllocEx, VirtualProtectEx,
        },
        Threading::{
            CREATE_NEW_CONSOLE, CREATE_SUSPENDED, PROCESS_INFORMATION, QueueUserAPC, ResumeThread,
            STARTUPINFOW,
        },
    },
};

/// Handles for an in-flight Early Bird injection.
///
/// Returned by [`spawn_suspended_and_inject`]. The caller is responsible for
/// closing handles via [`EarlyBirdHandles::close`].
#[derive(Debug)]
#[allow(dead_code)]
pub struct EarlyBirdHandles {
    /// Handle to the target process.
    #[cfg(windows)]
    pub process: HANDLE,
    /// Handle to the target process's main (suspended) thread.
    #[cfg(windows)]
    pub thread: HANDLE,
    /// Remote address of the allocated shellcode buffer.
    pub remote_shellcode: *mut core::ffi::c_void,
}

// SAFETY: The caller controls the handles and ensures they are used
// single-threadedly after injection.
#[cfg(windows)]
unsafe impl Send for EarlyBirdHandles {}
#[cfg(windows)]
unsafe impl Sync for EarlyBirdHandles {}

/// Errors returned by the Early Bird APC injection path.
#[derive(Debug, thiserror::Error)]
pub enum ApcError {
    /// Failed to create the target process in suspended state.
    #[error("CreateProcess failed: {0}")]
    CreateProcess(String),

    /// Failed to allocate memory in the target process.
    #[error("VirtualAllocEx failed: {0}")]
    VirtualAllocEx(String),

    /// Failed to write shellcode into the target process.
    #[error("WriteProcessMemory failed: {0}")]
    WriteProcessMemory(String),

    /// Failed to flip shellcode allocation to RX.
    #[error("VirtualProtectEx (RX flip) failed: {0}")]
    VirtualProtectEx(String),

    /// Failed to queue the APC on the target thread.
    #[error("QueueUserAPC failed: {0}")]
    QueueUserAPC(String),

    /// Failed to resume the target thread after APC was queued.
    #[error("ResumeThread failed: {0}")]
    ResumeThread(String),
}

/// Spawn `eqgame.exe` in a suspended state, inject `shellcode` via Early Bird
/// APC, and resume the main thread.
///
/// Returns [`EarlyBirdHandles`] containing open process/thread handles and the
/// remote shellcode address. The caller should call
/// [`EarlyBirdHandles::close`] after the target process has had time to
/// initialize.
///
/// # Safety
///
/// - `eq_path` must be a null-terminated wide string path to `eqgame.exe`.
/// - `shellcode` must be position-independent x86 code executable on the
///   target thread's initial stack, before CRT initialization.
/// - The caller must not free or alias `shellcode` while the target is running.
///
/// # Errors
///
/// Returns [`ApcError`] if any OS call fails.
#[cfg(windows)]
pub unsafe fn spawn_suspended_and_inject(
    eq_path: &[u16],
    shellcode: &[u8],
) -> Result<EarlyBirdHandles, ApcError> {
    // Step 1: Spawn eqgame.exe suspended.
    let mut si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();

    // Build null-terminated wide command line from path (may add args later).
    let mut cmd: Vec<u16> = eq_path.to_vec();
    if cmd.last() != Some(&0) {
        cmd.push(0);
    }

    // SAFETY: CreateProcessW with CREATE_SUSPENDED; pi is zeroed and valid.
    let ok = unsafe {
        windows::Win32::System::Threading::CreateProcessW(
            windows::core::PCWSTR(cmd.as_ptr()),
            None,
            None,
            None,
            false,
            CREATE_SUSPENDED | CREATE_NEW_CONSOLE,
            None,
            None,
            &mut si,
            &mut pi,
        )
    };
    ok.map_err(|e| ApcError::CreateProcess(e.to_string()))?;

    let process = pi.hProcess;
    let thread = pi.hThread;

    // Step 2: Allocate RW memory in the target.
    // SAFETY: process is valid; size is non-zero; flags are standard commit/reserve.
    let remote_buf = unsafe {
        VirtualAllocEx(
            process,
            None,
            shellcode.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    let remote_buf = match remote_buf {
        Ok(ptr) => ptr,
        Err(e) => {
            let _ = unsafe { CloseHandle(thread) };
            let _ = unsafe { CloseHandle(process) };
            return Err(ApcError::VirtualAllocEx(e.to_string()));
        }
    };

    // Step 3: Write shellcode.
    // SAFETY: remote_buf is a valid allocation of at least shellcode.len() bytes.
    let write_result = unsafe {
        WriteProcessMemory(
            process,
            remote_buf,
            shellcode.as_ptr() as *const core::ffi::c_void,
            shellcode.len(),
            None,
        )
    };
    if let Err(e) = write_result {
        let _ = unsafe { CloseHandle(thread) };
        let _ = unsafe { CloseHandle(process) };
        return Err(ApcError::WriteProcessMemory(e.to_string()));
    }

    // Step 4: Flip allocation to RX before resume (no AC running to observe the flip).
    let mut old_protect = PAGE_PROTECTION_FLAGS(0);
    let protect_result = unsafe {
        VirtualProtectEx(
            process,
            remote_buf,
            shellcode.len(),
            PAGE_EXECUTE_READ,
            &mut old_protect,
        )
    };
    if let Err(e) = protect_result {
        let _ = unsafe { CloseHandle(thread) };
        let _ = unsafe { CloseHandle(process) };
        return Err(ApcError::VirtualProtectEx(e.to_string()));
    }

    // Step 5: Queue APC to the suspended main thread.
    //
    // The APC routine signature is `PAPCFUNC = Option<unsafe extern "system" fn(ULONG_PTR)>`.
    // We transmute the remote shellcode pointer to this function type.
    //
    // SAFETY: remote_buf is an executable allocation in the target process;
    // the transmute is safe because PAPCFUNC has the same ABI as our shellcode entry.
    let apc_routine: windows::Win32::System::Threading::PAPCFUNC =
        Some(unsafe { std::mem::transmute(remote_buf) });

    // SAFETY: thread is the suspended main thread; apc_routine points to our shellcode.
    let apc_result = unsafe { QueueUserAPC(apc_routine, thread, 0) };
    if let Err(e) = apc_result {
        let _ = unsafe { CloseHandle(thread) };
        let _ = unsafe { CloseHandle(process) };
        return Err(ApcError::QueueUserAPC(e.to_string()));
    }

    // Step 6: Resume the thread. The OS drains the APC queue before EQ's entry point.
    // SAFETY: thread is a valid suspended thread handle.
    let prev_suspend = unsafe { ResumeThread(thread) };
    if prev_suspend == u32::MAX {
        // ResumeThread returns (DWORD)-1 on failure.
        let _ = unsafe { CloseHandle(thread) };
        let _ = unsafe { CloseHandle(process) };
        return Err(ApcError::ResumeThread(
            windows::core::Error::from_win32().to_string(),
        ));
    }

    Ok(EarlyBirdHandles {
        process,
        thread,
        remote_shellcode: remote_buf,
    })
}

impl EarlyBirdHandles {
    /// Close all open handles held by this struct.
    ///
    /// Call after the target process has initialized and the shellcode has
    /// completed execution. This method consumes `self`, so it can only be
    /// called once for a given `EarlyBirdHandles` value.
    #[cfg(windows)]
    pub fn close(self) {
        // SAFETY: handles are valid and owned by this struct.
        unsafe {
            let _ = CloseHandle(self.thread);
            let _ = CloseHandle(self.process);
        }
        // remote_shellcode is in the target process address space — we do not
        // free it from here. The target process owns that memory.
    }
}

/// No-op stub for non-Windows targets (compile-time compatibility).
#[cfg(not(windows))]
#[allow(dead_code)]
pub fn spawn_suspended_and_inject_stub() -> Result<(), ApcError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the error types implement Display correctly.
    #[test]
    fn apc_error_display() {
        let e = ApcError::CreateProcess("access denied".to_string());
        assert!(format!("{e}").contains("CreateProcess"));

        let e = ApcError::VirtualAllocEx("out of memory".to_string());
        assert!(format!("{e}").contains("VirtualAllocEx"));

        let e = ApcError::WriteProcessMemory("partial write".to_string());
        assert!(format!("{e}").contains("WriteProcessMemory"));

        let e = ApcError::VirtualProtectEx("invalid flags".to_string());
        assert!(format!("{e}").contains("VirtualProtectEx"));

        let e = ApcError::QueueUserAPC("handle invalid".to_string());
        assert!(format!("{e}").contains("QueueUserAPC"));

        let e = ApcError::ResumeThread("thread died".to_string());
        assert!(format!("{e}").contains("ResumeThread"));
    }

    /// Verify ApcError variants are Debug-printable.
    #[test]
    fn apc_error_debug() {
        let e = ApcError::CreateProcess("test".to_string());
        assert!(!format!("{e:?}").is_empty());
    }
}
