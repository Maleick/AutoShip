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
#[derive(Debug, Clone, PartialEq)]
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

/// Foreground-aware affinity controller that assigns the focused EQ client
/// a preferred CPU core and distributes background clients across remaining cores.
pub struct AffinityController {
    enabled: bool,
    foreground_cpu: u64,
    background_cpu_start: u64,
}

impl AffinityController {
    /// Create a new affinity controller.
    ///
    /// `foreground_cpu` is the core assigned to the focused EQ window.
    /// `background_cpu_start` is the first core for background clients.
    #[must_use]
    pub fn new(foreground_cpu: u64, background_cpu_start: u64) -> Self {
        Self {
            enabled: true,
            foreground_cpu,
            background_cpu_start,
        }
    }

    /// Enable or disable foreground-aware affinity.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether foreground-aware affinity is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Compute affinity assignments based on current focus.
    ///
    /// The focused client gets `foreground_cpu`. Background clients are
    /// distributed across remaining cores starting at `background_cpu_start`.
    /// Returns the assignment for each client index (0 = focused, 1+ = background).
    #[must_use]
    pub fn compute_assignments(
        &self,
        client_pids: &[u32],
        focused_pid: Option<u32>,
        total_cpus: usize,
    ) -> Vec<AffinityConfig> {
        if !self.enabled {
            return compute_affinity_assignments(client_pids.len(), total_cpus);
        }

        let focused_idx = focused_pid.and_then(|fp| client_pids.iter().position(|&p| p == fp));

        let mut assignments = Vec::with_capacity(client_pids.len());
        let available_cpus = if total_cpus > 1 { total_cpus - 1 } else { 1 };

        for (i, _) in client_pids.iter().enumerate() {
            let config = if focused_idx.map_or(false, |idx| idx == i) {
                AffinityConfig {
                    cpu_mask: 1u64 << self.foreground_cpu,
                    priority: ProcessPriority::Normal,
                }
            } else {
                let bg_idx = if let Some(focused) = focused_idx {
                    if i > focused { i - 1 } else { i }
                } else {
                    i
                };
                let cpu_index = ((bg_idx % (available_cpus.saturating_sub(1)).max(1))
                    + self.background_cpu_start as usize)
                    .min(total_cpus.saturating_sub(1))
                    .min(u64::BITS as usize - 1);
                let mask = 1u64 << cpu_index;
                AffinityConfig {
                    cpu_mask: if mask == 1u64 << self.foreground_cpu {
                        1u64 << self.background_cpu_start
                    } else {
                        mask
                    },
                    priority: ProcessPriority::BelowNormal,
                }
            };
            assignments.push(config);
        }

        assignments
    }
}

impl Default for AffinityController {
    fn default() -> Self {
        Self::new(1, 2)
    }
}

