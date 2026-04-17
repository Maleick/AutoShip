import { useCallback, useEffect, useMemo, useState } from "react";

import type {
  VendorWatchAlertEntry,
  VendorWatchAlertPage,
  VendorWatchConfig,
  VendorWatchStats,
} from "../types";
import { useWebSocket } from "./useWebSocket";

const defaultConfig: VendorWatchConfig = {
  enabled: true,
  items: [],
};

const defaultStats: VendorWatchStats = {
  total_alerts: 0,
  watched_items: 0,
  budget_hits: 0,
};

function getWebSocketUrl(): string {
  if (typeof window === "undefined") {
    return "ws://localhost/ws";
  }

  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/ws`;
}

async function readErrorMessage(response: Response): Promise<string> {
  try {
    const payload = (await response.json()) as { error?: string };
    if (typeof payload.error === "string" && payload.error.trim()) {
      return payload.error;
    }
  } catch {
    // Ignore JSON parse failures and fall back to the HTTP status.
  }

  return `HTTP ${response.status}`;
}

export function useVendorWatch() {
  const [alerts, setAlerts] = useState<VendorWatchAlertEntry[]>([]);
  const [config, setConfig] = useState<VendorWatchConfig>(defaultConfig);
  const [stats, setStats] = useState<VendorWatchStats>(defaultStats);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const wsUrl = useMemo(() => getWebSocketUrl(), []);
  const { connected, lastMessage } = useWebSocket(wsUrl);

  const fetchAlerts = useCallback(async () => {
    const response = await fetch("/api/vendor-watch/alerts");
    if (!response.ok) {
      throw new Error(await readErrorMessage(response));
    }
    const page = (await response.json()) as VendorWatchAlertPage;
    setAlerts(page.entries);
  }, []);

  const fetchConfig = useCallback(async () => {
    const response = await fetch("/api/vendor-watch/config");
    if (!response.ok) {
      throw new Error(await readErrorMessage(response));
    }
    const nextConfig = (await response.json()) as VendorWatchConfig;
    setConfig(nextConfig);
  }, []);

  const fetchStats = useCallback(async () => {
    const response = await fetch("/api/vendor-watch/stats");
    if (!response.ok) {
      throw new Error(await readErrorMessage(response));
    }
    const nextStats = (await response.json()) as VendorWatchStats;
    setStats(nextStats);
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      await Promise.all([fetchAlerts(), fetchConfig(), fetchStats()]);
      setError(null);
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Failed to load vendor watch state",
      );
    } finally {
      setLoading(false);
    }
  }, [fetchAlerts, fetchConfig, fetchStats]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!lastMessage) {
      return;
    }

    try {
      const event = JSON.parse(lastMessage) as {
        type?: string;
        data?: VendorWatchAlertEntry;
      };
      const alert = event.data;
      if (event.type !== "vendor_watch_alert" || !alert) {
        return;
      }
      setAlerts((current) => [alert, ...current].slice(0, 100));
      setStats((current) => ({
        ...current,
        total_alerts: current.total_alerts + 1,
        budget_hits: current.budget_hits + (alert.within_budget === true ? 1 : 0),
      }));
    } catch {
      // Ignore non-JSON websocket traffic.
    }
  }, [lastMessage]);

  const save = useCallback(async (nextConfig: VendorWatchConfig) => {
    setSaving(true);
    try {
      const response = await fetch("/api/vendor-watch/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(nextConfig),
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const savedConfig = (await response.json()) as VendorWatchConfig;
      setConfig(savedConfig);
      setSavedAt(Date.now());
      setError(null);
      await fetchStats();
      return savedConfig;
    } catch (nextError) {
      const message =
        nextError instanceof Error
          ? nextError.message
          : "Failed to save vendor watch settings";
      setError(message);
      throw new Error(message);
    } finally {
      setSaving(false);
    }
  }, [fetchStats]);

  const clearAlerts = useCallback(async () => {
    const response = await fetch("/api/vendor-watch/alerts", { method: "DELETE" });
    if (!response.ok && response.status !== 204) {
      throw new Error(await readErrorMessage(response));
    }
    setAlerts([]);
    await fetchStats();
  }, [fetchStats]);

  return {
    alerts,
    config,
    stats,
    loading,
    saving,
    error,
    savedAt,
    connected,
    refresh,
    save,
    clearAlerts,
  };
}
