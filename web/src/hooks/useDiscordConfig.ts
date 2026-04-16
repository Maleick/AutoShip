import { useCallback, useEffect, useState } from "react";

import type { DiscordSettings } from "../types";

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function createDefaultDiscordSettings(): DiscordSettings {
  return {
    webhook_url: "",
    channels: {
      kills: "",
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
      hvt: {
        enabled: true,
        webhook_url: "",
        level: "CRITICAL",
        message_mode: "rich_embed",
        mention_policy: "none",
      },
      crash: {
        enabled: true,
        webhook_url: "",
        level: "CRITICAL",
        message_mode: "rich_embed",
        mention_policy: "none",
      },
      mass_failure: {
        enabled: true,
        webhook_url: "",
        level: "CRITICAL",
        message_mode: "rich_embed",
        mention_policy: "none",
      },
    },
  };
}

function normalizeDiscordSettings(settings: DiscordSettings): DiscordSettings {
  const defaults = createDefaultDiscordSettings();
  const channels = isObjectRecord(settings.channels) ? settings.channels : {};
  const notificationRoutes = isObjectRecord(settings.notification_routes)
    ? settings.notification_routes
    : {};

  return {
    webhook_url: settings.webhook_url ?? defaults.webhook_url,
    channels: Object.fromEntries(
      Object.keys(defaults.channels).map((key) => [
        key,
        typeof channels[key] === "string" ? channels[key] : defaults.channels[key],
      ]),
    ),
    notification_routes: Object.fromEntries(
      Object.entries(defaults.notification_routes).map(([key, route]) => [
        key,
        {
          ...route,
          ...(isObjectRecord(notificationRoutes[key]) ? notificationRoutes[key] : {}),
        },
      ]),
    ) as DiscordSettings["notification_routes"],
  };
}

export function useDiscordConfig() {
  const [config, setConfig] = useState<DiscordSettings>(createDefaultDiscordSettings);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchConfig = useCallback(async () => {
    try {
      const res = await fetch("/api/config/discord");
      if (res.status === 404 || res.status === 501) {
        setConfig(createDefaultDiscordSettings());
        setError("Discord config backend unavailable (demo mode)");
        return;
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: DiscordSettings = await res.json();
      setConfig(normalizeDiscordSettings(data));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to fetch Discord config");
    } finally {
      setLoading(false);
    }
  }, []);

  const saveConfig = useCallback(
    async (updated: DiscordSettings): Promise<DiscordSettings | null> => {
      setSaving(true);
      try {
        const res = await fetch("/api/config/discord", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(updated),
        });
        if (res.status === 404 || res.status === 501) {
          setConfig(normalizeDiscordSettings(updated));
          setError("Discord config backend unavailable (demo mode)");
          return normalizeDiscordSettings(updated);
        }
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const stored: DiscordSettings = await res.json();
        const normalized = normalizeDiscordSettings(stored);
        setConfig(normalized);
        setError(null);
        return normalized;
      } catch (e) {
        setError(e instanceof Error ? e.message : "Failed to save Discord config");
        return null;
      } finally {
        setSaving(false);
      }
    },
    [],
  );

  useEffect(() => {
    void fetchConfig();
  }, [fetchConfig]);

  return { config, loading, saving, error, saveConfig, refresh: fetchConfig };
}
