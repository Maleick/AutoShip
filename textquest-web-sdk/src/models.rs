use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Error response from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Session information returned by GET /api/sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub client_id: u32,
    pub character_name: String,
    pub zone: String,
    pub level: u8,
    pub hp_pct: f32,
    pub mana_pct: f32,
    pub endurance_pct: f32,
    pub status: String,
    pub buff_count: usize,
    pub target_name: Option<String>,
    pub target_hp_pct: Option<f32>,
    pub pet_name: Option<String>,
}

/// Chat log rotation strategy payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatLogRotationPayload {
    Daily { daily: Option<()> },
    Size { size: u64 },
    None(String),
}

/// Chat log settings returned by GET /api/chat-log/settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatLogSettings {
    pub enabled: bool,
    pub channels: Vec<String>,
    pub rotation_strategy: ChatLogRotationPayload,
    pub max_file_size_bytes: u64,
    pub min_level: String,
    pub log_eq_chat: bool,
}

/// Box chat configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxChatConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub auto_connect: bool,
}

/// Single rotation entry in a character's combat rotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEntry {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub enabled: bool,
}

/// Class-specific tuning parameters
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClassParams {
    pub ch_chain_timing_ms: Option<u32>,
    pub dot_overlap_pct: Option<u8>,
    pub burn_at_hp_pct: Option<u8>,
    pub slow_at_hp_pct: Option<u8>,
}

/// Character configuration returned by GET /api/config/characters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterConfig {
    pub character_name: String,
    pub class: String,
    pub role: String,
    pub heal_at_pct: u8,
    pub mana_sit_pct: u8,
    pub nuke_at_pct: u8,
    pub rotation: Vec<RotationEntry>,
    pub class_params: ClassParams,
    pub group_override: bool,
    pub group_name: Option<String>,
}

/// Trust policy for incoming auto-accept requests
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutoAcceptTrustMode {
    /// Accept from anyone
    #[default]
    Anyone,
    /// Require the sender to be in the trusted_players list
    TrustList,
}

/// Player filter mode for zone-in/out monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlayerFilterMode {
    #[default]
    All,
    StrangersOnly,
    FriendsOnly,
}

/// Player watch configuration returned by GET /api/config/player-watch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerWatchConfig {
    pub filter_mode: PlayerFilterMode,
    pub sound_on_zone_in: bool,
    pub friends: Vec<String>,
}

/// Timestamp format variants
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    #[default]
    DateTime24,
    Time24,
    DateTime12,
    Time12,
}

/// Per-character timestamp display configuration (mirrors server TimestampConfig).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampConfig {
    pub enabled: bool,
    pub format: TimestampFormat,
}

impl Default for TimestampConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: TimestampFormat::DateTime24,
        }
    }
}

/// Krono-related economy settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KronoSettings {
    pub target_rate_per_day: u32,
    pub min_sell_price: u32,
    pub max_buy_price: u32,
    pub restock_threshold: u32,
    pub enabled: bool,
}

/// Vendor route definition matching the server contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorRoute {
    pub id: String,
    pub zone: String,
    pub npc_name: String,
    pub path_notes: String,
    pub item_categories: Vec<String>,
    pub enabled: bool,
}

/// Banking rule for automation matching the server contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankingRule {
    pub id: String,
    pub item_category: String,
    pub deposit_threshold: u32,
    pub keep_on_hand: u32,
    pub auto_deposit: bool,
}

/// Tradeskill supply definition matching the server contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeskillSupply {
    pub id: String,
    pub skill: String,
    pub materials: Vec<String>,
    pub restock_quantity: u32,
    pub source_zone: String,
    pub enabled: bool,
}

/// Wealth snapshot at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WealthSnapshot {
    pub timestamp: String,
    pub plat: u64,
    pub krono: u32,
    pub item_value_estimate: u64,
}

/// Historical wealth data returned by GET /api/economy/wealth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WealthHistory {
    pub current: WealthSnapshot,
    pub snapshots: Vec<WealthSnapshot>,
}

/// Economy settings and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomySettings {
    pub krono: KronoSettings,
    pub banking_rules: Vec<BankingRule>,
    pub tradeskill_supplies: Vec<TradeskillSupply>,
}

/// Loot rules configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootRules {
    pub rules: Vec<LootRule>,
    pub master_looter: String,
    pub auto_loot_enabled: bool,
}

/// Individual loot rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootRule {
    pub id: String,
    pub item_pattern: String,
    pub action: LootAction,
    pub priority: u32,
}

/// Loot action types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LootAction {
    Keep,
    Bank,
    Vendor,
    Ignore,
}

/// Loot distribution settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootDistribution {
    pub distribution_method: String,
    pub priority_classes: Vec<String>,
    pub need_before_greed: bool,
}

/// Loot filter per character
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootFilter {
    pub character: String,
    pub enabled: bool,
    pub filters: Vec<String>,
}

/// Master looter assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterLooter {
    pub character: String,
    pub enabled: bool,
}

/// Loot history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootHistoryEntry {
    pub timestamp: u64,
    pub character: String,
    pub item_name: String,
    pub quantity: u32,
    pub action_taken: String,
}

/// Soul state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulState {
    pub character_id: String,
    pub memory_usage: u64,
    pub processed_events: u64,
    pub last_action_time: u64,
    pub status: String,
}

/// Raid configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaidConfig {
    pub raid_name: String,
    pub members: Vec<String>,
    pub formation: String,
    pub targets: Vec<String>,
}

/// Spawn alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAlertConfig {
    pub enabled: bool,
    pub watch_patterns: Vec<String>,
    pub alert_method: String,
    pub cooldown_seconds: u32,
}

/// Spawn alert entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAlert {
    pub id: String,
    pub pattern: String,
    pub location: String,
    pub timestamp: u64,
    pub level: u32,
}

/// GM alert state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmAlert {
    pub zone: String,
    pub detected: bool,
    pub last_seen_timestamp: u64,
}

/// Say detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SayDetectionConfig {
    pub enabled: bool,
    pub patterns: Vec<String>,
    pub alert_on_match: bool,
}

/// Say detection match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SayMatch {
    pub pattern: String,
    pub speaker: String,
    pub message: String,
    pub timestamp: u64,
}

/// XAssist configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XAssistConfig {
    pub character: String,
    pub enabled: bool,
    pub assist_target: String,
    pub auto_attack: bool,
}

/// Chat pattern rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPatternRule {
    pub id: String,
    pub pattern: String,
    pub action: String,
    pub enabled: bool,
    pub cooldown_seconds: u32,
}

/// Chat pattern rule stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPatternRuleStats {
    pub total_rules: u32,
    pub enabled_rules: u32,
    pub total_matches: u64,
    pub total_fires: u64,
}

/// Discord routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: String,
    pub enabled: bool,
    pub channels: HashMap<String, String>,
}

/// Kill tracker entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillTrackerEntry {
    pub timestamp: u64,
    pub character: String,
    pub target: String,
    pub zone: String,
    pub level: u32,
}

/// Kill tracker stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillTrackerStats {
    pub total_kills: u64,
    pub unique_targets: u32,
    pub session_duration_seconds: u64,
}

/// Alert store entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEntry {
    pub id: String,
    pub alert_type: String,
    pub message: String,
    pub timestamp: u64,
    pub acknowledged: bool,
}

/// Alerting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    pub email_enabled: bool,
    pub discord_enabled: bool,
    pub log_enabled: bool,
}

/// Account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub email: Option<String>,
    pub created_at: u64,
    pub last_login: u64,
}

/// Credentials for account operations
#[derive(Debug, Serialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}
