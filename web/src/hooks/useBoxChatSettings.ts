import { useCallback, useEffect, useState } from "react";
import type { BoxChatSettings } from "../types";

const defaultSettings: BoxChatSettings = {
  enabled: false,
  host: "127.0.0.1",
  port: 2112,
  auto_connect: false,
};

export function useBoxChatSettings() {
  const [settings, setSettings] = useState<BoxChatSettings>(defaultSettings);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch("/api/box-chat/settings");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const next: BoxChatSettings = await res.json();
      setSettings(next);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Failed to load box-chat settings"
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const save = useCallback(async (next: BoxChatSettings): Promise<BoxChatSettings> => {
    setSaving(true);
    try {
      const res = await fetch("/api/box-chat/settings", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(next),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const saved: BoxChatSettings = await res.json();
      setSettings(saved);
      setSavedAt(Date.now());
      setError(null);
      return saved;
    } catch (e) {
      const message =
        e instanceof Error ? e.message : "Failed to save box-chat settings";
      setError(message);
      throw new Error(message);
    } finally {
      setSaving(false);
    }
  }, []);

  return {
    settings,
    loading,
    saving,
    error,
    savedAt,
    refresh,
    save,
  };
}
