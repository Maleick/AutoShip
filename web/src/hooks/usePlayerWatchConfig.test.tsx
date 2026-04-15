import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { usePlayerWatchConfig } from "./usePlayerWatchConfig";

const mockFetch = vi.fn();
global.fetch = mockFetch;

describe("usePlayerWatchConfig", () => {
  beforeEach(() => {
    mockFetch.mockReset();
  });

  it("loads config on mount", async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        filter_mode: "all",
        sound_on_zone_in: false,
        friends: ["FriendOne"],
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );

    const { result } = renderHook(() => usePlayerWatchConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.config.filter_mode).toBe("all");
    expect(result.current.config.friends).toContain("FriendOne");
  });

  it("sets filter mode", async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        filter_mode: "all",
        sound_on_zone_in: false,
        friends: [],
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );

    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        filter_mode: "strangers_only",
        sound_on_zone_in: false,
        friends: [],
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );

    const { result } = renderHook(() => usePlayerWatchConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.setFilterMode("strangers_only");
    });

    expect(mockFetch).toHaveBeenCalledWith(
      "/api/config/player-watch",
      expect.objectContaining({ method: "PUT" })
    );
  });

  it("toggles sound alert", async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        filter_mode: "all",
        sound_on_zone_in: false,
        friends: [],
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );

    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        filter_mode: "all",
        sound_on_zone_in: true,
        friends: [],
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );

    const { result } = renderHook(() => usePlayerWatchConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.setSoundOnZoneIn(true);
    });

    expect(result.current.config.sound_on_zone_in).toBe(true);
  });

  it("adds and removes friends", async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        filter_mode: "all",
        sound_on_zone_in: false,
        friends: [],
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );

    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        filter_mode: "all",
        sound_on_zone_in: false,
        friends: ["NewFriend"],
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );

    const { result } = renderHook(() => usePlayerWatchConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.addFriend("NewFriend");
    });

    expect(result.current.config.friends).toContain("NewFriend");

    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({
        filter_mode: "all",
        sound_on_zone_in: false,
        friends: [],
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );

    await act(async () => {
      await result.current.removeFriend("NewFriend");
    });

    expect(result.current.config.friends).not.toContain("NewFriend");
  });

  it("handles fetch error", async () => {
    mockFetch.mockResolvedValueOnce(
      new Response(null, { status: 500 })
    );

    const { result } = renderHook(() => usePlayerWatchConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeTruthy();
  });
});
