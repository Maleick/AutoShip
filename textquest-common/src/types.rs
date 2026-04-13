/// Unique ID for each managed EQ client
pub type ClientId = u32;

/// Full game state snapshot sent from the DLL to the manager
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    /// PID of the EQ client this state belongs to.
    pub client_id: ClientId,
    /// The local player's spawn data, if in-game.
    pub local_player: Option<SpawnData>,
    /// Current target's spawn data, if any.
    pub target: Option<SpawnData>,
    /// All spawns within render distance.
    pub nearby_spawns: Vec<SpawnData>,
    /// Millisecond timestamp when this snapshot was captured.
    pub timestamp_ms: u64,
    /// Current navigation FSM state.
    pub nav_status: crate::nav::NavStatus,
    /// Current combat FSM state.
    pub combat_status: crate::combat::CombatStatus,
    /// Zone short name (e.g. "qey2hh1").
    pub zone_short_name: String,
    /// Zone long name (e.g. "Queynos Hills").
    pub zone_long_name: String,
    /// Detected EQ patch date from `__ActualVersionDate` if available.
    #[serde(default)]
    pub actual_version: Option<String>,
}

impl GameState {
    /// Convert the public `GameState` into the internal shared-memory frame.
    #[must_use]
    pub fn to_shared_frame(&self, spawn_epoch: u64, include_spawns: bool) -> SharedStateFrame {
        SharedStateFrame {
            client_id: self.client_id,
            local_player: self.local_player.clone(),
            target: self.target.clone(),
            nearby_spawns: include_spawns.then(|| self.nearby_spawns.clone()),
            timestamp_ms: self.timestamp_ms,
            nav_status: self.nav_status.clone(),
            combat_status: self.combat_status,
            zone_short_name: self.zone_short_name.clone(),
            zone_long_name: self.zone_long_name.clone(),
            spawn_epoch,
            actual_version: self.actual_version.clone(),
        }
    }
}

/// Internal shared-memory payload written by the DLL and reconstructed by the reader.
///
/// This is intentionally separate from `GameState` so spawn data can be omitted on
/// non-refresh ticks without changing the public snapshot shape consumed elsewhere.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SharedStateFrame {
    /// PID of the EQ client this state belongs to.
    pub client_id: ClientId,
    /// The local player's spawn data, if in-game.
    pub local_player: Option<SpawnData>,
    /// Current target's spawn data, if any.
    pub target: Option<SpawnData>,
    /// Nearby spawn snapshot when it changed on this tick.
    pub nearby_spawns: Option<Vec<SpawnData>>,
    /// Millisecond timestamp when this snapshot was captured.
    pub timestamp_ms: u64,
    /// Current navigation FSM state.
    pub nav_status: crate::nav::NavStatus,
    /// Current combat FSM state.
    pub combat_status: crate::combat::CombatStatus,
    /// Zone short name (e.g. "qey2hh1").
    pub zone_short_name: String,
    /// Zone long name (e.g. "Queynos Hills").
    pub zone_long_name: String,
    /// Monotonic spawn snapshot version. Increments only when `nearby_spawns` is present.
    pub spawn_epoch: u64,
    /// Detected EQ patch date from `__ActualVersionDate` if available.
    #[serde(default)]
    pub actual_version: Option<String>,
}

impl SharedStateFrame {
    /// Reconstruct a full `GameState` by applying cached spawns when this frame omitted them.
    #[must_use]
    pub fn into_game_state(self, cached_spawns: Vec<SpawnData>) -> GameState {
        GameState {
            client_id: self.client_id,
            local_player: self.local_player,
            target: self.target,
            nearby_spawns: self.nearby_spawns.unwrap_or(cached_spawns),
            timestamp_ms: self.timestamp_ms,
            nav_status: self.nav_status,
            combat_status: self.combat_status,
            zone_short_name: self.zone_short_name,
            zone_long_name: self.zone_long_name,
            actual_version: self.actual_version,
        }
    }
}

