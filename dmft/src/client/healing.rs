//! Self-healing: monitors EQ client health and auto-recovers from crashes.

use dmft_common::types::ClientId;
use std::time::{Duration, Instant};

/// Health status of a monitored EQ client.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientHealth {
    /// Client is responding to pings normally.
    Healthy,
    /// Client stopped responding to pings.
    Unresponsive {
        /// When the client was first detected as unresponsive.
        since: Instant,
    },
    /// Client process has exited.
    Crashed,
    /// Client is being restarted by the self-healing monitor.
    Restarting,
}

/// Monitors an EQ client's health via IPC ping/pong.
pub struct HealthMonitor {
    client_id: ClientId,
    pid: u32,
    last_pong: Instant,
    health: ClientHealth,
    ping_interval: Duration,
    timeout: Duration,
    restart_count: u32,
    max_restarts: u32,
}

impl HealthMonitor {
    /// Creates a new health monitor for the given client and process.
    #[must_use]
    pub fn new(client_id: ClientId, pid: u32) -> Self {
        Self {
            client_id,
            pid,
            last_pong: Instant::now(),
            health: ClientHealth::Healthy,
            ping_interval: Duration::from_secs(5),
            timeout: Duration::from_secs(15),
            restart_count: 0,
            max_restarts: 10,
        }
    }

    /// Check if the process is still running.
    #[must_use]
    pub fn is_process_alive(&self) -> bool {
        is_process_running(self.pid)
    }

    /// Record a successful pong response.
    pub fn record_pong(&mut self) {
        self.last_pong = Instant::now();
        self.health = ClientHealth::Healthy;
    }

    /// Check health and return current status.
    pub fn check(&mut self) -> &ClientHealth {
        if !self.is_process_alive() {
            self.health = ClientHealth::Crashed;
        } else if self.last_pong.elapsed() > self.timeout {
            self.health = ClientHealth::Unresponsive {
                since: self.last_pong + self.timeout,
            };
        }
        &self.health
    }

    /// Return current health without re-checking (non-mutating).
    #[must_use]
    pub fn current_health(&self) -> &ClientHealth {
        &self.health
    }

    /// Whether this client should be restarted.
    #[must_use]
    pub fn should_restart(&self) -> bool {
        matches!(
            self.health,
            ClientHealth::Crashed | ClientHealth::Unresponsive { .. }
        ) && self.restart_count < self.max_restarts
    }

    /// Record that a restart was initiated.
    pub fn record_restart(&mut self) {
        self.restart_count += 1;
        self.health = ClientHealth::Restarting;
        tracing::info!(
            client_id = self.client_id,
            restart_count = self.restart_count,
            "Restarting EQ client"
        );
    }

    /// Update PID after a restart.
    pub fn update_pid(&mut self, new_pid: u32) {
        self.pid = new_pid;
        self.last_pong = Instant::now();
        self.health = ClientHealth::Healthy;
    }

    /// The client ID being monitored.
    #[must_use]
    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// The OS process ID of the monitored client.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// How many times this client has been restarted.
    #[must_use]
    pub fn restart_count(&self) -> u32 {
        self.restart_count
    }

    /// The interval between health check pings.
    #[must_use]
    pub fn ping_interval(&self) -> Duration {
        self.ping_interval
    }
}

/// Check if a process is still running by PID.
#[cfg(windows)]
fn is_process_running(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    let result = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    match result {
        Ok(handle) => {
            unsafe {
                let _ = CloseHandle(handle);
            }
            true
        }
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn is_process_running(_pid: u32) -> bool {
    false // stub — EQ only runs on Windows
}

// Process launching has moved to crate::launcher::spawner::spawn_eq_client
