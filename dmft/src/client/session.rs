//! EqSession — represents a single managed EQ client.

use super::healing::{ClientHealth, HealthMonitor};
use dmft_common::types::{ClientId, GameState, HookStatus};
use std::path::PathBuf;

/// Post-login setup phases after a client reaches InWorld.
#[derive(Debug, Clone)]
pub enum PostLoginPhase {
    NotStarted,
    JoiningGroup,
    Buffing,
    NavigatingToCamp,
    Ready,
}

/// A single managed EQ client session.
pub struct EqSession {
    pub client_id: ClientId,
    pub pid: u32,
    pub character_name: Option<String>,
    pub hook_status: HookStatus,
    pub dll_path: Option<PathBuf>,
    pub health_monitor: HealthMonitor,
    pub last_state: Option<GameState>,
    pub account_name: Option<String>,
    pub bound_toon: Option<dmft_common::login::AccountInfo>,
    pub post_login_phase: PostLoginPhase,
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
            account_name: None,
            bound_toon: None,
            post_login_phase: PostLoginPhase::NotStarted,
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

            // Cross-check: warn if the in-game character doesn't match the bound toon.
            if let Some(ref bound) = self.bound_toon && player.displayed_name != bound.character_name {
                tracing::warn!(
                    client_id = self.client_id,
                    expected = %bound.character_name,
                    actual = %player.displayed_name,
                    "Character mismatch: in-game name does not match bound toon"
                );
            }
        }
        self.last_state = Some(state);
    }
}
