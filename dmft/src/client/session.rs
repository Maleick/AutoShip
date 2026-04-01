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

#[cfg(test)]
mod tests {
    use super::*;
    use dmft_common::types::SpawnData;

    fn make_game_state(player_name: &str) -> GameState {
        GameState {
            client_id: 1,
            local_player: Some(SpawnData {
                displayed_name: player_name.to_string(),
                name: player_name.to_string(),
                ..SpawnData::default()
            }),
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: dmft_common::nav::NavStatus::Idle,
            combat_status: dmft_common::combat::CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
        }
    }

    #[test]
    fn new_session_defaults() {
        let s = EqSession::new(42, 1234);
        assert_eq!(s.client_id, 42);
        assert_eq!(s.pid, 1234);
        assert!(s.character_name.is_none());
        assert_eq!(s.hook_status, HookStatus::NotInjected);
        assert!(s.dll_path.is_none());
        assert!(s.last_state.is_none());
        assert!(s.account_name.is_none());
        assert!(s.bound_toon.is_none());
        assert!(matches!(s.post_login_phase, PostLoginPhase::NotStarted));
    }

    #[test]
    fn is_active_requires_hooks_active_and_healthy() {
        let mut s = EqSession::new(1, 100);
        // Not active by default (NotInjected)
        assert!(!s.is_active());

        // Still not active with just hooks
        s.hook_status = HookStatus::HooksActive;
        assert!(s.is_active()); // healthy by default

        // Not active if hook status is wrong
        s.hook_status = HookStatus::Injected;
        assert!(!s.is_active());
    }

    #[test]
    fn update_state_sets_character_name_from_first_player() {
        let mut s = EqSession::new(1, 100);
        let state = make_game_state("Frostreaver");
        s.update_state(state);
        assert_eq!(s.character_name.as_deref(), Some("Frostreaver"));
    }

    #[test]
    fn update_state_does_not_overwrite_character_name() {
        let mut s = EqSession::new(1, 100);
        s.update_state(make_game_state("FirstName"));
        s.update_state(make_game_state("SecondName"));
        // Should keep first name
        assert_eq!(s.character_name.as_deref(), Some("FirstName"));
    }

    #[test]
    fn update_state_stores_last_state() {
        let mut s = EqSession::new(1, 100);
        let state = make_game_state("Test");
        s.update_state(state.clone());
        assert!(s.last_state.is_some());
        assert_eq!(s.last_state.as_ref().unwrap().client_id, 1);
    }

    #[test]
    fn update_state_without_player_keeps_name_none() {
        let mut s = EqSession::new(1, 100);
        let state = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: dmft_common::nav::NavStatus::Idle,
            combat_status: dmft_common::combat::CombatStatus::Idle,
            zone_short_name: String::new(),
            zone_long_name: String::new(),
        };
        s.update_state(state);
        assert!(s.character_name.is_none());
    }

    #[test]
    fn post_login_phase_debug_format() {
        let phases = [
            PostLoginPhase::NotStarted,
            PostLoginPhase::JoiningGroup,
            PostLoginPhase::Buffing,
            PostLoginPhase::NavigatingToCamp,
            PostLoginPhase::Ready,
        ];
        for p in &phases {
            let _ = format!("{:?}", p);
        }
    }

    #[test]
    fn update_state_replaces_last_state() {
        let mut s = EqSession::new(1, 100);
        let state1 = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 100,
            nav_status: dmft_common::nav::NavStatus::Idle,
            combat_status: dmft_common::combat::CombatStatus::Idle,
            zone_short_name: "zone1".into(),
            zone_long_name: "Zone One".into(),
        };
        let state2 = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 200,
            nav_status: dmft_common::nav::NavStatus::Idle,
            combat_status: dmft_common::combat::CombatStatus::Idle,
            zone_short_name: "zone2".into(),
            zone_long_name: "Zone Two".into(),
        };
        s.update_state(state1);
        assert_eq!(s.last_state.as_ref().unwrap().timestamp_ms, 100);
        s.update_state(state2);
        assert_eq!(s.last_state.as_ref().unwrap().timestamp_ms, 200);
    }

    #[test]
    fn is_active_false_when_not_injected() {
        let s = EqSession::new(1, 100);
        assert_eq!(s.hook_status, HookStatus::NotInjected);
        assert!(!s.is_active());
    }

    #[test]
    fn post_login_phase_clone() {
        let p = PostLoginPhase::Buffing;
        let c = p.clone();
        assert_eq!(p, c);
    }

    #[test]
    fn session_fields_mutable() {
        let mut s = EqSession::new(1, 100);
        s.account_name = Some("test_account".into());
        assert_eq!(s.account_name.as_deref(), Some("test_account"));
        s.hook_status = HookStatus::HooksActive;
        assert!(s.is_active());
    }
}
