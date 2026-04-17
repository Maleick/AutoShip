import { useCallback, useEffect, useState } from "react";

import type {
  LiveTradeskillTrophyStatus,
  TradeskillTrophySettings,
} from "../types";

type ErrorPayload = {
  error?: string;
};

const DEFAULT_SETTINGS: TradeskillTrophySettings = {
  enabled: false,
  trophy_item_name: "",
};

async function readErrorMessage(
  response: Response,
  fallback: string,
): Promise<string> {
  try {
    const payload = (await response.json()) as ErrorPayload;
    if (payload.error) {
      return payload.error;
    }
  } catch {
    // Fall back to the HTTP status message below.
  }
  return `${fallback}: HTTP ${response.status}`;
}

export function useTradeskillTrophySettings() {
  const [settings, setSettings] =
    useState<TradeskillTrophySettings>(DEFAULT_SETTINGS);
  const [statuses, setStatuses] = useState<LiveTradeskillTrophyStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const settingsRes = await fetch("/api/config/tradeskill-trophy");
      if (settingsRes.status === 404 || settingsRes.status === 501) {
        setStatuses([]);
        setError("Tradeskill trophy backend unavailable (demo mode)");
        return;
      }
      if (!settingsRes.ok) {
        throw new Error(`HTTP ${settingsRes.status}`);
      }
      const nextSettings: TradeskillTrophySettings = await settingsRes.json();
      setSettings(nextSettings);

      const statusRes = await fetch("/api/tradeskill-trophy/status");
      if (statusRes.ok) {
        const nextStatuses: LiveTradeskillTrophyStatus[] = await statusRes.json();
        setStatuses(nextStatuses);
      } else {
        setStatuses([]);
      }

      setError(null);
    } catch (e) {
      setError(
        e instanceof Error
          ? `Network error fetching tradeskill trophy settings: ${e.message}`
          : "Failed to fetch tradeskill trophy settings",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  const saveSettings = useCallback(
    async (updated: TradeskillTrophySettings): Promise<boolean> => {
      setSaving(true);
      try {
        const res = await fetch("/api/config/tradeskill-trophy", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(updated),
        });
        if (res.status === 404 || res.status === 501) {
          setSettings(updated);
          setStatuses([]);
          setError("Tradeskill trophy backend unavailable (demo mode)");
          return true;
        }
        if (!res.ok) {
          throw new Error(
            await readErrorMessage(res, "Failed to save tradeskill trophy settings"),
          );
        }
        setError(null);
        await refresh();
        return true;
      } catch (e) {
        setError(
          e instanceof Error
            ? e.message
            : "Failed to save tradeskill trophy settings",
        );
        return false;
      } finally {
        setSaving(false);
      }
    },
    [refresh],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    settings,
    statuses,
    setSettings,
    loading,
    saving,
    error,
    saveSettings,
    refresh,
  };
}