/// Serializable representation of an EQ spawn (player, NPC, corpse, etc.)
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpawnData {
    /// Unique spawn ID assigned by the EQ server.
    pub spawn_id: u32,
    /// Internal name (e.g. "a_fire_beetle").
    pub name: String,
    /// Name shown in-game (e.g. "a fire beetle").
    pub displayed_name: String,
    /// Spawn type: 0=player, 1=NPC, 2=corpse, 3=any.
    pub spawn_type: u8,
    /// Character or mob level.
    pub level: u8,
    /// EQ class ID (1=Warrior, 2=Cleric, etc.).
    pub class_id: u8,
    /// World X position.
    pub x: f32,
    /// World Y position.
    pub y: f32,
    /// World Z position (vertical).
    pub z: f32,
    /// Facing direction in degrees (0-512 EQ heading units).
    pub heading: f32,
    /// Current hit points.
    pub hp_current: i64,
    /// Maximum hit points.
    pub hp_max: i64,
    /// Current mana.
    pub mana_current: i32,
    /// Maximum mana.
    pub mana_max: i32,
    /// Signed because EQ can drain endurance below zero internally.
    pub endurance_current: i32,
    /// Unsigned in the EQ struct (`PlayerZoneClient`). Do not compare directly
    /// with `endurance_current` without casting — signedness differs intentionally.
    pub endurance_max: u32,
    /// Current movement speed (`SpeedRun`). Non-zero means the character is in motion.
    pub speed_run: f32,
    /// Stand state: 0=standing, 1=frozen, 2=looting, 3=sitting, 4=ducking,
    /// 110=feigned, 111=dead. Only 0 (standing) allows spell casting.
    pub stand_state: u8,
    /// Whether this spawn is flagged as a GM (Game Master).
    pub is_gm: bool,
}

impl SpawnData {
    /// Returns current HP as a percentage (0.0 - 100.0). Returns 100.0 if max HP is zero or negative.
    #[must_use]
    pub fn hp_pct(&self) -> f32 {
        if self.hp_max > 0 {
            (self.hp_current as f32 / self.hp_max as f32) * 100.0
        } else {
            100.0
        }
    }

    /// Returns current mana as a percentage (0.0 - 100.0). Returns 100.0 if max mana is zero or negative.
    #[must_use]
    pub fn mana_pct(&self) -> f32 {
        if self.mana_max > 0 {
            (self.mana_current as f32 / self.mana_max as f32) * 100.0
        } else {
            100.0
        }
    }

    /// Returns `true` when the character is in motion (speed is non-negligible).
    ///
    /// EQ sets `SpeedRun` to a non-zero value while the character is moving.
    /// A small epsilon avoids false positives from floating-point noise.
    #[must_use]
    pub fn is_moving(&self) -> bool {
        self.speed_run.abs() > 0.01
    }

    /// Returns `true` when the character is standing and eligible to cast spells.
    ///
    /// Stand state 0 is the only state from which a spell cast can be initiated.
    /// Sitting (3), ducking (4), feigning death (110), and dead (111) all prevent casting.
    #[must_use]
    pub fn is_standing(&self) -> bool {
        self.stand_state == 0
    }
}

/// Operator-visible lifecycle state for a single managed session slot.
///
/// This is a display-oriented summary derived from the launcher `LoginPhase`,
/// hook status, and self-healing monitor state.  It gives operators a stable,
/// named vocabulary for what each slot is doing without exposing internal FSM
/// details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotLifecycle {
    /// Slot is defined in config but no launch has been initiated.
    Configured,
    /// EQ process is being spawned (pre-login screen).
    Launching,
    /// Process is running and working through the login / server / character
    /// select screens.
    WaitingForLogin,
    /// Character is zoning in or running post-login setup (buffs, group join).
    EnteringWorld,
    /// Slot is fully attached, hooks active, and ready for orchestration.
    Live,
    /// Slot experienced a crash or timeout and is being restarted.
    Recovering,
    /// Slot failed in a way that prevents automatic recovery; needs operator
    /// intervention.
    Blocked,
    /// Client is executing /camp or /quit and waiting to leave the world.
    CampingOut,
    /// Client process has exited (cleanly or via crash).
    Exited,
    /// Client is being restarted by the launcher after an exit or crash.
    Relaunching,
}

