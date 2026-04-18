//! TextQuest API models

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub client_id: u32,
    pub character_name: String,
    pub zone: String,
    pub level: u8,
    #[serde(rename = "hp_pct")]
    pub hp_pct: f32,
    #[serde(rename = "mana_pct")]
    pub mana_pct: f32,
    #[serde(rename = "endurance_pct")]
    pub endurance_pct: f32,
    pub status: String,
    #[serde(rename = "buff_count")]
    pub buff_count: usize,
    #[serde(rename = "target_name")]
    pub target_name: Option<String>,
    #[serde(rename = "target_hp_pct")]
    pub target_hp_pct: Option<f32>,
    #[serde(rename = "pet_name")]
    pub pet_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterConfig {
    #[serde(rename = "character_name")]
    pub character_name: String,
    pub class: String,
    pub role: String,
    #[serde(rename = "heal_at_pct")]
    pub heal_at_pct: u8,
    #[serde(rename = "mana_sit_pct")]
    pub mana_sit_pct: u8,
    #[serde(rename = "nuke_at_pct")]
    pub nuke_at_pct: u8,
    pub rotation: Vec<RotationEntry>,
    #[serde(rename = "class_params")]
    pub class_params: ClassParams,
    #[serde(rename = "group_override")]
    pub group_override: bool,
    #[serde(rename = "group_name")]
    pub group_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEntry {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClassParams {
    #[serde(rename = "ch_chain_timing_ms")]
    pub ch_chain_timing_ms: Option<u32>,
    #[serde(rename = "dot_overlap_pct")]
    pub dot_overlap_pct: Option<u8>,
    #[serde(rename = "burn_at_hp_pct")]
    pub burn_at_hp_pct: Option<u8>,
    #[serde(rename = "slow_at_hp_pct")]
    pub slow_at_hp_pct: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub command: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupAssignment {
    #[serde(rename = "group_id")]
    pub group_id: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub base_url: String,
    #[serde(default)]
    pub api_token: Option<String>,
    #[serde(default)]
    pub timeout_secs: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3001".to_string(),
            api_token: None,
            timeout_secs: 30,
        }
    }
}