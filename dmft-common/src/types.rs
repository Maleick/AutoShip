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
