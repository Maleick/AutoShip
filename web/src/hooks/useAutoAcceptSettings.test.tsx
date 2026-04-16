import { renderHook, waitFor, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAutoAcceptSettings } from "./useAutoAcceptSettings";
import { jsonResponse } from "../test/http";

describe("useAutoAcceptSettings", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("uses demo mode on backend unavailability", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(new Response("", { status: 501 }));

    const { result } = renderHook(() => useAutoAcceptSettings());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBe(
      "Auto-accept backend unavailable (demo mode)",
    );
  });

  it("loads and saves auto-accept settings", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse({
          enabled: true,
          accept_group_invites: true,
          accept_trades: true,
          accept_task_adds: true,
          accept_dz_adds: true,
          accept_translocates: true,
          accept_anchors: false,
          trust_mode: "anyone",
          trusted_players: [],
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          enabled: true,
          accept_group_invites: true,
          accept_trades: false,
          accept_task_adds: true,
          accept_dz_adds: true,
          accept_translocates: true,
          accept_anchors: true,
          trust_mode: "trust_list",
          trusted_players: ["Leaderone", "Clericone"],
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          enabled: true,
          accept_group_invites: true,
          accept_trades: false,
          accept_task_adds: true,
          accept_dz_adds: true,
          accept_translocates: true,
          accept_anchors: true,
          trust_mode: "trust_list",
          trusted_players: ["Leaderone", "Clericone"],
        }),
      );

    const { result } = renderHook(() => useAutoAcceptSettings());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.saveSettings({
        enabled: true,
        accept_group_invites: true,
        accept_trades: false,
        accept_task_adds: true,
        accept_dz_adds: true,
        accept_translocates: true,
        accept_anchors: true,
        trust_mode: "trust_list",
        trusted_players: ["Leaderone", "Clericone"],
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/config/auto-accept",
      expect.objectContaining({ method: "PUT" }),
    );
    expect(result.current.settings.accept_trades).toBe(false);
    expect(result.current.settings.trust_mode).toBe("trust_list");
    expect(result.current.error).toBeNull();
  });
});