impl SlotLifecycle {
    /// Short operator-facing label (fits in ≤ 8 chars for compact display).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Configured => "CFG",
            Self::Launching => "LAUNCH",
            Self::WaitingForLogin => "LOGIN",
            Self::EnteringWorld => "ZONE",
            Self::Live => "LIVE",
            Self::Recovering => "RECOV",
            Self::Blocked => "BLOCK",
            Self::CampingOut => "CAMP",
            Self::Exited => "EXIT",
            Self::Relaunching => "RELAUNCH",
        }
    }

    /// Long operator-facing label for sidebar panels.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::Configured => "Configured",
            Self::Launching => "Launching",
            Self::WaitingForLogin => "Waiting for login",
            Self::EnteringWorld => "Entering world",
            Self::Live => "Live",
            Self::Recovering => "Recovering",
            Self::Blocked => "Blocked",
            Self::CampingOut => "Camping out",
            Self::Exited => "Exited",
            Self::Relaunching => "Relaunching",
        }
    }

    /// Whether this lifecycle state represents a healthy, operational slot.
    #[must_use]
    pub fn is_healthy(self) -> bool {
        matches!(self, Self::Live)
    }

    /// Whether this lifecycle state represents a degraded or blocked slot.
    #[must_use]
    pub fn is_degraded(self) -> bool {
        matches!(
            self,
            Self::Recovering | Self::Blocked | Self::CampingOut | Self::Exited
        )
    }

    /// Whether this lifecycle state means the process is no longer running.
    #[must_use]
    pub fn is_offline(self) -> bool {
        matches!(self, Self::Exited | Self::Relaunching | Self::Configured)
    }

    /// Whether the slot is in a transitional shutdown/restart cycle.
    #[must_use]
    pub fn is_cycling(self) -> bool {
        matches!(self, Self::CampingOut | Self::Relaunching)
    }
}

