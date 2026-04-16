import { useCallback, useEffect, useState } from "react";
import type { ChatLogSettings, LogRotation } from "../types";

const defaultSettings: ChatLogSettings = {
  enabled: false,
  rotation: { type: "daily" },
  level: "info",
  channels: [],
};

export function useChatLogSettings() {
  const [settings, setSettings] = useState<ChatLogSettings>(defaultSettings);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch("/api/chat-log/settings");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const next: ChatLogSettings = await res.json();
      setSettings(next);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Failed to load chat log settings"
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const save = useCallback(async (next: ChatLogSettings): Promise<ChatLogSettings> => {
    setSaving(true);
    try {
      const res = await fetch("/api/chat-log/settings", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(next),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const saved: ChatLogSettings = await res.json();
      setSettings(saved);
      setSavedAt(Date.now());
      setError(null);
      return saved;
    } catch (e) {
      const message =
        e instanceof Error ? e.message : "Failed to save chat log settings";
      setError(message);
      throw new Error(message);
    } finally {
      setSaving(false);
    }
  }, []);

  const updateRotation = useCallback((rotation: LogRotation) => {
    setSettings((prev) => ({ ...prev, rotation }));
  }, []);

  const updateChannels = useCallback((channels: ChatLogSettings["channels"]) => {
    setSettings((prev) => ({ ...prev, channels }));
  }, []);

  const toggleChannel = useCallback((channel: ChatLogSettings["channels"][number]) => {
    setSettings((prev) => {
      const has = prev.channels.includes(channel);
      return {
        ...prev,
        channels: has
          ? prev.channels.filter((c) => c !== channel)
          : [...prev.channels, channel],
      };
    });
  }, []);

  return {
    settings,
    loading,
    saving,
    error,
    savedAt,
    refresh,
    save,
    updateRotation,
    updateChannels,
    toggleChannel,
  };
}
