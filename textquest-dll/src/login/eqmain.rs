//! eqmain.dll discovery and pointer resolution.
//!
//! eqmain.dll is loaded into the eqgame.exe process and contains the login UI,
//! `CSidlManager`, `LoginServerAPI`, and other pre-game systems.

/// Find eqmain.dll base address in the current process.
/// Returns 0 if not found (eqmain.dll may not be loaded yet during early
/// startup).
pub fn find_eqmain() -> u64 {
    #[cfg(windows)]
    {
        use windows::{Win32::System::LibraryLoader::GetModuleHandleW, core::w};

        // SAFETY: GetModuleHandleW is always safe to call — it queries the
        // module table for a loaded DLL by name. Returns NULL if not loaded.
        // The handle is used only as an integer base address.
        unsafe { GetModuleHandleW(w!("eqmain.dll")).map_or(0, |h| h.0 as u64) }
    }

    #[cfg(not(windows))]
    {
        // Stub: return the preferred base for macOS dev builds.
        // Login FSM tests don't exercise eqmain discovery.
        0
    }
}

/// Resolve the `CSidlManager` pointer from eqmain.dll globals.
/// Returns None if the pointer is null or eqmain isn't loaded.
pub fn resolve_sidl_manager(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain;

        // SAFETY: addr is a rebased global pointer within eqmain.dll's data
        // section. Dereferencing yields the CSidlManager singleton pointer
        // (null if not yet initialized). eqmain.dll is loaded in-process.
        let addr = eqmain::rebase(eqmain::SIDL_MANAGER, eqmain_base)?;
        let ptr = unsafe { *(addr as *const usize) };
        if ptr == 0 { None } else { Some(ptr) }
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        None
    }
}

/// Resolve the `LoginServerAPI` pointer from eqmain.dll globals.
pub fn resolve_login_server_api(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain;

        // SAFETY: Same pattern as resolve_sidl_manager — rebased global pointer
        // dereference within eqmain.dll's data section.
        let addr = eqmain::rebase(eqmain::LOGIN_SERVER_API, eqmain_base)?;
        let ptr = unsafe { *(addr as *const usize) };
        if ptr == 0 { None } else { Some(ptr) }
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        None
    }
}

/// Resolve the `LoginClient` pointer from eqmain.dll globals.
/// `LoginClient` contains pLoginData (`EQLogin`*) which has the
/// username/password char arrays.
pub fn resolve_login_client(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain;

        // SAFETY: Same pattern — rebased eqmain.dll global pointer dereference.
        let addr = eqmain::rebase(eqmain::PINST_LOGIN_CLIENT, eqmain_base)?;
        let ptr = unsafe { *(addr as *const usize) };
        if ptr == 0 { None } else { Some(ptr) }
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        None
    }
}

/// Resolve the `EQLogin` struct pointer from LoginClient→pLoginData.
/// Returns the address of the `EQLogin` struct which has Login/PW char arrays.
pub fn resolve_eqlogin(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain as eqmain_offsets;

        let login_client = resolve_login_client(eqmain_base)?;
        // SAFETY: login_client is a validated non-null LoginClient*. The
        // LOGINCLIENT_LOGIN_DATA offset (pLoginData field) is a known pointer
        // within LoginClient that points to the EQLogin struct.
        let eqlogin_ptr =
            unsafe { *((login_client + eqmain_offsets::LOGINCLIENT_LOGIN_DATA) as *const usize) };
        if eqlogin_ptr == 0 {
            None
        } else {
            Some(eqlogin_ptr)
        }
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        None
    }
}

/// Resolve the EQ window handle (HWND) from `EQLogin::hEQWnd`.
pub fn resolve_eq_hwnd(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain as eqmain_offsets;

        let eqlogin = resolve_eqlogin(eqmain_base)?;
        // SAFETY: eqlogin is a validated non-null EQLogin*. EQLOGIN_HWND is
        // a known field containing the HWND of EQ's window.
        let hwnd = unsafe { *((eqlogin + eqmain_offsets::EQLOGIN_HWND) as *const usize) };
        if hwnd == 0 { None } else { Some(hwnd) }
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        None
    }
}

/// Resolve the `CXWndManager` pointer from eqmain.dll globals.
pub fn resolve_cxwnd_manager(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain;

        // SAFETY: Same pattern — rebased eqmain.dll global pointer dereference.
        let addr = eqmain::rebase(eqmain::CXWND_MANAGER, eqmain_base)?;
        let ptr = unsafe { *(addr as *const usize) };
        if ptr == 0 { None } else { Some(ptr) }
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
        None
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn find_eqmain_returns_zero_on_macos() {
        // On macOS stub builds, eqmain.dll doesn't exist
        let base = find_eqmain();
        // Either 0 (stub) or the preferred base — both are valid
        assert!(base == 0 || base == textquest_common::offsets::eqmain::EQMAIN_PREFERRED_BASE);
    }

    #[test]
    fn resolve_pointers_return_none_on_macos() {
        assert!(resolve_sidl_manager(0x180000000).is_none());
        assert!(resolve_login_server_api(0x180000000).is_none());
        assert!(resolve_cxwnd_manager(0x180000000).is_none());
        assert!(resolve_login_client(0x180000000).is_none());
        assert!(resolve_eqlogin(0x180000000).is_none());
        assert!(resolve_eq_hwnd(0x180000000).is_none());
    }
}
