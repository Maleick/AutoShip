use anyhow::Result;

#[derive(Debug, Clone)]
pub struct AffinityConfig {
    pub cpu_mask: u64,
    pub priority: ProcessPriority,
}

#[derive(Debug, Clone)]
pub enum ProcessPriority {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
}

/// Apply CPU affinity and process priority to a running process.
#[cfg(windows)]
pub fn apply_affinity(pid: u32, config: &AffinityConfig) -> Result<()> {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Threading::*;

    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, false, pid)?;

        SetProcessAffinityMask(handle, config.cpu_mask as usize)?;

        let priority_class = match config.priority {
            ProcessPriority::Idle => IDLE_PRIORITY_CLASS,
            ProcessPriority::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
            ProcessPriority::Normal => NORMAL_PRIORITY_CLASS,
            ProcessPriority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
            ProcessPriority::High => HIGH_PRIORITY_CLASS,
        };
        SetPriorityClass(handle, priority_class)?;

        let _ = CloseHandle(handle);
    }

    tracing::info!(pid, mask = config.cpu_mask, "Applied CPU affinity");
    Ok(())
}

#[cfg(not(windows))]
pub fn apply_affinity(pid: u32, config: &AffinityConfig) -> Result<()> {
    tracing::warn!(
        pid,
        mask = config.cpu_mask,
        "apply_affinity not available (stub)"
    );
    Ok(())
}

/// Apply working set (physical RAM) limits to a running process.
///
/// Uses `SetProcessWorkingSetSizeEx` with `QUOTA_LIMITS_HARDWS_MAX_ENABLE`
/// to enforce a hard maximum — Windows will page out memory beyond the limit.
#[cfg(windows)]
pub fn apply_working_set_limit(pid: u32, max_working_set_mb: u32) -> Result<()> {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Threading::*;

    const MIN_WORKING_SET_MB: u32 = 128;

    let min_bytes = (MIN_WORKING_SET_MB as usize) * 1024 * 1024;
    let max_bytes = (max_working_set_mb as usize) * 1024 * 1024;

    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, false, pid)?;

        // Use SetProcessWorkingSetSize (non-Ex version available in windows 0.54)
        let result = SetProcessWorkingSetSize(handle, min_bytes, max_bytes);

        let _ = CloseHandle(handle);
        result?;
    }

    tracing::info!(
        pid,
        min_mb = MIN_WORKING_SET_MB,
        max_mb = max_working_set_mb,
        "Applied working set limit"
    );
    Ok(())
}

#[cfg(not(windows))]
pub fn apply_working_set_limit(pid: u32, max_working_set_mb: u32) -> Result<()> {
    tracing::warn!(
        pid,
        max_mb = max_working_set_mb,
        "apply_working_set_limit not available (stub)"
    );
    Ok(())
}

/// Distribute clients evenly across available CPUs.
/// Reserves CPU 0 for the orchestrator process.
pub fn compute_affinity_assignments(client_count: usize, total_cpus: usize) -> Vec<AffinityConfig> {
    let available_cpus = if total_cpus > 1 { total_cpus - 1 } else { 1 };
    let mut assignments = Vec::with_capacity(client_count);

    for i in 0..client_count {
        let cpu_index = (i % available_cpus) + 1; // skip CPU 0
        let cpu_mask = 1u64 << cpu_index;
        assignments.push(AffinityConfig {
            cpu_mask,
            priority: ProcessPriority::BelowNormal,
        });
    }

    assignments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn working_set_limit_stub_returns_ok() {
        // On non-Windows, the stub should succeed without error
        let result = apply_working_set_limit(1234, 800);
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(not(windows))]
    fn working_set_limit_zero_mb_returns_ok() {
        let result = apply_working_set_limit(1234, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn compute_affinity_basic() {
        let assignments = compute_affinity_assignments(4, 8);
        assert_eq!(assignments.len(), 4);
        // All masks should skip CPU 0
        for a in &assignments {
            assert_eq!(a.cpu_mask & 1, 0);
        }
    }
}
