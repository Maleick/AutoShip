import { renderHook, waitFor, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useTradeskillTrophySettings } from "./useTradeskillTrophySettings";
import { jsonResponse } from "../test/http";

describe("useTradeskillTrophySettings", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("uses demo mode on backend unavailability", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(new Response("", { status: 501 }));

    const { result } = renderHook(() => useTradeskillTrophySettings());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBe(
      "Tradeskill trophy backend unavailable (demo mode)",
    );
    expect(result.current.statuses).toEqual([]);
  });

  it("loads settings, live status, and saves updates", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse({
          enabled: true,
          trophy_item_name: "Geerlok Automated Hammer",
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse([
          {
            pid: 4242,
            status: {
              active: true,
              equipped_by_manager: true,
              open_container_name: "Forge",
              container_type: "blacksmithing",
              target_slot: "ammo",
              previous_item_name: "Fine Steel Spear",
              charges_remaining: 7,
            },
          },
        ]),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          enabled: true,
          trophy_item_name: "Geerlok Sewing Contraption",
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          enabled: true,
          trophy_item_name: "Geerlok Sewing Contraption",
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse([
          {
            pid: 4242,
            status: {
              active: false,
              equipped_by_manager: false,
              open_container_name: null,
              container_type: null,
              target_slot: null,
              previous_item_name: null,
              charges_remaining: 6,
            },
          },
        ]),
      );

    const { result } = renderHook(() => useTradeskillTrophySettings());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.statuses).toHaveLength(1);
    expect(result.current.statuses[0]?.status.charges_remaining).toBe(7);

    await act(async () => {
      await result.current.saveSettings({
        enabled: true,
        trophy_item_name: "Geerlok Sewing Contraption",
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "/api/config/tradeskill-trophy",
      expect.objectContaining({ method: "PUT" }),
    );
    expect(result.current.settings.trophy_item_name).toBe(
      "Geerlok Sewing Contraption",
    );
    expect(result.current.statuses[0]?.status.charges_remaining).toBe(6);
    expect(result.current.error).toBeNull();
  });
});
