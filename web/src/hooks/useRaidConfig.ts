import { useState, useEffect, useCallback } from "react";
import type { RaidConfig } from "../types";

const DEFAULT_CONFIG: RaidConfig = {
  main_assist: null,
  main_tank: null,
  ch_chain: [],
  pull_target: null,
  camp_position: null,
  behavior_mode: "camp",
  members: [],
};

export function useRaidConfig() {
  const [config, setConfig] = useState<RaidConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchConfig = useCallback(async () => {
    try {
      const res = await fetch("/api/raid/config");
      if (res.status === 404 || res.status === 501) {
        setError("Raid config backend unavailable (demo mode)");
        return;
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: RaidConfig = await res.json();
      setConfig(data);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to fetch raid config");
    } finally {
      setLoading(false);
    }
  }, []);

  const saveConfig = useCallback(async (updated: RaidConfig) => {
    setSaving(true);
    try {
      const res = await fetch("/api/raid/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(updated),
      });
      if (res.status === 404 || res.status === 501) {
        setConfig(updated);
        setError("Raid config backend unavailable (demo mode)");
        return;
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setConfig(updated);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to save raid config");
    } finally {
      setSaving(false);
    }
  }, []);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  return { config, setConfig, loading, saving, error, saveConfig, refresh: fetchConfig };
}
