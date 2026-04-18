/**
 * Tests for the TextQuest SDK Client
 */

import { TextQuestClient, TextQuestClientError } from "../src/client";
import * as Types from "../src/types";

// Mock fetch globally
const mockFetch = jest.fn();
global.fetch = mockFetch as any;

describe("TextQuestClient", () => {
  const baseUrl = "http://localhost:3001";
  const apiToken = "test-token";

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe("constructor", () => {
    it("should initialize with baseUrl", () => {
      const client = new TextQuestClient({ baseUrl });
      expect(client).toBeDefined();
    });

    it("should initialize with apiToken", () => {
      const client = new TextQuestClient({ baseUrl, apiToken });
      expect(client).toBeDefined();
    });

    it("should strip trailing slash from baseUrl", () => {
      const client = new TextQuestClient({ baseUrl: `${baseUrl}/` });
      expect(client).toBeDefined();
    });
  });

  describe("health endpoint", () => {
    it("should fetch health status", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: "ok", version: "0.1.0" }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.health();

      expect(result).toEqual({ status: "ok", version: "0.1.0" });
      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/health`,
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should handle health endpoint errors", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        json: async () => ({ error: "Internal server error" }),
      });

      const client = new TextQuestClient({ baseUrl });

      await expect(client.health()).rejects.toThrow(TextQuestClientError);
    });
  });

  describe("sessions endpoint", () => {
    it("should list all sessions", async () => {
      const mockSessions: Types.Session[] = [
        {
          client_id: 1,
          character_name: "Mage1",
          zone: "Gfay",
          level: 60,
          hp_pct: 100,
          mana_pct: 80,
          endurance_pct: 95,
          status: "active",
          buff_count: 5,
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ sessions: mockSessions }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.listSessions();

      expect(result.sessions).toEqual(mockSessions);
      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/sessions`,
        expect.objectContaining({ method: "GET" })
      );
    });
  });

  describe("economy endpoints", () => {
    it("should get economy settings", async () => {
      const mockSettings: Types.EconomySettings = {
        vendor_target_stock: 100,
        vendor_restock_interval_mins: 60,
        krono_target: 10,
        tradeskill_priority: ["Brewing", "Fishing"],
        auto_harvest: true,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockSettings,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getEconomySettings();

      expect(result).toEqual(mockSettings);
    });

    it("should update economy settings", async () => {
      const updateRequest: Types.EconomySettingsRequest = {
        vendor_target_stock: 150,
      };

      const mockSettings: Types.EconomySettings = {
        vendor_target_stock: 150,
        vendor_restock_interval_mins: 60,
        krono_target: 10,
        tradeskill_priority: ["Brewing", "Fishing"],
        auto_harvest: true,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockSettings,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.updateEconomySettings(updateRequest);

      expect(result).toEqual(mockSettings);
      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/economy/settings`,
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify(updateRequest),
        })
      );
    });

    it("should list vendor routes", async () => {
      const mockRoutes: Types.VendorRoute[] = [
        {
          id: "route-1",
          name: "Bazaar Loop",
          waypoints: ["Bazaar", "Overthere", "PoK"],
          priority: 1,
          enabled: true,
          created_at: "2026-04-17T00:00:00Z",
          last_updated: "2026-04-17T00:00:00Z",
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ routes: mockRoutes }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.listVendorRoutes();

      expect(result.routes).toEqual(mockRoutes);
    });

    it("should get wealth", async () => {
      const mockWealth: Types.WealthResponse = {
        total_krono: 50,
        total_plat: 500000,
        by_character: {
          Mage1: { krono: 10, plat: 100000 },
          Mage2: { krono: 20, plat: 200000 },
        },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockWealth,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getWealth();

      expect(result).toEqual(mockWealth);
    });
  });

  describe("loot endpoints", () => {
    it("should get loot rules", async () => {
      const mockRules: Types.LootRule[] = [
        {
          id: "rule-1",
          name: "Caster gear",
          pattern: ".*silk.*",
          priority: 1,
          action: "need",
          enabled: true,
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ rules: mockRules }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getLootRules();

      expect(result.rules).toEqual(mockRules);
    });

    it("should get loot history", async () => {
      const mockHistory: Types.LootHistoryResponse = {
        entries: [
          {
            item_id: "item-1",
            item_name: "Silken Gloves",
            recipient: "Mage1",
            timestamp: "2026-04-17T10:00:00Z",
            method: "need",
          },
        ],
        last_20_total: 5000,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockHistory,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getLootHistory();

      expect(result).toEqual(mockHistory);
    });
  });

  describe("soul audit endpoints", () => {
    it("should list soul states", async () => {
      const mockSouls: Types.SoulState[] = [
        {
          character_id: "Mage1",
          recovery_status: "healthy",
          memory_usage_mb: 256,
          last_check: "2026-04-17T10:00:00Z",
          issues: [],
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ souls: mockSouls }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.listSoulStates();

      expect(result.souls).toEqual(mockSouls);
    });

    it("should get character audit", async () => {
      const mockAudit: Types.SoulCharacterAuditResponse = {
        character_id: "Mage1",
        entries: [
          {
            character_id: "Mage1",
            timestamp: "2026-04-17T10:00:00Z",
            event_type: "recovery_check",
            details: "Health check passed",
            severity: "info",
          },
        ],
        total_count: 1,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockAudit,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getCharacterAudit("Mage1");

      expect(result).toEqual(mockAudit);
    });
  });

  describe("spawn alerts endpoints", () => {
    it("should list spawn alerts", async () => {
      const mockAlerts: Types.SpawnAlert[] = [
        {
          spawn_id: "spawn-1",
          spawn_name: "Rare Boss",
          zone: "Dragon Necropolis",
          timestamp: "2026-04-17T10:00:00Z",
          status: "spawned",
          last_seen_location: "Zone corner",
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ alerts: mockAlerts }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.listSpawnAlerts();

      expect(result.alerts).toEqual(mockAlerts);
    });

    it("should get spawn alert stats", async () => {
      const mockStats: Types.SpawnAlertStatsResponse = {
        total_alerts: 42,
        alerts_today: 5,
        unique_spawns: 3,
        active_watches: 7,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockStats,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getSpawnAlertStats();

      expect(result).toEqual(mockStats);
    });

    it("should add spawn alert watch pattern", async () => {
      const pattern = "Ancient Dragon";
      const mockResponse: Types.SpawnAlertWatchListResponse = {
        patterns: ["Ancient Dragon", "Rare Boss"],
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockResponse,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.addSpawnAlertWatchPattern(pattern);

      expect(result.patterns).toContain(pattern);
      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/spawn-alerts/watch-list`,
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify({ pattern }),
        })
      );
    });
  });

  describe("gm alerts endpoints", () => {
    it("should fetch GM alert status", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          config: { enabled: true, soundEnabled: true, soundFile: null, toastEnabled: true, autoPauseEnabled: false, discordWebhookUrl: null, broadcastAllClients: true },
          presence: { isGmInZone: false, gmCount: 0, gmNames: [] },
          automationPaused: false,
        }),
      });

      const client = new TextQuestClient({ baseUrl });
      await client.getGmAlerts();

      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/gm-alerts/status`,
        expect.objectContaining({ method: "GET" })
      );
    });
  });

  describe("kill tracker endpoints", () => {
    it("should fetch kill tracker settings", async () => {
      const mockSettings: Types.KillTrackerSettings = {
        enabled: true,
        auto_report_interval_minutes: 10,
        auto_report_channel: "group",
        auto_report_include_mobs: true,
        auto_report_include_kph: true,
        track_per_character: true,
        max_session_history: 100,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockSettings,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getKillTrackerSettings();

      expect(result).toEqual(mockSettings);
      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/kill-tracker/settings`,
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should derive kill tracker stats from history", async () => {
      mockFetch
        .mockResolvedValueOnce({
          ok: true,
          json: async () => [
            {
              sessions: [
                { total_kills: 4, zone: "Dreadlands" },
                { total_kills: 2, zone: "Dreadlands" },
              ],
            },
          ],
        });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getKillTrackerStats();

      expect(result.total_kills).toBe(6);
      expect(result.kills_by_zone.Dreadlands).toBe(6);
      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/kill-tracker/history`,
        expect.objectContaining({ method: "GET" })
      );
    });
  });

  describe("operational alerts", () => {
    it("should acknowledge alerts with the ack route", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          id: 1,
          created_at: "2026-04-17T00:00:00Z",
          severity: "info",
          kind: "config_changed",
          message: "saved",
          source: null,
          actor: null,
          zone: null,
          metadata_json: null,
          acknowledged_at: null,
          acknowledged_by: null,
        }),
      });

      const client = new TextQuestClient({ baseUrl });
      await client.acknowledgeAlert(1, "admin");

      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/alerts/1/ack`,
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({ acknowledged_by: "admin" }),
        })
      );
    });
  });

  describe("chat pattern rules endpoints", () => {
    it("should list chat pattern rules", async () => {
      const mockRules: Types.ChatPatternRule[] = [
        {
          id: "rule-1",
          pattern: "LFG",
          response: "Looking for group",
          enabled: true,
          priority: 1,
          cooldown_secs: 60,
          trigger_count: 10,
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ rules: mockRules }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.listChatPatternRules();

      expect(result.rules).toEqual(mockRules);
    });

    it("should get chat pattern rule stats", async () => {
      const mockStats: Types.ChatPatternRuleStatsResponse = {
        total_rules: 10,
        enabled_count: 8,
        total_triggers: 150,
        rules_on_cooldown: 2,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockStats,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getChatPatternRuleStats();

      expect(result).toEqual(mockStats);
    });

    it("should toggle chat pattern rule", async () => {
      const mockRule: Types.ChatPatternRule = {
        id: "rule-1",
        pattern: "LFG",
        response: "Looking for group",
        enabled: false,
        priority: 1,
        cooldown_secs: 60,
        trigger_count: 10,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockRule,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.toggleChatPatternRule("rule-1");

      expect(result.enabled).toBe(false);
    });

    it("should reset chat pattern rule cooldown", async () => {
      const mockRule: Types.ChatPatternRule = {
        id: "rule-1",
        pattern: "LFG",
        response: "Looking for group",
        enabled: true,
        priority: 1,
        cooldown_secs: 60,
        trigger_count: 10,
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockRule,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.resetChatPatternRuleCooldown("rule-1");

      expect(result.id).toBe("rule-1");
    });
  });

  describe("xassist endpoints", () => {
    it("should list xassist configs", async () => {
      const mockConfigs: Record<string, Types.XAssistConfig> = {
        Mage1: {
          character: "Mage1",
          enabled: true,
          auto_assist_on_follow: true,
          assist_target: "Tank",
          keybind: "alt+a",
          last_updated: "2026-04-17T00:00:00Z",
        },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ configs: mockConfigs }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.listXAssistConfigs();

      expect(result.configs).toEqual(mockConfigs);
    });

    it("should update xassist config", async () => {
      const mockConfig: Types.XAssistConfig = {
        character: "Mage1",
        enabled: true,
        auto_assist_on_follow: false,
        assist_target: "Cleric",
        keybind: "alt+b",
        last_updated: "2026-04-17T11:00:00Z",
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockConfig,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.updateXAssistConfig("Mage1", {
        assist_target: "Cleric",
      });

      expect(result.assist_target).toBe("Cleric");
    });
  });

  describe("API token authentication", () => {
    it("should include authorization header when apiToken is set", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: "ok", version: "0.1.0" }),
      });

      const client = new TextQuestClient({ baseUrl, apiToken });
      await client.health();

      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/health`,
        expect.objectContaining({
          headers: expect.objectContaining({
            Authorization: `Bearer ${apiToken}`,
          }),
        })
      );
    });

    it("should not include authorization header when apiToken is not set", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ status: "ok", version: "0.1.0" }),
      });

      const client = new TextQuestClient({ baseUrl });
      await client.health();

      expect(mockFetch).toHaveBeenCalledWith(
        `${baseUrl}/api/health`,
        expect.not.objectContaining({
          headers: expect.objectContaining({
            Authorization: expect.anything(),
          }),
        })
      );
    });
  });

  describe("error handling", () => {
    it("should throw TextQuestClientError on fetch failure", async () => {
      mockFetch.mockRejectedValueOnce(new Error("Network error"));

      const client = new TextQuestClient({ baseUrl });

      await expect(client.health()).rejects.toThrow(TextQuestClientError);
    });

    it("should include status code in error", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: async () => ({ error: "Unauthorized" }),
      });

      const client = new TextQuestClient({ baseUrl });

      try {
        await client.health();
      } catch (error) {
        expect(error).toBeInstanceOf(TextQuestClientError);
        expect((error as TextQuestClientError).statusCode).toBe(401);
      }
    });
  });

  describe("timestamp config endpoints", () => {
    it("should list timestamp configs", async () => {
      const mockConfigs: Record<string, Types.TimestampConfig> = {
        Mage1: {
          character: "Mage1",
          enabled: true,
          log_level: "normal",
          include_timestamps: true,
          format: "24h",
          last_updated: "2026-04-17T00:00:00Z",
        },
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ configs: mockConfigs }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.listTimestampConfigs();

      expect(result.configs).toEqual(mockConfigs);
    });

    it("should get timestamp config for character", async () => {
      const mockConfig: Types.TimestampConfig = {
        character: "Mage1",
        enabled: true,
        log_level: "verbose",
        include_timestamps: true,
        format: "12h",
        last_updated: "2026-04-17T00:00:00Z",
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockConfig,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.getTimestampConfig("Mage1");

      expect(result).toEqual(mockConfig);
    });
  });

  describe("character config endpoints", () => {
    it("should list character configs", async () => {
      const mockConfigs: Types.CharacterConfig[] = [
        {
          character_name: "Mage1",
          strategy_class: "EvocationMage",
          auto_combat: true,
          auto_buff: true,
          camp_mode: false,
          last_updated: "2026-04-17T00:00:00Z",
        },
      ];

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ configs: mockConfigs }),
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.listCharacterConfigs();

      expect(result.configs).toEqual(mockConfigs);
    });

    it("should update character config", async () => {
      const mockConfig: Types.CharacterConfig = {
        character_name: "Mage1",
        strategy_class: "EvocationMage",
        auto_combat: false,
        auto_buff: true,
        camp_mode: false,
        last_updated: "2026-04-17T11:00:00Z",
      };

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => mockConfig,
      });

      const client = new TextQuestClient({ baseUrl });
      const result = await client.updateCharacterConfig("Mage1", {
        strategy_class: "EvocationMage",
        auto_combat: false,
      });

      expect(result.auto_combat).toBe(false);
    });
  });
});
