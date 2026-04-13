//! Anti-cheat related memory readers and status checks.

use crate::process::memory::ProcessHandle;

/// Read the `CheaterLdFlag` anti-cheat indicator from EQ memory.
///
/// Returns `Some(value)` on Windows when the value can be read, and `None` otherwise.
#[cfg(windows)]
pub fn read_cheater_ld_flag(proc: &ProcessHandle, eq_base: u64) -> Option<i32> {
    let flag_addr =
        textquest_common::offsets::rebase(textquest_common::offsets::CHEATER_LD_FLAG_VAR, eq_base)?;
    proc.read::<i32>(flag_addr).ok()
}

/// Non-Windows stub — returns `None` because process memory reads are unsupported.
#[cfg(not(windows))]
pub fn read_cheater_ld_flag(_proc: &ProcessHandle, _eq_base: u64) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_windows_stub_returns_none() {
        // On non-Windows platforms the stub always returns None regardless of inputs.
        let proc = ProcessHandle::open(1).unwrap();
        assert_eq!(read_cheater_ld_flag(&proc, 0x0001_4000_0000), None);
    }

    #[test]
    fn non_windows_stub_zero_base() {
        let proc = ProcessHandle::open(1).unwrap();
        assert_eq!(read_cheater_ld_flag(&proc, 0), None);
    }

    #[test]
    fn non_windows_stub_max_base() {
        let proc = ProcessHandle::open(1).unwrap();
        assert_eq!(read_cheater_ld_flag(&proc, u64::MAX), None);
    }
}
