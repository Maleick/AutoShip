//! EqSession — represents a single managed EQ client.

use super::healing::{ClientHealth, HealthMonitor};
use dmft_common::types::{ClientId, GameState, HookStatus};
use std::path::PathBuf;

/// A single managed EQ client session.
pub struct EqSession {
    pub client_id: ClientId,
    pub pid: u32,
    pub character_name: Option<String>,
    pub hook_status: HookStatus,
    pub dll_path: Option<PathBuf>,
    pub health_monitor: HealthMonitor,
    pub last_state: Option<GameState>,
}

impl EqSession {
    pub fn new(client_id: ClientId, pid: u32) -> Self {
        Self {
            client_id,
            pid,
            character_name: None,
            hook_status: HookStatus::NotInjected,
            dll_path: None,
            health_monitor: HealthMonitor::new(client_id, pid),
            last_state: None,
        }
    }

    /// Whether this session is fully operational (non-mutating snapshot).
    pub fn is_active(&self) -> bool {
        matches!(self.hook_status, HookStatus::HooksActive)
            && matches!(self.health_monitor.current_health(), ClientHealth::Healthy)
    }

    /// Update game state from shared memory.
    pub fn update_state(&mut self, state: GameState) {
        if let Some(ref player) = state.local_player {
            if self.character_name.is_none() {
                self.character_name = Some(player.displayed_name.clone());
                tracing::info!(
                    client_id = self.client_id,
                    name = %player.displayed_name,
                    "Character identified"
                );
            }
        }
        self.last_state = Some(state);
    }
}
