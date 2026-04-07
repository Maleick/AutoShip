//! PoolParty-style thread pool execution.
//!
//! Replaces `CreateThread` / `CreateRemoteThread` with Windows thread pool
//! work items (`CreateThreadpoolWork` + `SubmitThreadpoolWork`). Our init
//! callback runs on a pre-existing OS worker thread — indistinguishable from
//! normal application thread pool activity. No suspicious thread creation
//! events are generated.
//!
//! # References
//!
//! - SafeBreach PoolParty: 8 thread pool injection variants
//! - Windows Thread Pool API: `TP_WORK`, `PTP_WORK_CALLBACK`

/// Submit a function to the process-default thread pool for execution.
///
/// The callback runs asynchronously on a worker thread managed by the OS.
/// Returns immediately; the caller does not block.
///
/// # Safety
///
/// The caller must ensure `callback` is safe to run on a worker thread
/// (no loader-lock dependencies, thread-safe access to shared state).
///
/// # Errors
///
/// Returns an error if the thread pool work item cannot be allocated.
#[cfg(windows)]
pub unsafe fn submit_to_thread_pool(
    callback: unsafe extern "system" fn(
        windows::Win32::System::Threading::PTP_CALLBACK_INSTANCE,
        *mut core::ffi::c_void,
        windows::Win32::System::Threading::PTP_WORK,
    ),
    context: Option<*mut core::ffi::c_void>,
) -> Result<(), PoolPartyError> {
    use windows::Win32::System::Threading::{
        CloseThreadpoolWork, CreateThreadpoolWork, SubmitThreadpoolWork,
    };

    // SAFETY: All three thread pool API calls require a valid work handle.
    // CreateThreadpoolWork allocates it, SubmitThreadpoolWork queues it,
    // CloseThreadpoolWork releases our reference (callback still runs).
    unsafe {
        let work = CreateThreadpoolWork(Some(callback), context, None)
            .map_err(|e| PoolPartyError::AllocFailed(format!("{e}")))?;

        SubmitThreadpoolWork(work);
        CloseThreadpoolWork(work);
    }

    Ok(())
}

#[cfg(all(test, windows))]
unsafe fn submit_to_thread_pool_and_wait(
    callback: unsafe extern "system" fn(
        windows::Win32::System::Threading::PTP_CALLBACK_INSTANCE,
        *mut core::ffi::c_void,
        windows::Win32::System::Threading::PTP_WORK,
    ),
    context: Option<*mut core::ffi::c_void>,
) -> Result<(), PoolPartyError> {
    use windows::Win32::System::Threading::{
        CloseThreadpoolWork, CreateThreadpoolWork, SubmitThreadpoolWork,
        WaitForThreadpoolWorkCallbacks,
    };
    // Deterministic for tests: wait until the queued work item has run.
    unsafe {
        let work = CreateThreadpoolWork(Some(callback), context, None)
            .map_err(|e| PoolPartyError::AllocFailed(format!("{e}")))?;
        SubmitThreadpoolWork(work);
        WaitForThreadpoolWorkCallbacks(work, false);
        CloseThreadpoolWork(work);
    }

    Ok(())
}

/// Errors from PoolParty thread pool execution.
#[derive(Debug, thiserror::Error)]
pub enum PoolPartyError {
    #[error("CreateThreadpoolWork failed: {0}")]
    AllocFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_party_error_display() {
        let err = PoolPartyError::AllocFailed("test error".to_string());
        assert!(err.to_string().contains("CreateThreadpoolWork"));
        assert!(err.to_string().contains("test error"));
    }

    #[cfg(all(windows, feature = "windows-dangerous-tests"))]
    mod windows_tests {
        use super::*;
        use std::sync::atomic::{AtomicBool, Ordering};

        static CALLBACK_FIRED: AtomicBool = AtomicBool::new(false);

        unsafe extern "system" fn test_callback(
            _instance: windows::Win32::System::Threading::PTP_CALLBACK_INSTANCE,
            _context: *mut core::ffi::c_void,
            _work: windows::Win32::System::Threading::PTP_WORK,
        ) {
            CALLBACK_FIRED.store(true, Ordering::Release);
        }

        #[test]
        fn submit_callback_executes() {
            CALLBACK_FIRED.store(false, Ordering::Release);
            unsafe {
                submit_to_thread_pool_and_wait(test_callback, None).expect("submit should succeed");
            }
            assert!(
                CALLBACK_FIRED.load(Ordering::Acquire),
                "Callback should have fired on thread pool"
            );
        }
    }
}