/// Status of the in-process hook inside an EQ client
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HookStatus {
    /// DLL has not been injected into this client.
    NotInjected,
    /// DLL injection is in progress.
    Injecting,
    /// DLL is loaded but hooks are not yet active.
    Injected,
    /// DLL hooks are active and processing game events.
    HooksActive,
    /// An error occurred during injection or hook setup.
    Error(String),
    /// DLL is being ejected from the process.
    Ejecting,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spawn(hp_current: i64, hp_max: i64, mana_current: i32, mana_max: i32) -> SpawnData {
        SpawnData {
            spawn_id: 1,
            name: "TestSpawn".to_string(),
            displayed_name: "Test Spawn".to_string(),
            spawn_type: 0,
            level: 60,
            class_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current,
            hp_max,
            mana_current,
            mana_max,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
        }
    }

    #[test]
    fn hp_pct_returns_100_when_hp_max_is_zero() {
        let spawn = make_spawn(0, 0, 0, 0);
        assert!((spawn.hp_pct() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hp_pct_returns_correct_percentage() {
        let spawn = make_spawn(750, 1000, 0, 0);
        assert!((spawn.hp_pct() - 75.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hp_pct_full_health() {
        let spawn = make_spawn(5000, 5000, 0, 0);
        assert!((spawn.hp_pct() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mana_pct_returns_100_when_mana_max_is_zero() {
        let spawn = make_spawn(100, 100, 0, 0);
        assert!((spawn.mana_pct() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mana_pct_returns_correct_percentage() {
        let spawn = make_spawn(100, 100, 200, 800);
        assert!((spawn.mana_pct() - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mana_pct_full_mana() {
        let spawn = make_spawn(100, 100, 3000, 3000);
        assert!((spawn.mana_pct() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hp_pct_negative_hp_max_returns_100() {
        // hp_max <= 0 should fall through to the 100.0 default
        let spawn = make_spawn(50, -10, 0, 0);
        assert!((spawn.hp_pct() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hp_pct_zero_hp_with_positive_max() {
        let spawn = make_spawn(0, 1000, 0, 0);
        assert!((spawn.hp_pct()).abs() < f32::EPSILON);
    }

    #[test]
    fn mana_pct_negative_mana_max_returns_100() {
        let spawn = make_spawn(100, 100, 50, -10);
        assert!((spawn.mana_pct() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mana_pct_zero_mana_with_positive_max() {
        let spawn = make_spawn(100, 100, 0, 1000);
        assert!((spawn.mana_pct()).abs() < f32::EPSILON);
    }

    #[test]
    fn spawn_data_default() {
        let spawn = SpawnData::default();
        assert_eq!(spawn.spawn_id, 0);
        assert_eq!(spawn.name, "");
        assert_eq!(spawn.level, 0);
        assert!((spawn.x).abs() < f32::EPSILON);
        assert_eq!(spawn.hp_current, 0);
        assert_eq!(spawn.hp_max, 0);
    }

    #[test]
    fn hook_status_all_variants() {
        let variants = vec![
            HookStatus::NotInjected,
            HookStatus::Injecting,
            HookStatus::Injected,
            HookStatus::HooksActive,
            HookStatus::Error("test error".into()),
            HookStatus::Ejecting,
        ];
        assert_eq!(variants.len(), 6);
        // Verify debug output works
        for v in &variants {
            let _ = format!("{:?}", v);
        }
    }

    #[test]
    fn hook_status_error_carries_message() {
        let status = HookStatus::Error("something broke".into());
        if let HookStatus::Error(msg) = status {
            assert_eq!(msg, "something broke");
        } else {
            panic!("expected Error");
        }
    }

    #[test]
    fn hook_status_serialization_roundtrip() {
        let statuses = vec![
            HookStatus::NotInjected,
            HookStatus::HooksActive,
            HookStatus::Error("test".into()),
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).expect("serialize");
            let restored: HookStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*s, restored);
        }
    }

    #[test]
    fn game_state_serialization_roundtrip() {
        let gs = GameState {
            client_id: 42,
            local_player: Some(make_spawn(1000, 1000, 500, 500)),
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 12345,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "qey2hh1".into(),
            zone_long_name: "Queynos Hills".into(),
            actual_version: None,
        };
        let json = serde_json::to_string(&gs).expect("serialize");
        let restored: GameState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, gs);
    }

    #[test]
    fn shared_state_frame_includes_spawns_when_requested() {
        let gs = GameState {
            client_id: 7,
            local_player: Some(make_spawn(1000, 1000, 500, 500)),
            target: None,
            nearby_spawns: vec![make_spawn(10, 10, 0, 0), make_spawn(20, 20, 0, 0)],
            timestamp_ms: 55,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "soldunga".into(),
            zone_long_name: "Solusek's Eye".into(),
            actual_version: None,
        };

        let frame = gs.to_shared_frame(3, true);

        assert_eq!(frame.spawn_epoch, 3);
        assert_eq!(frame.nearby_spawns.as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn shared_state_frame_omits_spawns_when_not_requested() {
        let gs = GameState {
            client_id: 7,
            local_player: Some(make_spawn(1000, 1000, 500, 500)),
            target: None,
            nearby_spawns: vec![make_spawn(10, 10, 0, 0)],
            timestamp_ms: 55,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "soldunga".into(),
            zone_long_name: "Solusek's Eye".into(),
            actual_version: None,
        };

        let frame = gs.to_shared_frame(4, false);

        assert_eq!(frame.spawn_epoch, 4);
        assert!(frame.nearby_spawns.is_none());
    }

    #[test]
    fn shared_state_frame_reconstructs_game_state_with_cached_spawns() {
        let cached_spawns = vec![make_spawn(10, 10, 0, 0), make_spawn(20, 20, 0, 0)];
        let frame = SharedStateFrame {
            client_id: 11,
            local_player: Some(make_spawn(1000, 1000, 500, 500)),
            target: Some(make_spawn(200, 400, 0, 0)),
            nearby_spawns: None,
            timestamp_ms: 999,
            nav_status: crate::nav::NavStatus::Arrived,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: "qcat".into(),
            zone_long_name: "Qeynos Catacombs".into(),
            spawn_epoch: 9,
            actual_version: None,
        };

        let state = frame.into_game_state(cached_spawns.clone());

        assert_eq!(state.nearby_spawns, cached_spawns);
        assert_eq!(state.zone_short_name, "qcat");
        assert_eq!(state.timestamp_ms, 999);
    }

    #[test]
    fn spawn_data_endurance_fields() {
        let spawn = SpawnData {
            endurance_current: -50, // Can be negative
            endurance_max: 100,
            ..SpawnData::default()
        };
        assert_eq!(spawn.endurance_current, -50);
        assert_eq!(spawn.endurance_max, 100);
    }

    #[test]
    fn hp_pct_over_100_when_buffed() {
        // Some EQ buffs can push HP above max
        let spawn = make_spawn(1200, 1000, 0, 0);
        assert!(spawn.hp_pct() > 100.0);
    }

    #[test]
    fn mana_pct_over_100_when_buffed() {
        let spawn = make_spawn(100, 100, 1500, 1000);
        assert!(spawn.mana_pct() > 100.0);
    }

    #[test]
    fn hp_pct_negative_hp_current() {
        let spawn = make_spawn(-100, 1000, 0, 0);
        assert!(spawn.hp_pct() < 0.0);
    }

    #[test]
    fn spawn_data_equality() {
        let a = make_spawn(100, 200, 50, 100);
        let b = make_spawn(100, 200, 50, 100);
        assert_eq!(a, b);
    }

    #[test]
    fn spawn_data_clone() {
        let original = make_spawn(500, 1000, 200, 400);
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn slot_lifecycle_labels() {
        assert_eq!(SlotLifecycle::Configured.label(), "CFG");
        assert_eq!(SlotLifecycle::Launching.label(), "LAUNCH");
        assert_eq!(SlotLifecycle::WaitingForLogin.label(), "LOGIN");
        assert_eq!(SlotLifecycle::EnteringWorld.label(), "ZONE");
        assert_eq!(SlotLifecycle::Live.label(), "LIVE");
        assert_eq!(SlotLifecycle::Recovering.label(), "RECOV");
        assert_eq!(SlotLifecycle::Blocked.label(), "BLOCK");
        assert_eq!(SlotLifecycle::CampingOut.label(), "CAMP");
        assert_eq!(SlotLifecycle::Exited.label(), "EXIT");
        assert_eq!(SlotLifecycle::Relaunching.label(), "RELAUNCH");
    }

    #[test]
    fn slot_lifecycle_descriptions_non_empty() {
        let variants = [
            SlotLifecycle::Configured,
            SlotLifecycle::Launching,
            SlotLifecycle::WaitingForLogin,
            SlotLifecycle::EnteringWorld,
            SlotLifecycle::Live,
            SlotLifecycle::Recovering,
            SlotLifecycle::Blocked,
            SlotLifecycle::CampingOut,
            SlotLifecycle::Exited,
            SlotLifecycle::Relaunching,
        ];
        for v in variants {
            assert!(!v.description().is_empty());
        }
    }

    #[test]
    fn slot_lifecycle_healthy_only_live() {
        assert!(SlotLifecycle::Live.is_healthy());
        assert!(!SlotLifecycle::Configured.is_healthy());
        assert!(!SlotLifecycle::Recovering.is_healthy());
        assert!(!SlotLifecycle::Blocked.is_healthy());
        assert!(!SlotLifecycle::CampingOut.is_healthy());
        assert!(!SlotLifecycle::Exited.is_healthy());
        assert!(!SlotLifecycle::Relaunching.is_healthy());
    }

    #[test]
    fn slot_lifecycle_degraded_states() {
        assert!(SlotLifecycle::Recovering.is_degraded());
        assert!(SlotLifecycle::Blocked.is_degraded());
        assert!(SlotLifecycle::CampingOut.is_degraded());
        assert!(SlotLifecycle::Exited.is_degraded());
        assert!(!SlotLifecycle::Live.is_degraded());
        assert!(!SlotLifecycle::Configured.is_degraded());
        assert!(!SlotLifecycle::Relaunching.is_degraded());
    }

    #[test]
    fn slot_lifecycle_offline_states() {
        assert!(SlotLifecycle::Exited.is_offline());
        assert!(SlotLifecycle::Relaunching.is_offline());
        assert!(SlotLifecycle::Configured.is_offline());
        assert!(!SlotLifecycle::Live.is_offline());
        assert!(!SlotLifecycle::CampingOut.is_offline());
        assert!(!SlotLifecycle::Launching.is_offline());
    }

    #[test]
    fn slot_lifecycle_cycling_states() {
        assert!(SlotLifecycle::CampingOut.is_cycling());
        assert!(SlotLifecycle::Relaunching.is_cycling());
        assert!(!SlotLifecycle::Live.is_cycling());
        assert!(!SlotLifecycle::Exited.is_cycling());
        assert!(!SlotLifecycle::Blocked.is_cycling());
    }

    #[test]
    fn hook_status_equality() {
        assert_eq!(HookStatus::NotInjected, HookStatus::NotInjected);
        assert_ne!(HookStatus::NotInjected, HookStatus::Injected);
        assert_ne!(HookStatus::Error("a".into()), HookStatus::Error("b".into()));
    }

    #[test]
    fn game_state_with_spawns() {
        let gs = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: vec![make_spawn(100, 100, 0, 0), make_spawn(200, 200, 0, 0)],
            timestamp_ms: 0,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: String::new(),
            zone_long_name: String::new(),
            actual_version: None,
        };
        assert_eq!(gs.nearby_spawns.len(), 2);
        assert_eq!(gs.nearby_spawns[0].hp_current, 100);
    }

    #[test]
    fn spawn_data_heading_preserved() {
        let spawn = SpawnData {
            heading: 256.0,
            ..SpawnData::default()
        };
        assert!((spawn.heading - 256.0).abs() < f32::EPSILON);
    }

    #[test]
    fn spawn_data_displayed_name_differs_from_name() {
        let spawn = SpawnData {
            name: "a_moss_snake".into(),
            displayed_name: "a moss snake".into(),
            ..SpawnData::default()
        };
        assert_ne!(spawn.name, spawn.displayed_name);
    }

    #[test]
    fn is_moving_false_when_stationary() {
        let spawn = SpawnData {
            speed_run: 0.0,
            ..SpawnData::default()
        };
        assert!(!spawn.is_moving());
    }

    #[test]
    fn is_moving_false_below_epsilon() {
        let spawn = SpawnData {
            speed_run: 0.005,
            ..SpawnData::default()
        };
        assert!(
            !spawn.is_moving(),
            "tiny speed below epsilon should not count as moving"
        );
    }

    #[test]
    fn is_moving_true_when_running() {
        let spawn = SpawnData {
            speed_run: 1.4,
            ..SpawnData::default()
        };
        assert!(spawn.is_moving());
    }

    #[test]
    fn is_moving_true_for_negative_speed() {
        let spawn = SpawnData {
            speed_run: -0.5,
            ..SpawnData::default()
        };
        assert!(
            spawn.is_moving(),
            "negative speed (backing up) counts as moving"
        );
    }

    #[test]
    fn is_standing_true_for_state_zero() {
        let spawn = SpawnData {
            stand_state: 0,
            ..SpawnData::default()
        };
        assert!(spawn.is_standing());
    }

    #[test]
    fn is_standing_false_for_sitting() {
        let spawn = SpawnData {
            stand_state: 3,
            ..SpawnData::default()
        };
        assert!(!spawn.is_standing());
    }

    #[test]
    fn is_standing_false_for_ducking() {
        let spawn = SpawnData {
            stand_state: 4,
            ..SpawnData::default()
        };
        assert!(!spawn.is_standing());
    }

    #[test]
    fn is_standing_false_for_feigning_death() {
        let spawn = SpawnData {
            stand_state: 110,
            ..SpawnData::default()
        };
        assert!(!spawn.is_standing());
    }

    #[test]
    fn spawn_data_default_is_stationary_and_standing() {
        let spawn = SpawnData::default();
        assert!(!spawn.is_moving(), "default spawn should be stationary");
        assert!(spawn.is_standing(), "default spawn should be standing");
    }
}
