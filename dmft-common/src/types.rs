/// Unique ID for each managed EQ client
pub type ClientId = u32;

/// Full game state snapshot sent from the DLL to the manager
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    pub client_id: ClientId,
    pub local_player: Option<SpawnData>,
    pub target: Option<SpawnData>,
    pub nearby_spawns: Vec<SpawnData>,
    pub timestamp_ms: u64,
    pub nav_status: crate::nav::NavStatus,
    pub combat_status: crate::combat::CombatStatus,
}

/// Serializable representation of an EQ spawn (player, NPC, corpse, etc.)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpawnData {
    pub spawn_id: u32,
    pub name: String,
    pub displayed_name: String,
    pub spawn_type: u8,
    pub level: u8,
    pub class_id: u8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
    pub hp_current: i64,
    pub hp_max: i64,
    pub mana_current: i32,
    pub mana_max: i32,
    /// Signed because EQ can drain endurance below zero internally.
    pub endurance_current: i32,
    /// Unsigned in the EQ struct (PlayerZoneClient). Do not compare directly
    /// with endurance_current without casting — signedness differs intentionally.
    pub endurance_max: u32,
}

impl SpawnData {
    pub fn hp_pct(&self) -> f32 {
        if self.hp_max > 0 {
            (self.hp_current as f32 / self.hp_max as f32) * 100.0
        } else {
            100.0
        }
    }

    pub fn mana_pct(&self) -> f32 {
        if self.mana_max > 0 {
            (self.mana_current as f32 / self.mana_max as f32) * 100.0
        } else {
            100.0
        }
    }
}

/// Status of the in-process hook inside an EQ client
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HookStatus {
    NotInjected,
    Injecting,
    Injected,
    HooksActive,
    Error(String),
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
}
