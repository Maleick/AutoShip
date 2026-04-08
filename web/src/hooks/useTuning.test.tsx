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
            class_params: {},
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
            class_params: {},
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
        class_params: {},
        group_override: true,
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/config/characters/Alpha",
      expect.objectContaining({ method: "PUT" })
    );
    expect(result.current.configs[0].heal_at_pct).toBe(60);
    expect(result.current.error).toBeNull();
  });
});
