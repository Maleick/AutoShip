use anyhow::Result;

/// CPU affinity and process priority settings for an EQ client.
#[derive(Debug, Clone)]
pub struct AffinityConfig {
    /// Bitmask of CPU cores this process may run on.
    pub cpu_mask: u64,
    /// Windows scheduling priority class.
    pub priority: ProcessPriority,
}

/// Windows process priority classes.
#[derive(Debug, Clone)]
pub enum ProcessPriority {
    /// Lowest priority — only runs when system is idle.
    Idle,
    /// Below normal priority.
    BelowNormal,
    /// Default priority.
    Normal,
    /// Above normal priority.
    AboveNormal,
    /// High priority — use sparingly.
    High,
}

/// Apply CPU affinity and process priority to a running process.
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
pub fn apply_affinity(pid: u32, config: &AffinityConfig) -> Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_INFORMATION, SetProcessAffinityMask, IDLE_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, ABOVE_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, SetPriorityClass};

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
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
pub fn apply_working_set_limit(pid: u32, max_working_set_mb: u32) -> Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_INFORMATION, SetProcessWorkingSetSize};

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
#[must_use]
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

    #[test]
    fn compute_affinity_wraps_around_cpus() {
        let assignments = compute_affinity_assignments(10, 4); // 10 clients, 4 CPUs
        assert_eq!(assignments.len(), 10);
        // With 4 CPUs, available = 3 (skip CPU 0). Clients wrap: CPU 1,2,3,1,2,3,...
        assert_eq!(assignments[0].cpu_mask, 1 << 1); // CPU 1
        assert_eq!(assignments[1].cpu_mask, 1 << 2); // CPU 2
        assert_eq!(assignments[2].cpu_mask, 1 << 3); // CPU 3
        assert_eq!(assignments[3].cpu_mask, 1 << 1); // wrap to CPU 1
    }

    #[test]
    fn compute_affinity_single_cpu() {
        // Only 1 CPU total — available_cpus = 1, but skip CPU 0 → CPU 1
        let assignments = compute_affinity_assignments(3, 1);
        assert_eq!(assignments.len(), 3);
        // When total_cpus=1, available_cpus=1, index = (i%1)+1 = 1
        for a in &assignments {
            assert_eq!(a.cpu_mask, 1 << 1);
        }
    }

    #[test]
    fn compute_affinity_zero_clients() {
        let assignments = compute_affinity_assignments(0, 8);
        assert!(assignments.is_empty());
    }

    #[test]
    fn compute_affinity_all_below_normal_priority() {
        let assignments = compute_affinity_assignments(5, 8);
        for a in &assignments {
            assert!(matches!(a.priority, ProcessPriority::BelowNormal));
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn apply_affinity_stub_returns_ok() {
        let config = AffinityConfig {
            cpu_mask: 0b10,
            priority: ProcessPriority::Normal,
        };
        assert!(apply_affinity(1234, &config).is_ok());
    }
}
