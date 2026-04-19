use crate::error::{Error, Result};
use crate::models::*;
use reqwest::{Client as ReqwestClient, StatusCode};
use std::collections::HashMap;

/// Async TextQuest Web API client
pub struct Client {
    base_url: String,
    http_client: ReqwestClient,
    api_token: Option<String>,
}

impl Client {
    /// Create a new API client with default configuration
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::with_token(base_url, None)
    }

    /// Create a new API client with an API token for authentication
    pub fn with_token(base_url: impl Into<String>, token: Option<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http_client: ReqwestClient::new(),
            api_token: token,
        }
    }

    /// Set the API token for subsequent requests
    pub fn set_token(&mut self, token: Option<String>) {
        self.api_token = token;
    }

    fn build_url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{}/api{}", base, path)
    }

    fn add_token(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.api_token {
            req.header("X-API-Token", token)
        } else {
            req
        }
    }

    // ─── Health & Info ──────────────────────────────────────────────────────

    /// Get API health status
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = self.build_url("/health");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<HealthResponse>(response).await
    }

    // ─── Sessions ───────────────────────────────────────────────────────────

    /// List all active sessions
    pub async fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        let url = self.build_url("/sessions");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<SessionInfo>>(response).await
    }

    // ─── Chat Log Settings ──────────────────────────────────────────────────

    /// Get chat log settings
    pub async fn get_chat_log_settings(&self) -> Result<ChatLogSettings> {
        let url = self.build_url("/chat-log/settings");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<ChatLogSettings>(response).await
    }

    /// Update chat log settings
    pub async fn put_chat_log_settings(
        &self,
        settings: ChatLogSettings,
    ) -> Result<ChatLogSettings> {
        let url = self.build_url("/chat-log/settings");
        let req = self.http_client.put(&url).json(&settings);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<ChatLogSettings>(response).await
    }

    // ─── Box Chat Settings ──────────────────────────────────────────────────

    /// Get box chat settings
    pub async fn get_box_chat_settings(&self) -> Result<BoxChatConfig> {
        let url = self.build_url("/box-chat/settings");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<BoxChatConfig>(response).await
    }

    /// Update box chat settings
    pub async fn put_box_chat_settings(&self, config: BoxChatConfig) -> Result<BoxChatConfig> {
        let url = self.build_url("/box-chat/settings");
        let req = self.http_client.put(&url).json(&config);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<BoxChatConfig>(response).await
    }

    // ─── Character Configuration ────────────────────────────────────────────

    /// List all character configurations
    pub async fn list_character_configs(&self) -> Result<Vec<CharacterConfig>> {
        let url = self.build_url("/config/characters");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<CharacterConfig>>(response).await
    }

    /// Update configuration for a character
    pub async fn put_character_config(
        &self,
        character: &str,
        config: CharacterConfig,
    ) -> Result<CharacterConfig> {
        let url = self.build_url(&format!("/config/characters/{}", character));
        let req = self.http_client.put(&url).json(&config);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<CharacterConfig>(response).await
    }

    // ─── Auto-Accept Settings ──────────────────────────────────────────────

    /// Get auto-accept settings
    pub async fn get_auto_accept_settings(&self) -> Result<AutoAcceptSettings> {
        let url = self.build_url("/config/auto-accept");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<AutoAcceptSettings>(response).await
    }

    /// Update auto-accept settings
    pub async fn put_auto_accept_settings(
        &self,
        settings: AutoAcceptSettings,
    ) -> Result<AutoAcceptSettings> {
        let url = self.build_url("/config/auto-accept");
        let req = self.http_client.put(&url).json(&settings);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<AutoAcceptSettings>(response).await
    }

    // ─── Player Watch Configuration ─────────────────────────────────────────

    /// Get player watch configuration
    pub async fn get_player_watch_config(&self) -> Result<PlayerWatchConfig> {
        let url = self.build_url("/config/player-watch");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<PlayerWatchConfig>(response).await
    }

    /// Update player watch configuration
    pub async fn put_player_watch_config(
        &self,
        config: PlayerWatchConfig,
    ) -> Result<PlayerWatchConfig> {
        let url = self.build_url("/config/player-watch");
        let req = self.http_client.put(&url).json(&config);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<PlayerWatchConfig>(response).await
    }

    // ─── Economy ────────────────────────────────────────────────────────────

    /// Get economy settings
    pub async fn get_economy_settings(&self) -> Result<EconomySettings> {
        let url = self.build_url("/economy/settings");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<EconomySettings>(response).await
    }

    /// Update economy settings
    pub async fn put_economy_settings(&self, settings: EconomySettings) -> Result<EconomySettings> {
        let url = self.build_url("/economy/settings");
        let req = self.http_client.put(&url).json(&settings);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<EconomySettings>(response).await
    }

    /// List all vendor routes
    pub async fn list_vendor_routes(&self) -> Result<Vec<VendorRoute>> {
        let url = self.build_url("/economy/vendor-routes");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<VendorRoute>>(response).await
    }

    /// Create a new vendor route
    pub async fn create_vendor_route(&self, route: VendorRoute) -> Result<VendorRoute> {
        let url = self.build_url("/economy/vendor-routes");
        let req = self.http_client.post(&url).json(&route);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<VendorRoute>(response).await
    }

    /// Update an existing vendor route
    pub async fn update_vendor_route(&self, id: &str, route: VendorRoute) -> Result<VendorRoute> {
        let url = self.build_url(&format!("/economy/vendor-routes/{}", id));
        let req = self.http_client.put(&url).json(&route);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<VendorRoute>(response).await
    }

    /// Delete a vendor route
    pub async fn delete_vendor_route(&self, id: &str) -> Result<()> {
        let url = self.build_url(&format!("/economy/vendor-routes/{}", id));
        let req = self.http_client.delete(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error = response.text().await.unwrap_or_default();
                Err(Error::ApiError {
                    status: status.as_u16(),
                    message: error,
                })
            }
        }
    }

    /// Get wealth summary
    pub async fn get_wealth(&self) -> Result<WealthHistory> {
        let url = self.build_url("/economy/wealth");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<WealthHistory>(response).await
    }

    // ─── Loot ──────────────────────────────────────────────────────────────

    /// Get loot rules
    pub async fn get_loot_rules(&self) -> Result<LootRules> {
        let url = self.build_url("/loot/rules");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<LootRules>(response).await
    }

    /// Update loot rules
    pub async fn put_loot_rules(&self, rules: LootRules) -> Result<LootRules> {
        let url = self.build_url("/loot/rules");
        let req = self.http_client.put(&url).json(&rules);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<LootRules>(response).await
    }

    /// Get loot filters
    pub async fn get_loot_filters(&self) -> Result<Vec<LootFilter>> {
        let url = self.build_url("/loot/filters");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<LootFilter>>(response).await
    }

    /// Update loot filter for a character
    pub async fn put_loot_filter(&self, character: &str, filter: LootFilter) -> Result<LootFilter> {
        let url = self.build_url(&format!("/loot/filters/{}", character));
        let req = self.http_client.put(&url).json(&filter);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<LootFilter>(response).await
    }

    /// Get master looter assignment
    pub async fn get_master_looter(&self) -> Result<MasterLooter> {
        let url = self.build_url("/loot/master-looter");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<MasterLooter>(response).await
    }

    /// Set master looter
    pub async fn put_master_looter(&self, looter: MasterLooter) -> Result<MasterLooter> {
        let url = self.build_url("/loot/master-looter");
        let req = self.http_client.put(&url).json(&looter);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<MasterLooter>(response).await
    }

    /// Get loot distribution settings
    pub async fn get_loot_distribution(&self) -> Result<LootDistribution> {
        let url = self.build_url("/loot/distribution");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<LootDistribution>(response).await
    }

    /// Update loot distribution settings
    pub async fn put_loot_distribution(&self, dist: LootDistribution) -> Result<LootDistribution> {
        let url = self.build_url("/loot/distribution");
        let req = self.http_client.put(&url).json(&dist);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<LootDistribution>(response).await
    }

    /// Get loot history
    pub async fn get_loot_history(&self) -> Result<Vec<LootHistoryEntry>> {
        let url = self.build_url("/loot/history");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<LootHistoryEntry>>(response)
            .await
    }

    // ─── Soul ──────────────────────────────────────────────────────────────

    /// List all soul states
    pub async fn list_soul_states(&self) -> Result<Vec<SoulState>> {
        let url = self.build_url("/soul");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<SoulState>>(response).await
    }

    /// Get soul state for a specific character
    pub async fn get_soul_state(&self, character_id: &str) -> Result<SoulState> {
        let url = self.build_url(&format!("/soul/{}", character_id));
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<SoulState>(response).await
    }

    /// Get soul audit log
    pub async fn get_soul_audit(&self) -> Result<Vec<serde_json::Value>> {
        let url = self.build_url("/soul/audit");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<serde_json::Value>>(response)
            .await
    }

    /// Get soul audit for a specific character
    pub async fn get_character_audit(&self, character_id: &str) -> Result<Vec<serde_json::Value>> {
        let url = self.build_url(&format!("/soul/audit/{}", character_id));
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<serde_json::Value>>(response)
            .await
    }

    // ─── Alerts ────────────────────────────────────────────────────────────

    /// Get alerting configuration
    pub async fn get_alerts_config(&self) -> Result<AlertingConfig> {
        let url = self.build_url("/alerts/config");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<AlertingConfig>(response).await
    }

    /// Update alerting configuration
    pub async fn put_alerts_config(&self, config: AlertingConfig) -> Result<AlertingConfig> {
        let url = self.build_url("/alerts/config");
        let req = self.http_client.put(&url).json(&config);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<AlertingConfig>(response).await
    }

    /// Get alert history
    pub async fn get_alerts_history(&self) -> Result<Vec<AlertEntry>> {
        let url = self.build_url("/alerts/history");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<AlertEntry>>(response).await
    }

    // ─── Spawn Alerts ──────────────────────────────────────────────────────

    /// List spawn alerts
    pub async fn list_spawn_alerts(&self) -> Result<Vec<SpawnAlert>> {
        let url = self.build_url("/spawn-alerts");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<SpawnAlert>>(response).await
    }

    /// Clear all spawn alerts
    pub async fn clear_spawn_alerts(&self) -> Result<()> {
        let url = self.build_url("/spawn-alerts");
        let req = self.http_client.delete(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error = response.text().await.unwrap_or_default();
                Err(Error::ApiError {
                    status: status.as_u16(),
                    message: error,
                })
            }
        }
    }

    /// Get spawn alert stats
    pub async fn get_spawn_alert_stats(&self) -> Result<serde_json::Value> {
        let url = self.build_url("/spawn-alerts/stats");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<serde_json::Value>(response).await
    }

    /// Get spawn alert configuration
    pub async fn get_spawn_alert_config(&self) -> Result<SpawnAlertConfig> {
        let url = self.build_url("/spawn-alerts/config");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<SpawnAlertConfig>(response).await
    }

    /// Update spawn alert configuration
    pub async fn put_spawn_alert_config(
        &self,
        config: SpawnAlertConfig,
    ) -> Result<SpawnAlertConfig> {
        let url = self.build_url("/spawn-alerts/config");
        let req = self.http_client.put(&url).json(&config);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<SpawnAlertConfig>(response).await
    }

    /// Get spawn alert watch list
    pub async fn get_spawn_watch_list(&self) -> Result<Vec<String>> {
        let url = self.build_url("/spawn-alerts/watch-list");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<String>>(response).await
    }

    /// Add a pattern to spawn alert watch list
    pub async fn put_spawn_watch_pattern(&self, pattern: &str) -> Result<()> {
        let url = self.build_url(&format!("/spawn-alerts/watch-list/{}", pattern));
        let req = self.http_client.put(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error = response.text().await.unwrap_or_default();
                Err(Error::ApiError {
                    status: status.as_u16(),
                    message: error,
                })
            }
        }
    }

    /// Remove a pattern from spawn alert watch list
    pub async fn delete_spawn_watch_pattern(&self, pattern: &str) -> Result<()> {
        let url = self.build_url(&format!("/spawn-alerts/watch-list/{}", pattern));
        let req = self.http_client.delete(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error = response.text().await.unwrap_or_default();
                Err(Error::ApiError {
                    status: status.as_u16(),
                    message: error,
                })
            }
        }
    }

    // ─── Timestamp Config ──────────────────────────────────────────────────

    /// List timestamp configurations (returns a map of character name → config)
    pub async fn list_timestamp_configs(&self) -> Result<HashMap<String, TimestampConfig>> {
        let url = self.build_url("/timestamp-config");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<HashMap<String, TimestampConfig>>(response)
            .await
    }

    /// Get timestamp configuration for a character
    pub async fn get_timestamp_config(&self, character: &str) -> Result<TimestampConfig> {
        let url = self.build_url(&format!("/timestamp-config/{}", character));
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<TimestampConfig>(response).await
    }

    /// Update timestamp configuration for a character
    pub async fn put_timestamp_config(
        &self,
        character: &str,
        config: TimestampFormat,
    ) -> Result<TimestampFormat> {
        let url = self.build_url(&format!("/timestamp-config/{}", character));
        let req = self.http_client.put(&url).json(&config);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<TimestampFormat>(response).await
    }

    // ─── Kill Tracker ──────────────────────────────────────────────────────

    /// Get kill tracker history
    pub async fn get_kill_tracker_history(&self) -> Result<Vec<KillTrackerEntry>> {
        let url = self.build_url("/kill-tracker/history");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<KillTrackerEntry>>(response)
            .await
    }

    /// Get kill tracker stats
    pub async fn get_kill_tracker_stats(&self) -> Result<KillTrackerStats> {
        let url = self.build_url("/kill-tracker/stats");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<KillTrackerStats>(response).await
    }

    // ─── GM Alerts ─────────────────────────────────────────────────────────

    /// Get GM alert status for zones
    pub async fn get_gm_alerts(&self) -> Result<Vec<GmAlert>> {
        let url = self.build_url("/gm-alerts");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<GmAlert>>(response).await
    }

    // ─── Say Detection ─────────────────────────────────────────────────────

    /// Get say detection configuration
    pub async fn get_say_detection_config(&self) -> Result<SayDetectionConfig> {
        let url = self.build_url("/say-detection/config");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<SayDetectionConfig>(response).await
    }

    /// Update say detection configuration
    pub async fn put_say_detection_config(
        &self,
        config: SayDetectionConfig,
    ) -> Result<SayDetectionConfig> {
        let url = self.build_url("/say-detection/config");
        let req = self.http_client.put(&url).json(&config);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<SayDetectionConfig>(response).await
    }

    /// Get say detection matches
    pub async fn get_say_detection_matches(&self) -> Result<Vec<SayMatch>> {
        let url = self.build_url("/say-detection/matches");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<SayMatch>>(response).await
    }

    // ─── XAssist ───────────────────────────────────────────────────────────

    /// List XAssist configurations
    pub async fn list_xassist_configs(&self) -> Result<Vec<XAssistConfig>> {
        let url = self.build_url("/xassist/configs");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<XAssistConfig>>(response).await
    }

    /// Get XAssist configuration for a character
    pub async fn get_xassist_config(&self, character: &str) -> Result<XAssistConfig> {
        let url = self.build_url(&format!("/xassist/config/{}", character));
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<XAssistConfig>(response).await
    }

    /// Update XAssist configuration for a character
    pub async fn put_xassist_config(
        &self,
        character: &str,
        config: XAssistConfig,
    ) -> Result<XAssistConfig> {
        let url = self.build_url(&format!("/xassist/config/{}", character));
        let req = self.http_client.put(&url).json(&config);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<XAssistConfig>(response).await
    }

    /// Delete XAssist configuration for a character
    pub async fn delete_xassist_config(&self, character: &str) -> Result<()> {
        let url = self.build_url(&format!("/xassist/config/{}", character));
        let req = self.http_client.delete(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error = response.text().await.unwrap_or_default();
                Err(Error::ApiError {
                    status: status.as_u16(),
                    message: error,
                })
            }
        }
    }

    // ─── Chat Pattern Rules ────────────────────────────────────────────────

    /// List chat pattern rules
    pub async fn list_chat_pattern_rules(&self) -> Result<Vec<ChatPatternRule>> {
        let url = self.build_url("/chat-pattern-rules");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<Vec<ChatPatternRule>>(response).await
    }

    /// Get stats for chat pattern rules
    pub async fn get_chat_pattern_rules_stats(&self) -> Result<ChatPatternRuleStats> {
        let url = self.build_url("/chat-pattern-rules/stats");
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<ChatPatternRuleStats>(response).await
    }

    /// Import chat pattern rules
    pub async fn import_chat_pattern_rules(&self, rules: Vec<ChatPatternRule>) -> Result<()> {
        let url = self.build_url("/chat-pattern-rules/import");
        let req = self.http_client.post(&url).json(&rules);
        let req = self.add_token(req);

        let response = req.send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::CREATED => Ok(()),
            status => {
                let error = response.text().await.unwrap_or_default();
                Err(Error::ApiError {
                    status: status.as_u16(),
                    message: error,
                })
            }
        }
    }

    /// Get a specific chat pattern rule
    pub async fn get_chat_pattern_rule(&self, id: &str) -> Result<ChatPatternRule> {
        let url = self.build_url(&format!("/chat-pattern-rules/{}", id));
        let req = self.http_client.get(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<ChatPatternRule>(response).await
    }

    /// Update a chat pattern rule
    pub async fn put_chat_pattern_rule(
        &self,
        id: &str,
        rule: ChatPatternRule,
    ) -> Result<ChatPatternRule> {
        let url = self.build_url(&format!("/chat-pattern-rules/{}", id));
        let req = self.http_client.put(&url).json(&rule);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<ChatPatternRule>(response).await
    }

    /// Delete a chat pattern rule
    pub async fn delete_chat_pattern_rule(&self, id: &str) -> Result<()> {
        let url = self.build_url(&format!("/chat-pattern-rules/{}", id));
        let req = self.http_client.delete(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error = response.text().await.unwrap_or_default();
                Err(Error::ApiError {
                    status: status.as_u16(),
                    message: error,
                })
            }
        }
    }

    /// Toggle a chat pattern rule on/off
    pub async fn toggle_chat_pattern_rule(&self, id: &str) -> Result<ChatPatternRule> {
        let url = self.build_url(&format!("/chat-pattern-rules/{}/toggle", id));
        let req = self.http_client.put(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<ChatPatternRule>(response).await
    }

    /// Reset cooldown for a chat pattern rule
    pub async fn reset_chat_pattern_rule_cooldown(&self, id: &str) -> Result<ChatPatternRule> {
        let url = self.build_url(&format!("/chat-pattern-rules/{}/reset-cooldown", id));
        let req = self.http_client.put(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        self.handle_response::<ChatPatternRule>(response).await
    }

    /// Reset all cooldowns for chat pattern rules
    pub async fn reset_all_chat_pattern_rule_cooldowns(&self) -> Result<()> {
        let url = self.build_url("/chat-pattern-rules/cooldowns/reset");
        let req = self.http_client.put(&url);
        let req = self.add_token(req);

        let response = req.send().await?;
        match response.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            status => {
                let error = response.text().await.unwrap_or_default();
                Err(Error::ApiError {
                    status: status.as_u16(),
                    message: error,
                })
            }
        }
    }

    // ─── Helper Methods ────────────────────────────────────────────────────

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();
        let body = response.text().await?;

        match status {
            StatusCode::OK | StatusCode::CREATED => serde_json::from_str::<T>(&body)
                .map_err(|e| Error::Serde(format!("Failed to deserialize response: {}", e))),
            _ => {
                if let Ok(error) = serde_json::from_str::<ErrorResponse>(&body) {
                    Err(Error::ApiError {
                        status: status.as_u16(),
                        message: error.error,
                    })
                } else {
                    Err(Error::ApiError {
                        status: status.as_u16(),
                        message: body,
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests_extended {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    async fn build_mock_response(
        status_line: &str,
        body: &str,
        content_type: Option<&str>,
    ) -> reqwest::Response {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let status_line = status_line.to_string();
        let body = body.to_string();
        let content_type = content_type.map(|ct| ct.to_string());

        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};

            let (stream, _) = listener.accept().unwrap();
            // Read and discard the request headers before sending the response
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap_or(0);
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            let ct_header = content_type
                .map(|ct| format!("Content-Type: {}\r\n", ct))
                .unwrap_or_default();
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                status_line,
                body.len(),
                ct_header,
                body
            );
            reader.get_ref().write_all(response.as_bytes()).unwrap();
        });

        reqwest::get(format!("http://{}", addr)).await.unwrap()
    }

    #[tokio::test]
    async fn handle_response_decodes_json_error_body_into_api_error() {
        let client = Client::new("http://127.0.0.1");
        let response = build_mock_response(
            "400 Bad Request",
            r#"{"error":"invalid token"}"#,
            Some("application/json"),
        )
        .await;

        let err = client
            .handle_response::<serde_json::Value>(response)
            .await
            .unwrap_err();

        match err {
            Error::ApiError { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, "invalid token");
            }
            other => panic!("expected ApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn handle_response_preserves_non_json_error_body() {
        let client = Client::new("http://127.0.0.1");
        let response =
            build_mock_response("502 Bad Gateway", "upstream exploded", Some("text/plain")).await;

        let err = client
            .handle_response::<serde_json::Value>(response)
            .await
            .unwrap_err();

        match err {
            Error::ApiError { status, message } => {
                assert_eq!(status, 502);
                assert_eq!(message, "upstream exploded");
            }
            other => panic!("expected ApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn handle_response_uses_raw_body_for_malformed_json_error() {
        let client = Client::new("http://127.0.0.1");
        let malformed = r#"{"error":"#;
        let response =
            build_mock_response("400 Bad Request", malformed, Some("application/json")).await;

        let err = client
            .handle_response::<serde_json::Value>(response)
            .await
            .unwrap_err();

        match err {
            Error::ApiError { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, malformed);
            }
            other => panic!("expected ApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn handle_response_uses_empty_message_for_empty_error_body() {
        let client = Client::new("http://127.0.0.1");
        let response =
            build_mock_response("500 Internal Server Error", "", Some("application/json")).await;

        let err = client
            .handle_response::<serde_json::Value>(response)
            .await
            .unwrap_err();

        match err {
            Error::ApiError { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "");
            }
            other => panic!("expected ApiError, got {:?}", other),
        }
    }
}
