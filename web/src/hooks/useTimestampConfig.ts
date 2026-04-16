import { useCallback, useEffect, useState } from "react";
import type { TimestampConfig, TimestampFormat } from "../types";

const defaultConfig: TimestampConfig = {
  enabled: false,
  format: "date_time_24",
};

export function useTimestampConfig(character: string) {
  const [config, setConfig] = useState<TimestampConfig>(defaultConfig);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch(`/api/timestamp-config/${encodeURIComponent(character)}`);
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const next: TimestampConfig = await res.json();
      setConfig(next);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Failed to load timestamp config"
      );
    } finally {
      setLoading(false);
    }
  }, [character]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const save = useCallback(async (next: TimestampConfig): Promise<TimestampConfig> => {
    setSaving(true);
    try {
      const res = await fetch(`/api/timestamp-config/${encodeURIComponent(character)}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(next),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const saved: TimestampConfig = await res.json();
      setConfig(saved);
      setSavedAt(Date.now());
      setError(null);
      return saved;
    } catch (e) {
      const message =
        e instanceof Error ? e.message : "Failed to save timestamp config";
      setError(message);
      throw new Error(message);
    } finally {
      setSaving(false);
    }
  }, [character]);

  const setFormat = useCallback(async (format: TimestampFormat) => {
    await save({ ...config, format });
  }, [config, save]);

  const toggle = useCallback(async () => {
    await save({ ...config, enabled: !config.enabled });
  }, [config, save]);

  return {
    config,
    loading,
    saving,
    error,
    savedAt,
    refresh,
    save,
    setFormat,
    toggle,
  };
}
