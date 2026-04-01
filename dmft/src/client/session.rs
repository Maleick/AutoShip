//! `EqSession` — represents a single managed EQ client.

use super::healing::{ClientHealth, HealthMonitor};
use dmft_common::types::{ClientId, GameState, HookStatus};
use std::path::PathBuf;

/// Post-login setup phases after a client reaches `InWorld`.
#[derive(Debug, Clone)]
pub enum PostLoginPhase {
    /// Post-login setup has not begun.
    NotStarted,
    /// Client is joining the designated group.
    JoiningGroup,
    /// Client is receiving buffs from group members.
    Buffing,
    /// Client is navigating to the camp location.
    NavigatingToCamp,
    /// Client is fully set up and ready for camp loop.
    Ready,
}

/// A single managed EQ client session.
pub struct EqSession {
    /// Unique identifier for this client slot.
    pub client_id: ClientId,
    /// OS process ID of the EQ client.
    pub pid: u32,
    /// In-game character name (populated after first state update).
    pub character_name: Option<String>,
    /// Current DLL injection and hook status.
    pub hook_status: HookStatus,
    /// Path to the injected DLL, if any.
    pub dll_path: Option<PathBuf>,
    /// Self-healing monitor for crash detection and recovery.
    pub health_monitor: HealthMonitor,
    /// Most recent game state snapshot from shared memory.
    pub last_state: Option<GameState>,
    /// Account name used to log in.
    pub account_name: Option<String>,
    /// Account info from the launch config, for cross-checking.
    pub bound_toon: Option<dmft_common::login::AccountInfo>,
    /// Current phase of post-login setup.
    pub post_login_phase: PostLoginPhase,
}

impl EqSession {
    /// Creates a new session for the given client and process.
    #[must_use]
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
    #[must_use]
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
            if let Some(ref bound) = self.bound_toon
                && player.displayed_name != bound.character_name
            {
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
