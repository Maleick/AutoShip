import { useCallback, useEffect, useState } from "react";
import type {
  SayDetectionConfig,
  SayDetectionStatus,
} from "../types";

const defaultConfig: SayDetectionConfig = {
  enabled: false,
  soundEnabled: true,
  soundFile: "say_alert.wav",
  toastEnabled: true,
  discordWebhookUrl: null,
  broadcastAllClients: false,
  rules: [],
};

export function useSayDetection() {
  const [config, setConfig] = useState<SayDetectionConfig>(defaultConfig);
  const [status, setStatus] = useState<SayDetectionStatus>({
    config: defaultConfig,
    totalMatches: 0,
    lastMatch: null,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch("/api/say-detection/status");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }

      const nextStatus: SayDetectionStatus = await res.json();
      setStatus(nextStatus);
      setConfig(nextStatus.config);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Failed to load say-detection status",
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

  const save = useCallback(
    async (next: SayDetectionConfig): Promise<SayDetectionConfig> => {
      setSaving(true);
      try {
        const res = await fetch("/api/say-detection/config", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(next),
        });
        if (!res.ok) {
          const body = await res.json().catch(() => ({}));
          throw new Error(body.error ?? `HTTP ${res.status}`);
        }

        const saved: SayDetectionConfig = await res.json();
        setConfig(saved);
        setStatus((prev) => ({ ...prev, config: saved }));
        setSavedAt(Date.now());
        setError(null);
        return saved;
      } catch (e) {
        const message =
          e instanceof Error ? e.message : "Failed to save say-detection settings";
        setError(message);
        throw new Error(message);
      } finally {
        setSaving(false);
      }
    },
    [],
  );

  return {
    config,
    status,
    loading,
    saving,
    error,
    savedAt,
    refresh,
    save,
  };
}
