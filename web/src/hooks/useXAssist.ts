import { useState, useEffect, useCallback } from "react";
import type { XAssistCharacterConfig, XAssistConfigUpdate } from "../types";

export function useXAssist() {
  const [configs, setConfigs] = useState<XAssistCharacterConfig[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchConfigs = useCallback(async () => {
    try {
      const res = await fetch("/api/xassist/configs");
      if (res.status === 404 || res.status === 501) {
        setError("XAssist backend unavailable (demo mode)");
        setLoading(false);
        return;
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: XAssistCharacterConfig[] = await res.json();
      setConfigs(data);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to fetch XAssist configs");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchConfigs();
  }, [fetchConfigs]);

  const updateConfig = useCallback(
    async (characterName: string, update: XAssistConfigUpdate): Promise<void> => {
      setSaving(true);
      try {
        const res = await fetch(`/api/xassist/config/${encodeURIComponent(characterName)}`, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(update),
        });
        if (!res.ok) {
          const body = await res.json().catch(() => ({}));
          throw new Error(body.error ?? `HTTP ${res.status}`);
        }
        const updated: XAssistCharacterConfig = await res.json();
        setConfigs((prev) => {
          const idx = prev.findIndex((c) => c.character_name === characterName);
          if (idx >= 0) {
            const next = [...prev];
            next[idx] = updated;
            return next;
          }
          return [...prev, updated].sort((a, b) =>
            a.character_name.localeCompare(b.character_name)
          );
        });
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Failed to update XAssist config");
        throw e;
      } finally {
        setSaving(false);
      }
    },
    []
  );

  const deleteConfig = useCallback(async (characterName: string): Promise<void> => {
    setSaving(true);
    try {
      const res = await fetch(`/api/xassist/config/${encodeURIComponent(characterName)}`, {
        method: "DELETE",
      });
      if (!res.ok && res.status !== 404) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      setConfigs((prev) => prev.filter((c) => c.character_name !== characterName));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to delete XAssist config");
      throw e;
    } finally {
      setSaving(false);
    }
  }, []);

  return {
    configs,
    loading,
    saving,
    error,
    refresh: fetchConfigs,
    updateConfig,
    deleteConfig,
  };
}
