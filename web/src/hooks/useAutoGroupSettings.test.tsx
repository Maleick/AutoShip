import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { jsonResponse } from "../test/http";
import { useAutoGroupSettings } from "./useAutoGroupSettings";

describe("useAutoGroupSettings", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("surfaces demo mode when the backend is unavailable", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(new Response("", { status: 501 }));

    const { result } = renderHook(() => useAutoGroupSettings());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBe("Auto-group backend unavailable (demo mode)");
  });

  it("loads and saves auto-group settings", async () => {
    const fetchMock = vi.mocked(fetch);
    const initial = {
      groups: [
        {
          name: "Fire Team",
          leader_name: "Alpha",
          enabled: true,
          invite_retry_interval_secs: 5,
          max_invite_retries: 3,
          completion_command: "/say ready",
          members: [
            { name: "Alpha", role: "none" },
            { name: "Bravo", role: "main_tank" },
          ],
        },
      ],
    };
    const updated = {
      groups: [
        {
          name: "Fire Team",
          leader_name: "Alpha",
          enabled: true,
          invite_retry_interval_secs: 6,
          max_invite_retries: 4,
          completion_command: "/say formed",
          members: [
            { name: "Alpha", role: "none" },
            { name: "Bravo", role: "main_tank" },
            { name: "Charlie", role: "main_assist" },
          ],
        },
      ],
    };
    fetchMock
      .mockResolvedValueOnce(jsonResponse(initial))
      .mockResolvedValueOnce(jsonResponse(updated));

    const { result } = renderHook(() => useAutoGroupSettings());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.saveSettings(updated);
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/config/auto-group",
      expect.objectContaining({ method: "PUT" }),
    );
    expect(result.current.settings).toEqual(updated);
    expect(result.current.error).toBeNull();
  });

  it("sets loading true when a manual refresh starts", async () => {
    const fetchMock = vi.mocked(fetch);
    let resolveRefresh: ((value: Response) => void) | undefined;

    fetchMock
      .mockResolvedValueOnce(new Response("", { status: 501 }))
      .mockImplementationOnce(
        () =>
          new Promise<Response>((resolve) => {
            resolveRefresh = resolve;
          }),
      );

    const { result } = renderHook(() => useAutoGroupSettings());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBe("Auto-group backend unavailable (demo mode)");

    act(() => {
      void result.current.refresh();
    });

    expect(result.current.loading).toBe(true);
    expect(result.current.error).toBeNull();

    await act(async () => {
      resolveRefresh?.(jsonResponse({ groups: [] }));
    });

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeNull();
  });
});
