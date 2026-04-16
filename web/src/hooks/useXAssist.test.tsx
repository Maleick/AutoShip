import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useXAssist } from "./useXAssist";
import { jsonResponse } from "../test/http";

describe("useXAssist", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("loads XAssist configs from the API", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(
      jsonResponse([
        { character_name: "BoxDPS1", ma_name: "MainTank", enabled: true },
        { character_name: "BoxDPS2", ma_name: "MainTank", enabled: false },
      ])
    );

    const { result } = renderHook(() => useXAssist());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.configs).toHaveLength(2);
    expect(result.current.configs[0].character_name).toBe("BoxDPS1");
    expect(result.current.error).toBeNull();
  });

  it("handles API 404 as demo mode", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(
      new Response(null, { status: 404 })
    );

    const { result } = renderHook(() => useXAssist());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.error).toBe("XAssist backend unavailable (demo mode)");
    expect(result.current.configs).toHaveLength(0);
  });

  it("updates a character config", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse([
          { character_name: "BoxDPS1", ma_name: "MainTank", enabled: false },
        ])
      )
      .mockResolvedValueOnce(
        jsonResponse({
          character_name: "BoxDPS1",
          ma_name: "NewTank",
          enabled: true,
        })
      );

    const { result } = renderHook(() => useXAssist());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.updateConfig("BoxDPS1", {
        ma_name: "NewTank",
        enabled: true,
      });
    });

    expect(result.current.configs[0].ma_name).toBe("NewTank");
    expect(result.current.configs[0].enabled).toBe(true);
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/xassist/config/BoxDPS1",
      expect.objectContaining({
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ma_name: "NewTank", enabled: true }),
      })
    );
  });

  it("deletes a character config", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse([
          { character_name: "BoxDPS1", ma_name: "MainTank", enabled: true },
          { character_name: "BoxDPS2", ma_name: "MainTank", enabled: false },
        ])
      )
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    const { result } = renderHook(() => useXAssist());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.configs).toHaveLength(2);

    await act(async () => {
      await result.current.deleteConfig("BoxDPS1");
    });

    expect(result.current.configs).toHaveLength(1);
    expect(result.current.configs[0].character_name).toBe("BoxDPS2");
  });

  it("refreshes configs", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse([{ character_name: "BoxDPS1", ma_name: null, enabled: false }])
      )
      .mockResolvedValueOnce(
        jsonResponse([
          { character_name: "BoxDPS1", ma_name: "NewTank", enabled: true },
          { character_name: "BoxDPS2", ma_name: "NewTank", enabled: true },
        ])
      );

    const { result } = renderHook(() => useXAssist());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.configs).toHaveLength(1);

    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.configs).toHaveLength(2);
  });
});
