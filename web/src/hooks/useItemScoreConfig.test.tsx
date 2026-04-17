import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

import { useItemScoreConfig } from "./useItemScoreConfig";

const fetchMock = vi.fn();
global.fetch = fetchMock;

describe("useItemScoreConfig", () => {
  beforeEach(() => {
    fetchMock.mockReset();
  });

  it("loads item score config on mount", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          min_upgrade_delta: 1.75,
          class_weights: {
            Warrior: { STR: 1.2, AC: 0.9 },
            Cleric: { WIS: 1.4, MANA: 0.6 },
          },
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    const { result } = renderHook(() => useItemScoreConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.config.min_upgrade_delta).toBe(1.75);
    expect(result.current.config.class_weights.Warrior?.STR).toBe(1.2);
    expect(result.current.error).toBeNull();
    expect(result.current.loaded).toBe(true);
  });

  it("saves updated item score config", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          min_upgrade_delta: 0,
          class_weights: {
            Rogue: { DEX: 1.1, STR: 0.4 },
          },
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }));

    const { result } = renderHook(() => useItemScoreConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.save({
        min_upgrade_delta: 2.25,
        class_weights: {
          Rogue: { DEX: 1.5, STR: 0.7 },
        },
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/loot/item-score",
      expect.objectContaining({
        method: "PUT",
        headers: { "Content-Type": "application/json" },
      }),
    );
    expect(result.current.config.min_upgrade_delta).toBe(2.25);
    expect(result.current.config.class_weights.Rogue?.DEX).toBe(1.5);
    expect(result.current.savedAt).not.toBeNull();
    expect(result.current.loaded).toBe(true);
  });

  it("does not mark the config as loaded after the initial fetch fails", async () => {
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 500 }));

    const { result } = renderHook(() => useItemScoreConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBe("HTTP 500");
  });
});
