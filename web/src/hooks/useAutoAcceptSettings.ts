import { useCallback, useEffect, useState } from "react";

import type { AutoAcceptSettings } from "../types";

const DEFAULT_SETTINGS: AutoAcceptSettings = {
  enabled: false,
  accept_group_invites: true,
  accept_trades: true,
  accept_task_adds: true,
  accept_dz_adds: true,
  accept_translocates: true,
  accept_anchors: true,
  trust_mode: "anyone",
  trusted_players: [],
};

export function useAutoAcceptSettings() {
  const [settings, setSettings] = useState<AutoAcceptSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const res = await fetch("/api/config/auto-accept");
      if (res.status === 404 || res.status === 501) {
        setError("Auto-accept backend unavailable (demo mode)");
        return;
      }
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
      const data: AutoAcceptSettings = await res.json();
      setSettings(data);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error
          ? `Network error fetching auto-accept settings: ${e.message}`
          : "Failed to fetch auto-accept settings",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  const saveSettings = useCallback(
    async (updated: AutoAcceptSettings): Promise<boolean> => {
      setSaving(true);
      try {
        const res = await fetch("/api/config/auto-accept", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(updated),
        });
        if (res.status === 404 || res.status === 501) {
          setSettings(updated);
          setError("Auto-accept backend unavailable (demo mode)");
          return true;
        }
        if (!res.ok) {
          throw new Error(`HTTP ${res.status}`);
        }
        setError(null);
        await refresh();
        return true;
      } catch (e) {
        setError(
          e instanceof Error
            ? `Failed to save auto-accept settings: ${e.message}`
            : "Failed to save auto-accept settings",
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

  return { settings, setSettings, loading, saving, error, saveSettings, refresh };
}
