import { renderHook, waitFor, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useCharacterConfigs } from "./useTuning";
import { jsonResponse } from "../test/http";

describe("useCharacterConfigs", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("uses demo mode on backend unavailability", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(new Response("", { status: 501 }));

    const { result } = renderHook(() => useCharacterConfigs());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.configs).toEqual([]);
    expect(result.current.error).toBe(
      "Character config backend unavailable (demo mode)"
    );
  });

  it("saves a character config and refreshes the list", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse([
          {
            character_name: "Alpha",
            class: "Wizard",
            role: "DPS",
            heal_at_pct: 50,
            mana_sit_pct: 20,
            nuke_at_pct: 80,
            rotation: [],
            class_params: {
              cross_client_heal_enabled: true,
              cross_client_heal_threshold_pct: 82,
              cross_client_heal_priority: 14,
              cross_client_claim_timeout_ms: 3100,
            },
            auto_rez: {
              enabled: false,
              min_xp_pct: 90,
              trusted_casters: [],
              decline_if_untrusted: false,
              delay_ms: 0,
            },
            group_override: false,
            reward_automation: { rules: [] },
            tribute_preferences: {
              auto_activate: false,
              warning_threshold_secs: 300,
              preferred_tributes: [],
            },
            tribute_status: {
              active: false,
              remaining_secs: 0,
              point_balance: 0,
              active_tributes: [],
              alert_state: "expired",
            },
          },
        ])
      )
      .mockResolvedValueOnce(
        jsonResponse({
          updated: true,
        })
      )
      .mockResolvedValueOnce(
        jsonResponse([
          {
            character_name: "Alpha",
            class: "Wizard",
            role: "DPS",
            heal_at_pct: 60,
            mana_sit_pct: 25,
            nuke_at_pct: 90,
            rotation: [],
            class_params: {
              cross_client_heal_enabled: true,
              cross_client_heal_threshold_pct: 75,
              cross_client_heal_priority: 11,
              cross_client_claim_timeout_ms: 4000,
            },
            auto_rez: {
              enabled: true,
              min_xp_pct: 96,
              trusted_casters: ["Frostreaver"],
              decline_if_untrusted: true,
              delay_ms: 5100,
            },
            group_override: true,
            reward_automation: {
              rules: [
                {
                  task_matcher: "*",
                  preference: { kind: "by_name", reward_name: "Ancient Coin" },
                },
              ],
            },
            window_title_format: "[{server}] Alpha (60 WIZ)",
            tribute_preferences: {
              auto_activate: true,
              warning_threshold_secs: 180,
              preferred_tributes: ["Arcane Fury", "Hero's Fortitude"],
            },
            tribute_status: {
              active: false,
              remaining_secs: 0,
              point_balance: 0,
              active_tributes: [],
              alert_state: "expired",
            },
          },
        ])
      );

    const { result } = renderHook(() => useCharacterConfigs());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.saveConfig({
        character_name: "Alpha",
        class: "Wizard",
        role: "DPS",
        heal_at_pct: 60,
        mana_sit_pct: 25,
        nuke_at_pct: 90,
        rotation: [],
        class_params: {
          cross_client_heal_enabled: true,
          cross_client_heal_threshold_pct: 75,
          cross_client_heal_priority: 11,
          cross_client_claim_timeout_ms: 4000,
        },
        auto_rez: {
          enabled: true,
          min_xp_pct: 96,
          trusted_casters: ["Frostreaver"],
          decline_if_untrusted: true,
          delay_ms: 5100,
        },
        group_override: true,
        reward_automation: {
          rules: [
            {
              task_matcher: "*",
              preference: { kind: "by_name", reward_name: "Ancient Coin" },
            },
          ],
        },
        window_title_format: "[{server}] Alpha (60 WIZ)",
        tribute_preferences: {
          auto_activate: true,
          warning_threshold_secs: 180,
          preferred_tributes: ["Arcane Fury", "Hero's Fortitude"],
        },
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/config/characters/Alpha",
      expect.objectContaining({ method: "PUT" })
    );
    const [, requestInit] = fetchMock.mock.calls[1]!;
    expect(JSON.parse(String(requestInit?.body))).toMatchObject({
      auto_rez: {
        enabled: true,
        min_xp_pct: 96,
      },
      reward_automation: {
        rules: [
          {
            task_matcher: "*",
            preference: { kind: "by_name", reward_name: "Ancient Coin" },
          },
        ],
      },
      window_title_format: "[{server}] Alpha (60 WIZ)",
      tribute_preferences: {
        warning_threshold_secs: 180,
        preferred_tributes: ["Arcane Fury", "Hero's Fortitude"],
      },
    });
    expect(result.current.configs[0].heal_at_pct).toBe(60);
    expect(result.current.configs[0].class_params.cross_client_claim_timeout_ms).toBe(4000);
    expect(result.current.configs[0].reward_automation?.rules).toHaveLength(1);
    expect(result.current.configs[0].window_title_format).toBe(
      "[{server}] Alpha (60 WIZ)"
    );
    expect(result.current.configs[0].tribute_preferences.warning_threshold_secs).toBe(180);
    expect(result.current.error).toBeNull();
  });

  it("normalizes optional fields before save", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse([
          {
            character_name: "Alpha",
            class: "Wizard",
            role: "DPS",
            heal_at_pct: 50,
            mana_sit_pct: 20,
            nuke_at_pct: 80,
            rotation: [],
            class_params: {},
            group_override: false,
          },
        ])
      )
      .mockResolvedValueOnce(jsonResponse({ updated: true }))
      .mockResolvedValueOnce(jsonResponse([]));

    const { result } = renderHook(() => useCharacterConfigs());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.saveConfig({
        character_name: "Alpha",
        class: "Wizard",
        role: "DPS",
        heal_at_pct: 55,
        mana_sit_pct: 25,
        nuke_at_pct: 90,
        rotation: [],
        class_params: {},
        group_override: false,
      });
    });

    const [, requestInit] = fetchMock.mock.calls[1]!;
    expect(JSON.parse(String(requestInit?.body))).toMatchObject({
      auto_rez: {
        enabled: false,
        min_xp_pct: 90,
        trusted_casters: [],
        decline_if_untrusted: false,
        delay_ms: 3000,
      },
      reward_automation: { rules: [] },
      window_title_format: "[{server}] {character} ({level} {class_short})",
      tribute_preferences: {
        auto_activate: false,
        warning_threshold_secs: 300,
        preferred_tributes: [],
      },
    });
  });
});
