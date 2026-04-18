/**
 * TextQuest Web API Client
 *
 * A comprehensive TypeScript client for the TextQuest web API with full type support.
 * Supports both fetch and axios HTTP backends.
 */

import * as Types from "./types";

export type HttpClient = "fetch" | "axios";

/**
 * Configuration for the TextQuestClient
 */
export interface TextQuestClientConfig {
  baseUrl: string;
  apiToken?: string;
  httpClient?: HttpClient;
  timeout?: number;
}

/**
 * Main TextQuest API Client
 *
 * Provides strongly-typed methods for all TextQuest web API endpoints.
 */
export class TextQuestClient {
  private baseUrl: string;
  private apiToken?: string;
  private httpClient: HttpClient;
  private timeout: number;

  constructor(config: TextQuestClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\/$/, ""); // Remove trailing slash
    this.apiToken = config.apiToken;
    this.httpClient = config.httpClient || "fetch";
    this.timeout = config.timeout || 30000;
  }

  /**
   * Make a request to the API
   */
  private async request<T>(
    method: "GET" | "POST" | "PUT" | "DELETE",
    path: string,
    body?: unknown
  ): Promise<T> {
    const url = `${this.baseUrl}/api${path}`;
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };

    if (this.apiToken) {
      headers["Authorization"] = `Bearer ${this.apiToken}`;
    }

    try {
      if (this.httpClient === "axios") {
        return await this.requestWithAxios<T>(url, method, headers, body);
      } else {
        return await this.requestWithFetch<T>(url, method, headers, body);
      }
    } catch (error) {
      throw new TextQuestClientError(
        `Failed to ${method} ${path}: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }

  /**
   * Make a request using the fetch API
   */
  private async requestWithFetch<T>(
    url: string,
    method: string,
    headers: Record<string, string>,
    body?: unknown
  ): Promise<T> {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(url, {
        method,
        headers,
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({ error: response.statusText }));
        throw new TextQuestClientError(
          errorData.error || `HTTP ${response.status}`,
          response.status
        );
      }

      return await response.json();
    } finally {
      clearTimeout(timeoutId);
    }
  }

  /**
   * Make a request using axios
   */
  private async requestWithAxios<T>(
    url: string,
    method: string,
    headers: Record<string, string>,
    body?: unknown
  ): Promise<T> {
    // Dynamically import axios to avoid hard dependency
    const axios = (await import("axios")).default;
    try {
      const response = await axios({
        url,
        method,
        headers,
        data: body,
        timeout: this.timeout,
      });
      return response.data;
    } catch (error: unknown) {
      if (axios.isAxiosError(error)) {
        const status = error.response?.status;
        const errorMessage = error.response?.data?.error || error.message;
        throw new TextQuestClientError(errorMessage, status);
      }
      throw error;
    }
  }

  // ─── Health ───────────────────────────────────────────────────────────

  /**
   * Check API health status
   */
  async health(): Promise<Types.HealthResponse> {
    return this.request<Types.HealthResponse>("GET", "/health");
  }

  // ─── Sessions ──────────────────────────────────────────────────────────

  /**
   * List all active sessions
   */
  async listSessions(): Promise<Types.SessionsResponse> {
    return this.request<Types.SessionsResponse>("GET", "/sessions");
  }

  // ─── Accounts ──────────────────────────────────────────────────────────

  /**
   * List all accounts
   */
  async listAccounts(): Promise<Types.AccountsResponse> {
    return this.request<Types.AccountsResponse>("GET", "/accounts");
  }

  /**
   * Get a specific account
   */
  async getAccount(id: string): Promise<Types.Account> {
    return this.request<Types.Account>("GET", `/accounts/${id}`);
  }

  /**
   * Create a new account
   */
  async createAccount(request: Types.AccountCreateRequest): Promise<Types.Account> {
    return this.request<Types.Account>("POST", "/accounts", request);
  }

  /**
   * Update an account
   */
  async updateAccount(id: string, request: Types.AccountUpdateRequest): Promise<Types.Account> {
    return this.request<Types.Account>("PUT", `/accounts/${id}`, request);
  }

  /**
   * Delete an account
   */
  async deleteAccount(id: string): Promise<void> {
    return this.request<void>("DELETE", `/accounts/${id}`);
  }

  /**
   * Get credential store status
   */
  async getCredentialStoreStatus(): Promise<Types.CredentialStoreResponse> {
    return this.request<Types.CredentialStoreResponse>("GET", "/accounts/credentials");
  }

  // ─── Box Chat Settings ────────────────────────────────────────────────

  /**
   * Get Box Chat configuration
   */
  async getBoxChatSettings(): Promise<Types.BoxChatConfig> {
    return this.request<Types.BoxChatConfig>("GET", "/config/box-chat");
  }

  /**
   * Update Box Chat configuration
   */
  async updateBoxChatSettings(config: Types.BoxChatConfig): Promise<Types.BoxChatConfig> {
    return this.request<Types.BoxChatConfig>("PUT", "/config/box-chat", config);
  }

  // ─── Chat Log Settings ────────────────────────────────────────────────

  /**
   * Get Chat Log settings
   */
  async getChatLogSettings(): Promise<Types.ChatLogSettings> {
    return this.request<Types.ChatLogSettings>("GET", "/config/chat-log");
  }

  /**
   * Update Chat Log settings
   */
  async updateChatLogSettings(settings: Types.ChatLogSettings): Promise<Types.ChatLogSettings> {
    return this.request<Types.ChatLogSettings>("PUT", "/config/chat-log", settings);
  }

  // ─── Character Configuration ───────────────────────────────────────────

  /**
   * List all character configurations
   */
  async listCharacterConfigs(): Promise<Types.CharacterConfigsResponse> {
    return this.request<Types.CharacterConfigsResponse>("GET", "/config/characters");
  }

  /**
   * Update character configuration
   */
  async updateCharacterConfig(
    character: string,
    config: Types.CharacterConfigRequest
  ): Promise<Types.CharacterConfig> {
    return this.request<Types.CharacterConfig>(
      "PUT",
      `/config/characters/${character}`,
      config
    );
  }

  // ─── Auto Accept Settings ─────────────────────────────────────────────

  /**
   * Get Auto Accept settings
   */
  async getAutoAcceptSettings(): Promise<Types.AutoAcceptSettings> {
    return this.request<Types.AutoAcceptSettings>("GET", "/config/auto-accept");
  }

  /**
   * Update Auto Accept settings
   */
  async updateAutoAcceptSettings(
    settings: Types.AutoAcceptRequest
  ): Promise<Types.AutoAcceptSettings> {
    return this.request<Types.AutoAcceptSettings>("PUT", "/config/auto-accept", settings);
  }

  // ─── Player Watch Configuration ───────────────────────────────────────

  /**
   * Get Player Watch configuration
   */
  async getPlayerWatchConfig(): Promise<Types.PlayerWatchConfig> {
    return this.request<Types.PlayerWatchConfig>("GET", "/config/player-watch");
  }

  /**
   * Update Player Watch configuration
   */
  async updatePlayerWatchConfig(
    config: Types.PlayerWatchConfigRequest
  ): Promise<Types.PlayerWatchConfig> {
    return this.request<Types.PlayerWatchConfig>("PUT", "/config/player-watch", config);
  }

  // ─── Economy Settings ─────────────────────────────────────────────────

  /**
   * Get Economy settings
   */
  async getEconomySettings(): Promise<Types.EconomySettings> {
    return this.request<Types.EconomySettings>("GET", "/economy/settings");
  }

  /**
   * Update Economy settings
   */
  async updateEconomySettings(
    settings: Types.EconomySettingsRequest
  ): Promise<Types.EconomySettings> {
    return this.request<Types.EconomySettings>("PUT", "/economy/settings", settings);
  }

  /**
   * List vendor routes
   */
  async listVendorRoutes(): Promise<Types.VendorRoutesResponse> {
    return this.request<Types.VendorRoutesResponse>("GET", "/economy/vendor-routes");
  }

  /**
   * Create a vendor route
   */
  async createVendorRoute(route: Types.VendorRouteRequest): Promise<Types.VendorRoute> {
    return this.request<Types.VendorRoute>("POST", "/economy/vendor-routes", route);
  }

  /**
   * Update a vendor route
   */
  async updateVendorRoute(
    id: string,
    route: Partial<Types.VendorRouteRequest>
  ): Promise<Types.VendorRoute> {
    return this.request<Types.VendorRoute>("PUT", `/economy/vendor-routes/${id}`, route);
  }

  /**
   * Delete a vendor route
   */
  async deleteVendorRoute(id: string): Promise<void> {
    return this.request<void>("DELETE", `/economy/vendor-routes/${id}`);
  }

  /**
   * Get current wealth status
   */
  async getWealth(): Promise<Types.WealthResponse> {
    return this.request<Types.WealthResponse>("GET", "/economy/wealth");
  }

  // ─── Soul Audit ────────────────────────────────────────────────────────

  /**
   * List all soul states
   */
  async listSoulStates(): Promise<Types.SoulStatesResponse> {
    return this.request<Types.SoulStatesResponse>("GET", "/soul");
  }

  /**
   * Get a specific character's soul state
   */
  async getSoulState(characterId: string): Promise<Types.SoulState> {
    return this.request<Types.SoulState>("GET", `/soul/${characterId}`);
  }

  /**
   * Get all audit entries
   */
  async getAllAudit(): Promise<Types.SoulAuditResponse> {
    return this.request<Types.SoulAuditResponse>("GET", "/soul/audit");
  }

  /**
   * Export all audit entries as CSV
   */
  async exportAllAuditCsv(): Promise<string> {
    return this.request<string>("GET", "/soul/audit/export.csv");
  }

  /**
   * Get audit entries for a specific character
   */
  async getCharacterAudit(characterId: string): Promise<Types.SoulCharacterAuditResponse> {
    return this.request<Types.SoulCharacterAuditResponse>("GET", `/soul/audit/${characterId}`);
  }

  /**
   * Export character audit entries as CSV
   */
  async exportCharacterAuditCsv(characterId: string): Promise<string> {
    return this.request<string>("GET", `/soul/audit/${characterId}/export.csv`);
  }

  // ─── Loot Configuration ────────────────────────────────────────────────

  /**
   * Get loot rules
   */
  async getLootRules(): Promise<Types.LootRulesResponse> {
    return this.request<Types.LootRulesResponse>("GET", "/loot/rules");
  }

  /**
   * Update loot rules
   */
  async updateLootRules(rules: Types.LootRule[]): Promise<Types.LootRulesResponse> {
    return this.request<Types.LootRulesResponse>("PUT", "/loot/rules", { rules });
  }

  /**
   * Get loot filters for all characters
   */
  async getLootFilters(): Promise<Types.LootFiltersResponse> {
    return this.request<Types.LootFiltersResponse>("GET", "/loot/filters");
  }

  /**
   * Update loot filter for a character
   */
  async updateLootFilter(
    character: string,
    filter: Types.LootFilterRequest
  ): Promise<Types.LootFilter> {
    return this.request<Types.LootFilter>("PUT", `/loot/filters/${character}`, filter);
  }

  /**
   * Get master looter configuration
   */
  async getMasterLooter(): Promise<Types.MasterLooterResponse> {
    return this.request<Types.MasterLooterResponse>("GET", "/loot/master-looter");
  }

  /**
   * Update master looter configuration
   */
  async updateMasterLooter(
    config: Types.MasterLooterRequest
  ): Promise<Types.MasterLooterResponse> {
    return this.request<Types.MasterLooterResponse>("PUT", "/loot/master-looter", config);
  }

  /**
   * Get loot distribution
   */
  async getLootDistribution(): Promise<Types.LootDistributionResponse> {
    return this.request<Types.LootDistributionResponse>("GET", "/loot/distribution");
  }

  /**
   * Update loot distribution
   */
  async updateLootDistribution(
    entry: Types.LootDistributionRequest
  ): Promise<Types.LootDistributionResponse> {
    return this.request<Types.LootDistributionResponse>("PUT", "/loot/distribution", entry);
  }

  /**
   * Get loot history
   */
  async getLootHistory(): Promise<Types.LootHistoryResponse> {
    return this.request<Types.LootHistoryResponse>("GET", "/loot/history");
  }

  // ─── Kill Tracker ─────────────────────────────────────────────────────

  /**
   * Get kill tracker data
   */
  async getKillTracker(): Promise<Types.KillTrackerResponse> {
    return this.request<Types.KillTrackerResponse>("GET", "/kill-tracker");
  }

  /**
   * Get kill tracker statistics
   */
  async getKillTrackerStats(): Promise<Types.KillTrackerStatsResponse> {
    return this.request<Types.KillTrackerStatsResponse>("GET", "/kill-tracker/stats");
  }

  // ─── Spawn Alerts ─────────────────────────────────────────────────────

  /**
   * List spawn alerts
   */
  async listSpawnAlerts(): Promise<Types.SpawnAlertsResponse> {
    return this.request<Types.SpawnAlertsResponse>("GET", "/spawn-alerts");
  }

  /**
   * Clear all spawn alerts
   */
  async clearSpawnAlerts(): Promise<void> {
    return this.request<void>("DELETE", "/spawn-alerts");
  }

  /**
   * Get spawn alert statistics
   */
  async getSpawnAlertStats(): Promise<Types.SpawnAlertStatsResponse> {
    return this.request<Types.SpawnAlertStatsResponse>("GET", "/spawn-alerts/stats");
  }

  /**
   * Get spawn alert configuration
   */
  async getSpawnAlertConfig(): Promise<Types.SpawnAlertConfig> {
    return this.request<Types.SpawnAlertConfig>("GET", "/spawn-alerts/config");
  }

  /**
   * Update spawn alert configuration
   */
  async updateSpawnAlertConfig(
    config: Types.SpawnAlertConfigRequest
  ): Promise<Types.SpawnAlertConfig> {
    return this.request<Types.SpawnAlertConfig>("PUT", "/spawn-alerts/config", config);
  }

  /**
   * Get spawn alert watch list
   */
  async getSpawnAlertWatchList(): Promise<Types.SpawnAlertWatchListResponse> {
    return this.request<Types.SpawnAlertWatchListResponse>("GET", "/spawn-alerts/watch-list");
  }

  /**
   * Add a pattern to the spawn alert watch list
   */
  async addSpawnAlertWatchPattern(pattern: string): Promise<Types.SpawnAlertWatchListResponse> {
    return this.request<Types.SpawnAlertWatchListResponse>(
      "PUT",
      `/spawn-alerts/watch-list/${encodeURIComponent(pattern)}`,
      { pattern }
    );
  }

  /**
   * Remove a pattern from the spawn alert watch list
   */
  async removeSpawnAlertWatchPattern(pattern: string): Promise<Types.SpawnAlertWatchListResponse> {
    return this.request<Types.SpawnAlertWatchListResponse>(
      "DELETE",
      `/spawn-alerts/watch-list/${encodeURIComponent(pattern)}`
    );
  }

  // ─── Timestamp Configuration ───────────────────────────────────────────

  /**
   * List all timestamp configurations
   */
  async listTimestampConfigs(): Promise<Types.TimestampConfigsResponse> {
    return this.request<Types.TimestampConfigsResponse>("GET", "/timestamp-config");
  }

  /**
   * Get timestamp configuration for a character
   */
  async getTimestampConfig(character: string): Promise<Types.TimestampConfig> {
    return this.request<Types.TimestampConfig>("GET", `/timestamp-config/${character}`);
  }

  /**
   * Update timestamp configuration for a character
   */
  async updateTimestampConfig(
    character: string,
    config: Types.TimestampConfigRequest
  ): Promise<Types.TimestampConfig> {
    return this.request<Types.TimestampConfig>(
      "PUT",
      `/timestamp-config/${character}`,
      config
    );
  }

  // ─── GM Alerts ────────────────────────────────────────────────────────

  /**
   * Get GM alert status
   */
  async getGmAlerts(): Promise<Types.GmAlertsResponse> {
    return this.request<Types.GmAlertsResponse>("GET", "/gm-alerts");
  }

  // ─── Operational Alerts ───────────────────────────────────────────────

  /**
   * Get alerting configuration
   */
  async getAlertingConfig(): Promise<Types.AlertingConfig> {
    return this.request<Types.AlertingConfig>("GET", "/alerts/config");
  }

  /**
   * Update alerting configuration
   */
  async updateAlertingConfig(
    config: Types.AlertingConfigRequest
  ): Promise<Types.AlertingConfig> {
    return this.request<Types.AlertingConfig>("PUT", "/alerts/config", config);
  }

  /**
   * List operational alerts
   */
  async listOperationalAlerts(): Promise<Types.OperationalAlertsResponse> {
    return this.request<Types.OperationalAlertsResponse>("GET", "/alerts");
  }

  /**
   * Acknowledge an alert
   */
  async acknowledgeAlert(id: number, acknowledgedBy: string): Promise<Types.OperationalAlert> {
    return this.request<Types.OperationalAlert>("PUT", `/alerts/${id}/acknowledge`, {
      acknowledged_by: acknowledgedBy,
    });
  }

  // ─── XAssist Configuration ────────────────────────────────────────────

  /**
   * List all XAssist configurations
   */
  async listXAssistConfigs(): Promise<Types.XAssistConfigsResponse> {
    return this.request<Types.XAssistConfigsResponse>("GET", "/xassist/configs");
  }

  /**
   * Get XAssist configuration for a character
   */
  async getXAssistConfig(character: string): Promise<Types.XAssistConfig> {
    return this.request<Types.XAssistConfig>("GET", `/xassist/config/${character}`);
  }

  /**
   * Update XAssist configuration for a character
   */
  async updateXAssistConfig(
    character: string,
    config: Types.XAssistConfigRequest
  ): Promise<Types.XAssistConfig> {
    return this.request<Types.XAssistConfig>("PUT", `/xassist/config/${character}`, config);
  }

  /**
   * Delete XAssist configuration for a character
   */
  async deleteXAssistConfig(character: string): Promise<void> {
    return this.request<void>("DELETE", `/xassist/config/${character}`);
  }

  // ─── Chat Pattern Rules ───────────────────────────────────────────────

  /**
   * List chat pattern rules
   */
  async listChatPatternRules(): Promise<Types.ChatPatternRulesResponse> {
    return this.request<Types.ChatPatternRulesResponse>("GET", "/chat-pattern-rules");
  }

  /**
   * Get chat pattern rule statistics
   */
  async getChatPatternRuleStats(): Promise<Types.ChatPatternRuleStatsResponse> {
    return this.request<Types.ChatPatternRuleStatsResponse>("GET", "/chat-pattern-rules/stats");
  }

  /**
   * Get a specific chat pattern rule
   */
  async getChatPatternRule(id: string): Promise<Types.ChatPatternRule> {
    return this.request<Types.ChatPatternRule>("GET", `/chat-pattern-rules/${id}`);
  }

  /**
   * Create or update a chat pattern rule
   */
  async updateChatPatternRule(
    id: string,
    rule: Types.ChatPatternRuleRequest
  ): Promise<Types.ChatPatternRule> {
    return this.request<Types.ChatPatternRule>("PUT", `/chat-pattern-rules/${id}`, rule);
  }

  /**
   * Delete a chat pattern rule
   */
  async deleteChatPatternRule(id: string): Promise<void> {
    return this.request<void>("DELETE", `/chat-pattern-rules/${id}`);
  }

  /**
   * Toggle a chat pattern rule enabled/disabled
   */
  async toggleChatPatternRule(id: string): Promise<Types.ChatPatternRule> {
    return this.request<Types.ChatPatternRule>("PUT", `/chat-pattern-rules/${id}/toggle`);
  }

  /**
   * Reset cooldown for a chat pattern rule
   */
  async resetChatPatternRuleCooldown(id: string): Promise<Types.ChatPatternRule> {
    return this.request<Types.ChatPatternRule>(
      "PUT",
      `/chat-pattern-rules/${id}/reset-cooldown`
    );
  }

  /**
   * Reset all chat pattern rule cooldowns
   */
  async resetAllChatPatternRuleCooldowns(): Promise<Types.ChatPatternRulesResponse> {
    return this.request<Types.ChatPatternRulesResponse>(
      "PUT",
      "/chat-pattern-rules/cooldowns/reset"
    );
  }

  /**
   * Import chat pattern rules
   */
  async importChatPatternRules(
    rules: Types.ChatPatternRuleRequest[]
  ): Promise<Types.ChatPatternRulesResponse> {
    return this.request<Types.ChatPatternRulesResponse>(
      "POST",
      "/chat-pattern-rules/import",
      { rules }
    );
  }

  // ─── Say Detection ────────────────────────────────────────────────────

  /**
   * Get say detection configuration
   */
  async getSayDetectionConfig(): Promise<Types.SayDetectionResponse> {
    return this.request<Types.SayDetectionResponse>("GET", "/say-detection");
  }
}

/**
 * Custom error class for TextQuestClient errors
 */
export class TextQuestClientError extends Error {
  public statusCode?: number;

  constructor(message: string, statusCode?: number) {
    super(message);
    this.name = "TextQuestClientError";
    this.statusCode = statusCode;
  }
}

export * as Types from "./types";
