import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { jsonResponse } from "../test/http";
import { createDefaultDiscordSettings, useDiscordConfig } from "./useDiscordConfig";

describe("useDiscordConfig", () => {
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

    const { result } = renderHook(() => useDiscordConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBe("Discord config backend unavailable (demo mode)");
    expect(result.current.config).toEqual(createDefaultDiscordSettings());

    const updated = {
      ...createDefaultDiscordSettings(),
      webhook_url: "https://discord.example.com/default",
    };

    await act(async () => {
      const saved = await result.current.saveConfig(updated);
      expect(saved).toEqual(updated);
    });

    expect(result.current.config.webhook_url).toBe("https://discord.example.com/default");
    expect(result.current.error).toBe("Discord config backend unavailable (demo mode)");
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/config/discord",
      expect.objectContaining({ method: "PUT" }),
    );
  });

  it("loads and saves a real discord config", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse({
          webhook_url: "https://discord.example.com/default",
          channels: {
            kills: "https://discord.example.com/kills",
            loot: "",
            timers: "",
            feats: "",
            status: "",
          },
          notification_routes: {
            death: {
              enabled: true,
              webhook_url: "",
              level: "CRITICAL",
              message_mode: "rich_embed",
              mention_policy: "everyone",
            },
            status: {
              enabled: true,
              webhook_url: "",
              level: "INFO",
              message_mode: "rich_embed",
              mention_policy: "none",
            },
          },
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          webhook_url: "https://discord.example.com/trimmed",
          channels: {
            kills: "https://discord.example.com/kills",
            loot: "https://discord.example.com/loot",
            timers: "",
            feats: "",
            status: "",
          },
          notification_routes: {
            death: {
              enabled: true,
              webhook_url: "https://discord.example.com/death",
              level: "CRITICAL",
              message_mode: "plain_text",
              mention_policy: "everyone",
            },
            status: {
              enabled: true,
              webhook_url: "",
              level: "INFO",
              message_mode: "rich_embed",
              mention_policy: "none",
            },
          },
        }),
      );

    const { result } = renderHook(() => useDiscordConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.config.channels.kills).toBe("https://discord.example.com/kills");

    await act(async () => {
      const saved = await result.current.saveConfig({
        ...result.current.config,
        webhook_url: " https://discord.example.com/trimmed ",
        channels: {
          ...result.current.config.channels,
          loot: " https://discord.example.com/loot ",
        },
        notification_routes: {
          ...result.current.config.notification_routes,
          death: {
            ...result.current.config.notification_routes.death,
            webhook_url: " https://discord.example.com/death ",
            message_mode: "plain_text",
          },
        },
      });

      expect(saved?.webhook_url).toBe("https://discord.example.com/trimmed");
    });

    expect(result.current.config.webhook_url).toBe("https://discord.example.com/trimmed");
    expect(result.current.config.channels.loot).toBe("https://discord.example.com/loot");
    expect(result.current.config.notification_routes.death.webhook_url).toBe(
      "https://discord.example.com/death",
    );
    expect(result.current.config.notification_routes.death.message_mode).toBe("plain_text");
    expect(result.current.error).toBeNull();
  });

  it("normalizes partial payloads and drops unknown route keys", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        webhook_url: "https://discord.example.com/default",
        notification_routes: {
          death: {
            enabled: true,
            webhook_url: "https://discord.example.com/death",
            level: "CRITICAL",
            message_mode: "plain_text",
            mention_policy: "everyone",
          },
          mystery: {
            enabled: true,
            webhook_url: "https://discord.example.com/mystery",
            level: "WARNING",
            message_mode: "plain_text",
            mention_policy: "none",
          },
        },
      }),
    );

    const { result } = renderHook(() => useDiscordConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.config.channels).toEqual(createDefaultDiscordSettings().channels);
    expect(result.current.config.notification_routes).not.toHaveProperty("mystery");
    expect(result.current.config.notification_routes.death.webhook_url).toBe(
      "https://discord.example.com/death",
    );
    expect(result.current.config.notification_routes.status.level).toBe("INFO");
  });
});
