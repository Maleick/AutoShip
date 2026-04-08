import { renderHook, waitFor, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useRaidConfig } from "./useRaidConfig";
import { jsonResponse } from "../test/http";

describe("useRaidConfig", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("keeps demo defaults when the backend is unavailable and saves locally", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(new Response("", { status: 404 }))
      .mockResolvedValueOnce(new Response("", { status: 501 }));

    const { result } = renderHook(() => useRaidConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBe("Raid config backend unavailable (demo mode)");
    expect(result.current.config).toMatchObject({
      main_assist: null,
      main_tank: null,
      ch_chain: [],
      pull_target: null,
      behavior_mode: "camp",
      members: [],
    });

    await act(async () => {
      await result.current.saveConfig({
        main_assist: "Alpha",
        main_tank: "Bravo",
        ch_chain: ["Cleric"],
        pull_target: "Mobs",
        camp_position: { x: 1, y: 2, z: 3 },
        behavior_mode: "hunt",
        members: [],
      });
    });

    expect(result.current.config).toMatchObject({
      main_assist: "Alpha",
      main_tank: "Bravo",
      behavior_mode: "hunt",
    });
    expect(result.current.error).toBe("Raid config backend unavailable (demo mode)");
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/raid/config",
      expect.objectContaining({ method: "PUT" })
    );
  });

  it("loads and saves a real raid config", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse({
          main_assist: "Ma",
          main_tank: "Mt",
          ch_chain: ["C1"],
          pull_target: "Target",
          camp_position: { x: 1, y: 2, z: 3 },
          behavior_mode: "camp",
          members: [],
        })
      )
      .mockResolvedValueOnce(
        jsonResponse({
          main_assist: "Ma2",
          main_tank: "Mt2",
          ch_chain: ["C2"],
          pull_target: "Target2",
          camp_position: null,
          behavior_mode: "hunt",
          members: [],
        })
      );

    const { result } = renderHook(() => useRaidConfig());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.config.main_assist).toBe("Ma");

    await act(async () => {
      await result.current.saveConfig({
        main_assist: "Ma2",
        main_tank: "Mt2",
        ch_chain: ["C2"],
        pull_target: "Target2",
        camp_position: null,
        behavior_mode: "hunt",
        members: [],
      });
    });

    expect(result.current.config.main_assist).toBe("Ma2");
    expect(result.current.error).toBeNull();
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/raid/config",
      expect.objectContaining({ method: "PUT" })
    );
  });
});
