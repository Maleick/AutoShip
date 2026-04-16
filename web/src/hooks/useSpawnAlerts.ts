import { useCallback, useEffect, useState } from "react";
import type {
  SpawnAlertConfig,
  SpawnAlertEntry,
  SpawnAlertStats,
  WatchPattern,
  SpawnAlertPage,
} from "../types";

const defaultConfig: SpawnAlertConfig = {
  watch_named_enabled: true,
  watch_patterns: [],
  broadcast_to_web: true,
  broadcast_to_clients: false,
};

export function useSpawnAlerts() {
  const [alerts, setAlerts] = useState<SpawnAlertEntry[]>([]);
  const [config, setConfig] = useState<SpawnAlertConfig>(defaultConfig);
  const [stats, setStats] = useState<SpawnAlertStats>({ total_alerts: 0, spawns_up: 0, spawns_down: 0 });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [notificationsEnabled, setNotificationsEnabled] = useState(false);

  const requestNotificationPermission = useCallback(async () => {
    if (!("Notification" in window)) {
      console.warn("Browser does not support notifications");
      return false;
    }
    if (Notification.permission === "granted") {
      setNotificationsEnabled(true);
      return true;
    }
    if (Notification.permission !== "denied") {
      const permission = await Notification.requestPermission();
      setNotificationsEnabled(permission === "granted");
      return permission === "granted";
    }
    return false;
  }, []);

  const showBrowserNotification = useCallback((alert: SpawnAlertEntry) => {
    if (!notificationsEnabled || !config.broadcast_to_web) {
      return;
    }
    const title = alert.is_up
      ? `Rare Spawn UP: ${alert.spawn_name}`
      : `Rare Spawn DOWN: ${alert.spawn_name}`;
    const body = `${alert.spawn_name} is ${alert.is_up ? "now" : "no longer"} in ${alert.zone}`;
    try {
      new Notification(title, {
        body,
        icon: alert.is_up ? "/favicon.ico" : "/favicon.ico",
        tag: `spawn-${alert.spawn_name}-${alert.id}`,
      });
    } catch {
      console.warn("Failed to show browser notification");
    }
  }, [notificationsEnabled, config.broadcast_to_web]);

  const fetchAlerts = useCallback(async () => {
    try {
      const res = await fetch("/api/spawn-alerts");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const page: SpawnAlertPage = await res.json();
      setAlerts(page.entries);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load spawn alerts");
    }
  }, []);

  const fetchConfig = useCallback(async () => {
    try {
      const res = await fetch("/api/spawn-alerts/config");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const cfg: SpawnAlertConfig = await res.json();
      setConfig(cfg);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load spawn alert config");
    }
  }, []);

  const fetchStats = useCallback(async () => {
    try {
      const res = await fetch("/api/spawn-alerts/stats");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const st: SpawnAlertStats = await res.json();
      setStats(st);
    } catch {
      // Stats are non-critical, don't update error state
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    await Promise.all([fetchAlerts(), fetchConfig(), fetchStats()]);
    setLoading(false);
  }, [fetchAlerts, fetchConfig, fetchStats]);

  useEffect(() => {
    refresh();
    const interval = setInterval(fetchStats, 5000);
    return () => clearInterval(interval);
  }, [refresh, fetchStats]);

  useEffect(() => {
    requestNotificationPermission();
  }, [requestNotificationPermission]);

  const save = useCallback(async (next: SpawnAlertConfig): Promise<SpawnAlertConfig> => {
    setSaving(true);
    try {
      const res = await fetch("/api/spawn-alerts/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(next),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const saved: SpawnAlertConfig = await res.json();
      setConfig(saved);
      setSavedAt(Date.now());
      setError(null);
      return saved;
    } catch (e) {
      const message =
        e instanceof Error ? e.message : "Failed to save spawn alert settings";
      setError(message);
      throw new Error(message);
    } finally {
      setSaving(false);
    }
  }, []);

  const addPattern = useCallback(async (pattern: string): Promise<void> => {
    try {
      const res = await fetch("/api/spawn-alerts/watch-list", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ pattern, enabled: true }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      await fetchConfig();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to add pattern");
    }
  }, [fetchConfig]);

  const removePattern = useCallback(async (pattern: string): Promise<void> => {
    try {
      const encoded = encodeURIComponent(pattern);
      const res = await fetch(`/api/spawn-alerts/watch-list/${encoded}`, {
        method: "DELETE",
      });
      if (!res.ok && res.status !== 204) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      await fetchConfig();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to remove pattern");
    }
  }, [fetchConfig]);

  const togglePattern = useCallback(async (pattern: string, enabled: boolean): Promise<void> => {
    try {
      const res = await fetch("/api/spawn-alerts/watch-list", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ pattern, enabled }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      await fetchConfig();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to toggle pattern");
    }
  }, [fetchConfig]);

  const handleWebSocketMessage = useCallback((data: string) => {
    try {
      const event = JSON.parse(data);
      if (event.type === "spawn_alert" && event.data) {
        const alert = event.data as SpawnAlertEntry;
        setAlerts((prev) => [alert, ...prev].slice(0, 100));
        showBrowserNotification(alert);
      }
    } catch {
      // Ignore malformed messages
    }
  }, [showBrowserNotification]);

  return {
    alerts,
    config,
    stats,
    loading,
    saving,
    error,
    savedAt,
    notificationsEnabled,
    requestNotificationPermission,
    refresh,
    save,
    addPattern,
    removePattern,
    togglePattern,
    handleWebSocketMessage,
  };
}
