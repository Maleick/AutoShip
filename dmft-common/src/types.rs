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
        };
        let json = serde_json::to_string(&gs).expect("serialize");
        let restored: GameState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, gs);
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
    fn hook_status_equality() {
        assert_eq!(HookStatus::NotInjected, HookStatus::NotInjected);
        assert_ne!(HookStatus::NotInjected, HookStatus::Injected);
        assert_ne!(
            HookStatus::Error("a".into()),
            HookStatus::Error("b".into())
        );
    }

    #[test]
    fn game_state_with_spawns() {
        let gs = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: vec![
                make_spawn(100, 100, 0, 0),
                make_spawn(200, 200, 0, 0),
            ],
            timestamp_ms: 0,
            nav_status: crate::nav::NavStatus::Idle,
            combat_status: crate::combat::CombatStatus::Idle,
            zone_short_name: String::new(),
            zone_long_name: String::new(),
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
}
