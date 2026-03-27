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
    use windows::Win32::System::Threading::*;
    use windows::Win32::Foundation::*;

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
    tracing::warn!(pid, mask = config.cpu_mask, "apply_affinity not available (stub)");
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
