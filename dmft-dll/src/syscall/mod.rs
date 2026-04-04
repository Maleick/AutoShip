//! Indirect syscall layer (RecycledGate pattern).
//!
//! This module provides NT API calls that go through a `syscall; ret` gadget
//! inside the real loaded ntdll.dll, making the call stack appear legitimate
//! to EDR/anti-cheat call stack inspection.
//!
//! # Architecture
//!
//! 1. **hash** — DJB2 hashing of function names (no plaintext API strings in binary)
//! 2. **table** — TartarusGate SSN extraction from a fresh ntdll mapped via KnownDlls
//! 3. **gate** — Assembly stubs that JMP to ntdll's `syscall;ret` gadget
//!
//! # Usage
//!
//! Call [`init()`] once during DLL initialization (after tracing is set up).
//! Then use the `gate::nt_*` functions which read from the global syscall table.
//!
//! All code is behind `#[cfg(windows)]` with macOS stubs that return STATUS_SUCCESS.

pub mod gate;
pub mod hash;
pub mod table;

use std::sync::OnceLock;
use table::SyscallTable;

/// Global syscall table, initialized once during DLL startup.
static SYSCALL_TABLE: OnceLock<SyscallTable> = OnceLock::new();

/// Target NT API hashes to resolve at init time.
const TARGET_HASHES: [u32; 4] = [
    hash::NT_PROTECT_VIRTUAL_MEMORY,
    hash::NT_SET_CONTEXT_THREAD,
    hash::NT_GET_CONTEXT_THREAD,
    hash::NT_ALLOCATE_VIRTUAL_MEMORY,
];

/// Initialize the indirect syscall layer.
///
/// Maps a fresh ntdll from KnownDlls, extracts SSNs for target functions,
/// locates a `syscall;ret` gadget in the real ntdll, then unmaps the fresh copy.
///
/// Call once during DLL init, after tracing is configured. Safe to call
/// multiple times (subsequent calls are no-ops).
pub fn init() -> Result<(), table::SyscallError> {
    if SYSCALL_TABLE.get().is_some() {
        return Ok(());
    }

    tracing::info!("Initializing indirect syscall layer (RecycledGate)");

    let tbl = table::build_syscall_table(&TARGET_HASHES)?;

    tracing::info!(
        resolved = tbl.len(),
        total = TARGET_HASHES.len(),
        "Syscall table built"
    );

    for &h in &TARGET_HASHES {
        if tbl.get(h).is_some() {
            tracing::debug!(hash = format!("{:#x}", h), "Syscall resolved");
        } else {
            tracing::warn!(hash = format!("{:#x}", h), "Syscall NOT resolved");
        }
    }

    let _ = SYSCALL_TABLE.set(tbl);
    Ok(())
}

/// Get a reference to the global syscall table.
///
/// Returns `None` if [`init()`] hasn't been called yet.
pub fn table() -> Option<&'static SyscallTable> {
    SYSCALL_TABLE.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_hashes_are_unique() {
        for i in 0..TARGET_HASHES.len() {
            for j in (i + 1)..TARGET_HASHES.len() {
                assert_ne!(
                    TARGET_HASHES[i], TARGET_HASHES[j],
                    "Duplicate target hash detected"
                );
            }
        }
    }

    #[test]
    fn target_hashes_match_precomputed() {
        assert_eq!(TARGET_HASHES[0], hash::NT_PROTECT_VIRTUAL_MEMORY);
        assert_eq!(TARGET_HASHES[1], hash::NT_SET_CONTEXT_THREAD);
        assert_eq!(TARGET_HASHES[2], hash::NT_GET_CONTEXT_THREAD);
        assert_eq!(TARGET_HASHES[3], hash::NT_ALLOCATE_VIRTUAL_MEMORY);
    }

    #[cfg(not(windows))]
    #[test]
    fn init_succeeds_on_macos() {
        assert!(init().is_ok());
    }
}
