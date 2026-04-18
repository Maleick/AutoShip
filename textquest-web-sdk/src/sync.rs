use crate::client::Client;
use crate::error::Result;
use crate::models::*;

/// Blocking (synchronous) TextQuest Web API client
///
/// This client wraps the async `Client` and provides a synchronous interface
/// by running operations on a tokio runtime. Use this if you need to call
/// the API from synchronous code without async/await.
///
/// # Example
///
/// ```ignore
/// use textquest_web_sdk::BlockingClient;
///
/// fn main() {
///     let client = BlockingClient::new("http://localhost:3000");
///
///     // Get health status
///     match client.health() {
///         Ok(health) => println!("Status: {}", health.status),
///         Err(e) => eprintln!("Error: {}", e),
///     }
/// }
/// ```
pub struct BlockingClient {
    client: Client,
    runtime: tokio::runtime::Runtime,
}

impl BlockingClient {
    /// Create a new blocking client
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| {
            crate::Error::Configuration(format!("Failed to create tokio runtime: {}", e))
        })?;

        Ok(Self {
            client: Client::new(base_url),
            runtime,
        })
    }

    /// Create a new blocking client with an API token
    pub fn with_token(base_url: impl Into<String>, token: Option<String>) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| {
            crate::Error::Configuration(format!("Failed to create tokio runtime: {}", e))
        })?;

        Ok(Self {
            client: Client::with_token(base_url, token),
            runtime,
        })
    }

    /// Set the API token
    pub fn set_token(&mut self, token: Option<String>) {
        self.client.set_token(token);
    }

    /// Get API health status
    pub fn health(&self) -> Result<HealthResponse> {
        self.runtime.block_on(self.client.health())
    }

    /// List all active sessions
    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        self.runtime.block_on(self.client.list_sessions())
    }

    /// Get chat log settings
    pub fn get_chat_log_settings(&self) -> Result<ChatLogSettings> {
        self.runtime.block_on(self.client.get_chat_log_settings())
    }

    /// Update chat log settings
    pub fn put_chat_log_settings(&self, settings: ChatLogSettings) -> Result<ChatLogSettings> {
        self.runtime
            .block_on(self.client.put_chat_log_settings(settings))
    }

    /// Get box chat settings
    pub fn get_box_chat_settings(&self) -> Result<BoxChatConfig> {
        self.runtime.block_on(self.client.get_box_chat_settings())
    }

    /// Update box chat settings
    pub fn put_box_chat_settings(&self, config: BoxChatConfig) -> Result<BoxChatConfig> {
        self.runtime
            .block_on(self.client.put_box_chat_settings(config))
    }

    /// List all character configurations
    pub fn list_character_configs(&self) -> Result<Vec<CharacterConfig>> {
        self.runtime.block_on(self.client.list_character_configs())
    }

    /// Get configuration for a specific character
    /// Update configuration for a character
    pub fn put_character_config(
        &self,
        character: &str,
        config: CharacterConfig,
    ) -> Result<CharacterConfig> {
        self.runtime
            .block_on(self.client.put_character_config(character, config))
    }

    /// Get auto-accept settings
    pub fn get_auto_accept_settings(&self) -> Result<AutoAcceptSettings> {
        self.runtime
            .block_on(self.client.get_auto_accept_settings())
    }

    /// Update auto-accept settings
    pub fn put_auto_accept_settings(
        &self,
        settings: AutoAcceptSettings,
    ) -> Result<AutoAcceptSettings> {
        self.runtime
            .block_on(self.client.put_auto_accept_settings(settings))
    }

    /// Get player watch configuration
    pub fn get_player_watch_config(&self) -> Result<PlayerWatchConfig> {
        self.runtime.block_on(self.client.get_player_watch_config())
    }

    /// Update player watch configuration
    pub fn put_player_watch_config(&self, config: PlayerWatchConfig) -> Result<PlayerWatchConfig> {
        self.runtime
            .block_on(self.client.put_player_watch_config(config))
    }

    /// Get economy settings
    pub fn get_economy_settings(&self) -> Result<EconomySettings> {
        self.runtime.block_on(self.client.get_economy_settings())
    }

    /// Update economy settings
    pub fn put_economy_settings(&self, settings: EconomySettings) -> Result<EconomySettings> {
        self.runtime
            .block_on(self.client.put_economy_settings(settings))
    }

    /// List all vendor routes
    pub fn list_vendor_routes(&self) -> Result<Vec<VendorRoute>> {
        self.runtime.block_on(self.client.list_vendor_routes())
    }

    /// Create a new vendor route
    pub fn create_vendor_route(&self, route: VendorRoute) -> Result<VendorRoute> {
        self.runtime
            .block_on(self.client.create_vendor_route(route))
    }

    /// Update an existing vendor route
    pub fn update_vendor_route(&self, id: &str, route: VendorRoute) -> Result<VendorRoute> {
        self.runtime
            .block_on(self.client.update_vendor_route(id, route))
    }

    /// Delete a vendor route
    pub fn delete_vendor_route(&self, id: &str) -> Result<()> {
        self.runtime.block_on(self.client.delete_vendor_route(id))
    }

    /// Get wealth summary
    pub fn get_wealth(&self) -> Result<WealthHistory> {
        self.runtime.block_on(self.client.get_wealth())
    }

    /// Get loot rules
    pub fn get_loot_rules(&self) -> Result<LootRules> {
        self.runtime.block_on(self.client.get_loot_rules())
    }

    /// Update loot rules
    pub fn put_loot_rules(&self, rules: LootRules) -> Result<LootRules> {
        self.runtime.block_on(self.client.put_loot_rules(rules))
    }

    /// Get loot filters
    pub fn get_loot_filters(&self) -> Result<Vec<LootFilter>> {
        self.runtime.block_on(self.client.get_loot_filters())
    }

    /// Update loot filter for a character
    pub fn put_loot_filter(&self, character: &str, filter: LootFilter) -> Result<LootFilter> {
        self.runtime
            .block_on(self.client.put_loot_filter(character, filter))
    }

    /// Get master looter assignment
    pub fn get_master_looter(&self) -> Result<MasterLooter> {
        self.runtime.block_on(self.client.get_master_looter())
    }

    /// Set master looter
    pub fn put_master_looter(&self, looter: MasterLooter) -> Result<MasterLooter> {
        self.runtime.block_on(self.client.put_master_looter(looter))
    }

    /// Get loot distribution settings
    pub fn get_loot_distribution(&self) -> Result<LootDistribution> {
        self.runtime.block_on(self.client.get_loot_distribution())
    }

    /// Update loot distribution settings
    pub fn put_loot_distribution(&self, dist: LootDistribution) -> Result<LootDistribution> {
        self.runtime
            .block_on(self.client.put_loot_distribution(dist))
    }

    /// Get loot history
    pub fn get_loot_history(&self) -> Result<Vec<LootHistoryEntry>> {
        self.runtime.block_on(self.client.get_loot_history())
    }

    /// List all soul states
    pub fn list_soul_states(&self) -> Result<Vec<SoulState>> {
        self.runtime.block_on(self.client.list_soul_states())
    }

    /// Get soul state for a specific character
    pub fn get_soul_state(&self, character_id: &str) -> Result<SoulState> {
        self.runtime
            .block_on(self.client.get_soul_state(character_id))
    }

    /// Get alerting configuration
    pub fn get_alerts_config(&self) -> Result<AlertingConfig> {
        self.runtime.block_on(self.client.get_alerts_config())
    }

    /// Update alerting configuration
    pub fn put_alerts_config(&self, config: AlertingConfig) -> Result<AlertingConfig> {
        self.runtime.block_on(self.client.put_alerts_config(config))
    }

    /// Get alert history
    pub fn get_alerts_history(&self) -> Result<Vec<AlertEntry>> {
        self.runtime.block_on(self.client.get_alerts_history())
    }

    /// List spawn alerts
    pub fn list_spawn_alerts(&self) -> Result<Vec<SpawnAlert>> {
        self.runtime.block_on(self.client.list_spawn_alerts())
    }

    /// Clear all spawn alerts
    pub fn clear_spawn_alerts(&self) -> Result<()> {
        self.runtime.block_on(self.client.clear_spawn_alerts())
    }

    /// Get spawn alert stats
    pub fn get_spawn_alert_stats(&self) -> Result<serde_json::Value> {
        self.runtime.block_on(self.client.get_spawn_alert_stats())
    }

    /// Get spawn alert configuration
    pub fn get_spawn_alert_config(&self) -> Result<SpawnAlertConfig> {
        self.runtime.block_on(self.client.get_spawn_alert_config())
    }

    /// Update spawn alert configuration
    pub fn put_spawn_alert_config(&self, config: SpawnAlertConfig) -> Result<SpawnAlertConfig> {
        self.runtime
            .block_on(self.client.put_spawn_alert_config(config))
    }

    /// Get spawn alert watch list
    pub fn get_spawn_watch_list(&self) -> Result<Vec<String>> {
        self.runtime.block_on(self.client.get_spawn_watch_list())
    }

    /// Add a pattern to spawn alert watch list
    pub fn put_spawn_watch_pattern(&self, pattern: &str) -> Result<()> {
        self.runtime
            .block_on(self.client.put_spawn_watch_pattern(pattern))
    }

    /// Remove a pattern from spawn alert watch list
    pub fn delete_spawn_watch_pattern(&self, pattern: &str) -> Result<()> {
        self.runtime
            .block_on(self.client.delete_spawn_watch_pattern(pattern))
    }

    /// List timestamp configurations (returns a map of character name → config)
    pub fn list_timestamp_configs(&self) -> Result<std::collections::HashMap<String, TimestampFormat>> {
        self.runtime.block_on(self.client.list_timestamp_configs())
    }

    /// Get timestamp configuration for a character
    pub fn get_timestamp_config(&self, character: &str) -> Result<TimestampFormat> {
        self.runtime.block_on(self.client.get_timestamp_config(character))
    }

    /// Update timestamp configuration for a character
    pub fn put_timestamp_config(&self, character: &str, config: TimestampFormat) -> Result<TimestampFormat> {
        self.runtime.block_on(self.client.put_timestamp_config(character, config))
    }

    /// Get kill tracker history
    pub fn get_kill_tracker_history(&self) -> Result<Vec<KillTrackerEntry>> {
        self.runtime
            .block_on(self.client.get_kill_tracker_history())
    }

    /// Get kill tracker stats
    pub fn get_kill_tracker_stats(&self) -> Result<KillTrackerStats> {
        self.runtime.block_on(self.client.get_kill_tracker_stats())
    }

    /// Get GM alert status for zones
    pub fn get_gm_alerts(&self) -> Result<Vec<GmAlert>> {
        self.runtime.block_on(self.client.get_gm_alerts())
    }

    /// Get say detection configuration
    pub fn get_say_detection_config(&self) -> Result<SayDetectionConfig> {
        self.runtime
            .block_on(self.client.get_say_detection_config())
    }

    /// Update say detection configuration
    pub fn put_say_detection_config(
        &self,
        config: SayDetectionConfig,
    ) -> Result<SayDetectionConfig> {
        self.runtime
            .block_on(self.client.put_say_detection_config(config))
    }

    /// Get say detection matches
    pub fn get_say_detection_matches(&self) -> Result<Vec<SayMatch>> {
        self.runtime
            .block_on(self.client.get_say_detection_matches())
    }

    /// List XAssist configurations
    pub fn list_xassist_configs(&self) -> Result<Vec<XAssistConfig>> {
        self.runtime.block_on(self.client.list_xassist_configs())
    }

    /// Get XAssist configuration for a character
    pub fn get_xassist_config(&self, character: &str) -> Result<XAssistConfig> {
        self.runtime
            .block_on(self.client.get_xassist_config(character))
    }

    /// Update XAssist configuration for a character
    pub fn put_xassist_config(
        &self,
        character: &str,
        config: XAssistConfig,
    ) -> Result<XAssistConfig> {
        self.runtime
            .block_on(self.client.put_xassist_config(character, config))
    }

    /// Delete XAssist configuration for a character
    pub fn delete_xassist_config(&self, character: &str) -> Result<()> {
        self.runtime
            .block_on(self.client.delete_xassist_config(character))
    }

    /// List chat pattern rules
    pub fn list_chat_pattern_rules(&self) -> Result<Vec<ChatPatternRule>> {
        self.runtime.block_on(self.client.list_chat_pattern_rules())
    }

    /// Get stats for chat pattern rules
    pub fn get_chat_pattern_rules_stats(&self) -> Result<ChatPatternRuleStats> {
        self.runtime
            .block_on(self.client.get_chat_pattern_rules_stats())
    }

    /// Import chat pattern rules
    pub fn import_chat_pattern_rules(&self, rules: Vec<ChatPatternRule>) -> Result<()> {
        self.runtime
            .block_on(self.client.import_chat_pattern_rules(rules))
    }

    /// Get a specific chat pattern rule
    pub fn get_chat_pattern_rule(&self, id: &str) -> Result<ChatPatternRule> {
        self.runtime.block_on(self.client.get_chat_pattern_rule(id))
    }

    /// Update a chat pattern rule
    pub fn put_chat_pattern_rule(
        &self,
        id: &str,
        rule: ChatPatternRule,
    ) -> Result<ChatPatternRule> {
        self.runtime
            .block_on(self.client.put_chat_pattern_rule(id, rule))
    }

    /// Delete a chat pattern rule
    pub fn delete_chat_pattern_rule(&self, id: &str) -> Result<()> {
        self.runtime
            .block_on(self.client.delete_chat_pattern_rule(id))
    }

    /// Toggle a chat pattern rule on/off
    pub fn toggle_chat_pattern_rule(&self, id: &str) -> Result<ChatPatternRule> {
        self.runtime
            .block_on(self.client.toggle_chat_pattern_rule(id))
    }

    /// Reset cooldown for a chat pattern rule
    pub fn reset_chat_pattern_rule_cooldown(&self, id: &str) -> Result<ChatPatternRule> {
        self.runtime
            .block_on(self.client.reset_chat_pattern_rule_cooldown(id))
    }

    /// Reset all cooldowns for chat pattern rules
    pub fn reset_all_chat_pattern_rule_cooldowns(&self) -> Result<()> {
        self.runtime
            .block_on(self.client.reset_all_chat_pattern_rule_cooldowns())
    }
}
