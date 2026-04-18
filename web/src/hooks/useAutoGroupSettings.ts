import { useCallback, useEffect, useState } from "react";

import type { AutoGroupSettings } from "../types";

const DEFAULT_SETTINGS: AutoGroupSettings = {
  groups: [],
};

export function useAutoGroupSettings() {
  const [settings, setSettings] = useState<AutoGroupSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/config/auto-group");
      if (res.status === 404 || res.status === 501) {
        setError("Auto-group backend unavailable (demo mode)");
        setLoading(false);
        return;
      }
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const data: AutoGroupSettings = await res.json();
      setSettings(data);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error
          ? `Failed to fetch auto-group settings: ${e.message}`
          : "Failed to fetch auto-group settings",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  const saveSettings = useCallback(
    async (updated: AutoGroupSettings): Promise<AutoGroupSettings | null> => {
      setSaving(true);
      try {
        const res = await fetch("/api/config/auto-group", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(updated),
        });
        if (!res.ok) {
          const body = await res.json().catch(() => ({}));
          throw new Error(body.error ?? `HTTP ${res.status}`);
        }
        const saved: AutoGroupSettings = await res.json();
        setSettings(saved);
        setSavedAt(Date.now());
        setError(null);
        return saved;
      } catch (e) {
        setError(
          e instanceof Error
            ? `Failed to save auto-group settings: ${e.message}`
            : "Failed to save auto-group settings",
        );
        return null;
      } finally {
        setSaving(false);
      }
    },
    [],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    settings,
    setSettings,
    loading,
    saving,
    error,
    savedAt,
    refresh,
    saveSettings,
  };
}