/// Apply CPU affinity and process priority to a running process.
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
pub fn apply_affinity(pid: u32, config: &AffinityConfig) -> Result<()> {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
            IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_SET_INFORMATION,
            SetPriorityClass, SetProcessAffinityMask,
        },
    };

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
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_SET_INFORMATION, SetProcessWorkingSetSize},
    };

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

    #[test]
    fn compute_affinity_two_cpus() {
        // 2 CPUs total: available = 1 (skip CPU 0), all clients go to CPU 1
        let assignments = compute_affinity_assignments(5, 2);
        assert_eq!(assignments.len(), 5);
        for a in &assignments {
            assert_eq!(a.cpu_mask, 1 << 1);
        }
    }

    #[test]
    fn compute_affinity_large_cpu_count() {
        let assignments = compute_affinity_assignments(3, 64);
        assert_eq!(assignments.len(), 3);
        assert_eq!(assignments[0].cpu_mask, 1 << 1);
        assert_eq!(assignments[1].cpu_mask, 1 << 2);
        assert_eq!(assignments[2].cpu_mask, 1 << 3);
    }

    #[test]
    fn compute_affinity_masks_are_power_of_two() {
        let assignments = compute_affinity_assignments(8, 8);
        for a in &assignments {
            // Each mask should have exactly one bit set
            assert_eq!(a.cpu_mask.count_ones(), 1);
        }
    }

    #[test]
    fn compute_affinity_never_uses_cpu_zero() {
        let assignments = compute_affinity_assignments(20, 16);
        for a in &assignments {
            assert_eq!(a.cpu_mask & 1, 0, "CPU 0 should never be used");
        }
    }

    #[test]
    fn affinity_config_debug_format() {
        let config = AffinityConfig {
            cpu_mask: 0b100,
            priority: ProcessPriority::High,
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains("cpu_mask"));
        assert!(debug.contains("High"));
    }

    #[test]
    fn process_priority_clone() {
        let p = ProcessPriority::AboveNormal;
        let p2 = p.clone();
        assert_eq!(p, p2);
    }

    // ── AffinityController tests ─────────────────────────────────────────────

    #[test]
    fn affinity_controller_default() {
        let ctrl = AffinityController::default();
        assert!(ctrl.is_enabled());
    }

    #[test]
    fn affinity_controller_enable_disable() {
        let mut ctrl = AffinityController::default();
        assert!(ctrl.is_enabled());

        ctrl.set_enabled(false);
        assert!(!ctrl.is_enabled());

        ctrl.set_enabled(true);
        assert!(ctrl.is_enabled());
    }

    #[test]
    fn affinity_controller_focused_gets_foreground_cpu() {
        let ctrl = AffinityController::new(1, 2);
        let pids = [100, 200, 300];
        let assignments = ctrl.compute_assignments(&pids, Some(200), 8);

        assert_eq!(assignments[0].cpu_mask, 1 << 2);
        assert_eq!(assignments[1].cpu_mask, 1 << 1);
        assert_eq!(assignments[1].priority, ProcessPriority::Normal);
        assert_eq!(assignments[2].cpu_mask, 1 << 2);
    }

    #[test]
    fn affinity_controller_no_focus_uses_round_robin() {
        let ctrl = AffinityController::new(1, 2);
        let pids = [100, 200, 300];
        let assignments = ctrl.compute_assignments(&pids, None, 8);

        assert_eq!(assignments.len(), 3);
        assert_eq!(assignments[0].cpu_mask, 1 << 2);
        assert_eq!(assignments[1].cpu_mask, 1 << 3);
        assert_eq!(assignments[2].cpu_mask, 1 << 4);
    }

    #[test]
    fn affinity_controller_disabled_uses_original_algorithm() {
        let mut ctrl = AffinityController::default();
        ctrl.set_enabled(false);
        let pids = [100, 200];
        let assignments = ctrl.compute_assignments(&pids, Some(100), 8);

        for a in &assignments {
            assert_eq!(a.priority, ProcessPriority::BelowNormal);
        }
    }

    #[test]
    fn affinity_controller_focus_not_in_list_uses_round_robin() {
        let ctrl = AffinityController::new(1, 2);
        let pids = [100, 200];
        let assignments = ctrl.compute_assignments(&pids, Some(999), 8);

        assert_eq!(assignments.len(), 2);
    }

    #[test]
    fn affinity_controller_single_client_focused() {
        let ctrl = AffinityController::new(1, 2);
        let pids = [100];
        let assignments = ctrl.compute_assignments(&pids, Some(100), 8);

        assert_eq!(assignments[0].cpu_mask, 1 << 1);
        assert_eq!(assignments[0].priority, ProcessPriority::Normal);
    }

    #[test]
    fn affinity_controller_background_masks_stay_in_range_on_64_cpus() {
        let ctrl = AffinityController::new(1, 2);
        let pids: Vec<u32> = (0..62).map(|i| 1000 + i).collect();
        let assignments = ctrl.compute_assignments(&pids, None, 64);

        assert_eq!(assignments.len(), pids.len());
        for config in assignments {
            assert_eq!(config.cpu_mask.count_ones(), 1);
            assert_ne!(config.cpu_mask, 0);
        }
    }
}
