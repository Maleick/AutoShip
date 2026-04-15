import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useBoxChatSettings } from "./useBoxChatSettings";
import { jsonResponse } from "../test/http";

describe("useBoxChatSettings", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("loads and saves box-chat settings through the API", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse({
          enabled: false,
          host: "127.0.0.1",
          port: 2112,
          auto_connect: false,
        })
      )
      .mockResolvedValueOnce(
        jsonResponse({
          enabled: true,
          host: "192.168.1.25",
          port: 3002,
          auto_connect: true,
        })
      );

    const { result } = renderHook(() => useBoxChatSettings());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.settings.host).toBe("127.0.0.1");

    await act(async () => {
      await result.current.save({
        enabled: true,
        host: "192.168.1.25",
        port: 3002,
        auto_connect: true,
      });
    });

    expect(result.current.settings.host).toBe("192.168.1.25");
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/box-chat/settings",
      expect.objectContaining({
        method: "PUT",
        headers: { "Content-Type": "application/json" },
      })
    );
  });
});
