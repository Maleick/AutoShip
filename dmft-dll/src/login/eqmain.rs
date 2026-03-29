//! eqmain.dll discovery and pointer resolution.
//!
//! eqmain.dll is loaded into the eqgame.exe process and contains the login UI,
//! CSidlManager, LoginServerAPI, and other pre-game systems.

/// Find eqmain.dll base address in the current process.
/// Returns 0 if not found (eqmain.dll may not be loaded yet during early startup).
pub fn find_eqmain() -> u64 {
    #[cfg(windows)]
    {
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::core::w;

        unsafe {
            GetModuleHandleW(w!("eqmain.dll"))
                .map(|h| h.0 as u64)
                .unwrap_or(0)
        }
    }

    #[cfg(not(windows))]
    {
        // Stub: return the preferred base for macOS dev builds.
        // Login FSM tests don't exercise eqmain discovery.
        0
    }
}

/// Resolve the CSidlManager pointer from eqmain.dll globals.
/// Returns None if the pointer is null or eqmain isn't loaded.
pub fn resolve_sidl_manager(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use dmft_common::offsets::eqmain;

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

/// Resolve the LoginServerAPI pointer from eqmain.dll globals.
pub fn resolve_login_server_api(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use dmft_common::offsets::eqmain;

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

/// Resolve the CXWndManager pointer from eqmain.dll globals.
pub fn resolve_cxwnd_manager(eqmain_base: u64) -> Option<usize> {
    #[cfg(windows)]
    {
        use dmft_common::offsets::eqmain;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_eqmain_returns_zero_on_macos() {
        // On macOS stub builds, eqmain.dll doesn't exist
        let base = find_eqmain();
        // Either 0 (stub) or the preferred base — both are valid
        assert!(base == 0 || base == dmft_common::offsets::eqmain::EQMAIN_PREFERRED_BASE);
    }

    #[test]
    fn resolve_pointers_return_none_on_macos() {
        assert!(resolve_sidl_manager(0x180000000).is_none());
        assert!(resolve_login_server_api(0x180000000).is_none());
        assert!(resolve_cxwnd_manager(0x180000000).is_none());
    }
}
