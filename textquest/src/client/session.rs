//! `EqSession` — represents a single managed EQ client.

use super::healing::{ClientHealth, HealthMonitor};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use textquest_common::types::{ClientId, GameState, HookStatus};

/// Outer lifecycle state for a managed client slot.
///
/// Tracks the full slot lifecycle from initial configuration through to live
/// operation and recovery.  This is the M8 slot-lifecycle model derived from
/// the JMB session-and-relay comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotLifecycle {
    /// Slot is configured with account/character info but the process has not
    /// been started yet.
    Configured,
    /// EQ process is being spawned by the launcher.
    Launching,
    /// EQ client is running and showing the login screen.
    WaitingForLogin,
    /// Character has passed login and is loading into the world.
    EnteringWorld,
    /// Slot is fully operational: hooks active, in-zone, ready for commands.
    Live,
    /// Slot is recovering from a crash, disconnect, or unexpected state.
    Recovering,
    /// Slot is blocked and requires operator attention before it can proceed.
    Blocked,
    /// Client is executing /camp or /quit and waiting to leave the world.
    CampingOut,
    /// Client process has exited (cleanly or via crash).
    Exited,
    /// Client is being restarted by the launcher after an exit or crash.
    Relaunching,
}

impl SlotLifecycle {
    /// Returns a short human-readable label for the lifecycle state.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Launching => "launching",
            Self::WaitingForLogin => "login",
            Self::EnteringWorld => "entering",
            Self::Live => "live",
            Self::Recovering => "recovering",
            Self::Blocked => "blocked",
            Self::CampingOut => "camping",
            Self::Exited => "exited",
            Self::Relaunching => "relaunching",
        }
    }

    /// Returns `true` when the slot is fully operational.
    #[must_use]
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Live)
    }

    /// Returns `true` when the slot needs operator attention.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked)
    }
}

impl std::fmt::Display for SlotLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Post-login setup phases after a client reaches `InWorld`.
#[derive(Debug, Clone, PartialEq)]
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

/// Default timeout for camp-out before force-killing the process.
const CAMP_OUT_TIMEOUT: Duration = Duration::from_secs(45);

/// Tracks a graceful camp-out in progress.
#[derive(Debug, Clone)]
pub struct CampOutTracker {
    /// When the camp-out command was sent.
    pub started_at: Instant,
    /// Maximum time to wait before force-killing.
    pub timeout: Duration,
}

