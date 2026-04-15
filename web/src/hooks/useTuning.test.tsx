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
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/config/characters/Alpha",
      expect.objectContaining({ method: "PUT" })
    );
    const [, requestInit] = fetchMock.mock.calls[1]!;
    expect(JSON.parse(String(requestInit?.body))).toMatchObject({
      auto_camp_on_death: {
        enabled: true,
        camp_delay_secs: 45,
        relog_wait_secs: 1800,
      },
    });
    expect(result.current.configs[0].heal_at_pct).toBe(60);
    expect(result.current.configs[0].class_params.cross_client_claim_timeout_ms).toBe(4000);
    expect(result.current.configs[0].auto_camp_on_death.relog_wait_secs).toBe(1800);
    expect(result.current.configs[0].tribute_preferences.warning_threshold_secs).toBe(180);
    expect(result.current.error).toBeNull();
  });
});
