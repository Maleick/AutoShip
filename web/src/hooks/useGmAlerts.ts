import { useCallback, useEffect, useState } from "react";
import type { GmAlertConfig, GmAlertStatus } from "../types";

const defaultConfig: GmAlertConfig = {
  enabled: true,
  soundEnabled: true,
  soundFile: "gm_alert.wav",
  toastEnabled: true,
  autoPauseEnabled: false,
  discordWebhookUrl: null,
  broadcastAllClients: true,
};

export function useGmAlerts() {
  const [config, setConfig] = useState<GmAlertConfig>(defaultConfig);
  const [presence, setPresence] = useState({
    isGmInZone: false,
    gmCount: 0,
    gmNames: [] as string[],
  });
  const [automationPaused, setAutomationPaused] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch("/api/gm-alerts/status");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const status: GmAlertStatus = await res.json();
      setConfig(status.config);
      setPresence(status.presence);
      setAutomationPaused(status.automationPaused);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Failed to load GM alert status"
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  }, [refresh]);

  const save = useCallback(async (next: GmAlertConfig): Promise<GmAlertConfig> => {
    setSaving(true);
    try {
      const res = await fetch("/api/gm-alerts/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(next),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const saved: GmAlertConfig = await res.json();
      setConfig(saved);
      setSavedAt(Date.now());
      setError(null);
      return saved;
    } catch (e) {
      const message =
        e instanceof Error ? e.message : "Failed to save GM alert settings";
      setError(message);
      throw new Error(message);
    } finally {
      setSaving(false);
    }
  }, []);

  return {
    config,
    presence,
    automationPaused,
    loading,
    saving,
    error,
    savedAt,
    refresh,
    save,
  };
}