impl CampOutTracker {
    /// Create a new tracker with the default timeout.
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            timeout: CAMP_OUT_TIMEOUT,
        }
    }

    /// Create a tracker with a custom timeout.
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            started_at: Instant::now(),
            timeout,
        }
    }

    /// Whether the timeout has expired.
    #[must_use]
    pub fn is_timed_out(&self) -> bool {
        self.started_at.elapsed() >= self.timeout
    }

    /// How long the camp-out has been running.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
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
    pub bound_toon: Option<textquest_common::login::AccountInfo>,
    /// Current phase of post-login setup.
    pub post_login_phase: PostLoginPhase,
    /// Current outer lifecycle state for this slot.
    pub slot_lifecycle: SlotLifecycle,
    /// Tracks an in-progress graceful camp-out, if any.
    pub camp_out_tracker: Option<CampOutTracker>,
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
            slot_lifecycle: SlotLifecycle::Configured,
            camp_out_tracker: None,
        }
    }

    /// Begin a graceful camp-out for this session.
    ///
    /// Sets the lifecycle to `CampingOut` and starts the timeout tracker.
    /// Returns `false` if the session is already camping out.
    pub fn begin_camp_out(&mut self) -> bool {
        if matches!(self.slot_lifecycle, SlotLifecycle::CampingOut) {
            return false;
        }
        self.slot_lifecycle = SlotLifecycle::CampingOut;
        self.camp_out_tracker = Some(CampOutTracker::new());
        tracing::info!(
            client_id = self.client_id,
            "Graceful camp-out initiated"
        );
        true
    }

    /// Check if the camp-out has timed out.
    #[must_use]
    pub fn is_camp_out_timed_out(&self) -> bool {
        self.camp_out_tracker
            .as_ref()
            .is_some_and(|t| t.is_timed_out())
    }

    /// Mark this session as exited and clear the camp-out tracker.
    pub fn mark_exited(&mut self) {
        self.slot_lifecycle = SlotLifecycle::Exited;
        self.camp_out_tracker = None;
        tracing::info!(
            client_id = self.client_id,
            "Session marked as exited"
        );
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
    use textquest_common::types::SpawnData;

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
            nav_status: textquest_common::nav::NavStatus::Idle,
            combat_status: textquest_common::combat::CombatStatus::Idle,
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
        assert!(matches!(s.slot_lifecycle, SlotLifecycle::Configured));
        assert!(s.camp_out_tracker.is_none());
    }

    #[test]
    fn is_active_requires_hooks_active_and_healthy() {
        let mut s = EqSession::new(1, 100);
        assert!(!s.is_active());
        s.hook_status = HookStatus::HooksActive;
        assert!(s.is_active());
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
        assert_eq!(s.character_name.as_deref(), Some("FirstName"));
    }

    #[test]
    fn update_state_stores_last_state() {
        let mut s = EqSession::new(1, 100);
        let state = make_game_state("Test");
        s.update_state(state);
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
            nav_status: textquest_common::nav::NavStatus::Idle,
            combat_status: textquest_common::combat::CombatStatus::Idle,
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
            nav_status: textquest_common::nav::NavStatus::Idle,
            combat_status: textquest_common::combat::CombatStatus::Idle,
            zone_short_name: "zone1".into(),
            zone_long_name: "Zone One".into(),
        };
        let state2 = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 200,
            nav_status: textquest_common::nav::NavStatus::Idle,
            combat_status: textquest_common::combat::CombatStatus::Idle,
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
        assert!(matches!(p, PostLoginPhase::Buffing));
    }

    #[test]
    fn session_fields_mutable() {
        let mut s = EqSession::new(1, 100);
        s.account_name = Some("test_account".into());
        assert_eq!(s.account_name.as_deref(), Some("test_account"));
        s.hook_status = HookStatus::HooksActive;
        assert!(s.is_active());
    }

    // ── SlotLifecycle tests ─────────────────────────────────────────────────

    #[test]
    fn slot_lifecycle_default_is_configured() {
        let s = EqSession::new(1, 100);
        assert_eq!(s.slot_lifecycle, SlotLifecycle::Configured);
    }

    #[test]
    fn slot_lifecycle_is_live() {
        let mut s = EqSession::new(1, 100);
        assert!(!s.slot_lifecycle.is_live());
        s.slot_lifecycle = SlotLifecycle::Live;
        assert!(s.slot_lifecycle.is_live());
    }

    #[test]
    fn slot_lifecycle_is_blocked() {
        let mut s = EqSession::new(1, 100);
        assert!(!s.slot_lifecycle.is_blocked());
        s.slot_lifecycle = SlotLifecycle::Blocked;
        assert!(s.slot_lifecycle.is_blocked());
    }

    #[test]
    fn slot_lifecycle_labels() {
        assert_eq!(SlotLifecycle::Configured.label(), "configured");
        assert_eq!(SlotLifecycle::Launching.label(), "launching");
        assert_eq!(SlotLifecycle::WaitingForLogin.label(), "login");
        assert_eq!(SlotLifecycle::EnteringWorld.label(), "entering");
        assert_eq!(SlotLifecycle::Live.label(), "live");
        assert_eq!(SlotLifecycle::Recovering.label(), "recovering");
        assert_eq!(SlotLifecycle::Blocked.label(), "blocked");
        assert_eq!(SlotLifecycle::CampingOut.label(), "camping");
        assert_eq!(SlotLifecycle::Exited.label(), "exited");
        assert_eq!(SlotLifecycle::Relaunching.label(), "relaunching");
    }

    #[test]
    fn slot_lifecycle_display_matches_label() {
        let lifecycle = SlotLifecycle::Recovering;
        assert_eq!(format!("{lifecycle}"), lifecycle.label());
    }

    #[test]
    fn slot_lifecycle_clone_and_eq() {
        let a = SlotLifecycle::Live;
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ── CampOutTracker tests ──────────────────────────────────────────────

    #[test]
    fn camp_out_tracker_default_timeout() {
        let tracker = CampOutTracker::new();
        assert_eq!(tracker.timeout, Duration::from_secs(45));
        assert!(!tracker.is_timed_out());
    }

    #[test]
    fn camp_out_tracker_custom_timeout() {
        let tracker = CampOutTracker::with_timeout(Duration::from_secs(10));
        assert_eq!(tracker.timeout, Duration::from_secs(10));
    }

    #[test]
    fn camp_out_tracker_instant_timeout() {
        let tracker = CampOutTracker::with_timeout(Duration::ZERO);
        assert!(tracker.is_timed_out());
    }

    #[test]
    fn camp_out_tracker_elapsed_increases() {
        let tracker = CampOutTracker::new();
        assert!(tracker.elapsed() < Duration::from_secs(1));
    }

    // ── Session camp-out integration tests ────────────────────────────────

    #[test]
    fn begin_camp_out_sets_lifecycle_and_tracker() {
        let mut s = EqSession::new(1, 100);
        s.slot_lifecycle = SlotLifecycle::Live;
        assert!(s.begin_camp_out());
        assert_eq!(s.slot_lifecycle, SlotLifecycle::CampingOut);
        assert!(s.camp_out_tracker.is_some());
    }

    #[test]
    fn begin_camp_out_returns_false_if_already_camping() {
        let mut s = EqSession::new(1, 100);
        s.slot_lifecycle = SlotLifecycle::Live;
        assert!(s.begin_camp_out());
        assert!(!s.begin_camp_out());
    }

    #[test]
    fn is_camp_out_timed_out_false_when_no_tracker() {
        let s = EqSession::new(1, 100);
        assert!(!s.is_camp_out_timed_out());
    }

    #[test]
    fn mark_exited_clears_tracker() {
        let mut s = EqSession::new(1, 100);
        s.begin_camp_out();
        s.mark_exited();
        assert_eq!(s.slot_lifecycle, SlotLifecycle::Exited);
        assert!(s.camp_out_tracker.is_none());
    }

    #[test]
    fn camp_out_tracker_none_by_default() {
        let s = EqSession::new(1, 100);
        assert!(s.camp_out_tracker.is_none());
    }
}
