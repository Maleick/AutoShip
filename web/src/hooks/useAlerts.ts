import { useEffect, useState } from "react";

import type { AlertingConfig, OperationalAlert } from "../types";

interface AlertsApiResponse {
  alerts: OperationalAlert[];
  unread_count: number;
}

interface AlertConfigResponse {
  config: AlertingConfig;
}

const DEFAULT_ALERT_CONFIG: AlertingConfig = {
  enable_discord: false,
  discord_webhook_url: "",
  enable_email: false,
  smtp_server: "",
  smtp_port: 587,
  smtp_username: "",
  smtp_password: "",
  email_from: "",
  email_recipients: [],
  email_subject_prefix: "[TextQuest] ",
  warning_batch_window_secs: 300,
  thresholds: {
    death_alert: true,
    stuck_alert: true,
    memory_warning_mb: 200,
    ipc_latency_warning_ms: 10,
    error_rate_warning_per_min: 5,
    dps_drop_warning_pct: 20,
    zone_timeout_secs: 60,
  },
};

export function useAlerts() {
  const [alerts, setAlerts] = useState<OperationalAlert[]>([]);
  const [unreadCount, setUnreadCount] = useState(0);
  const [config, setConfig] = useState<AlertingConfig>(DEFAULT_ALERT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;

    const load = async () => {
      try {
        const [alertsRes, configRes] = await Promise.all([
          fetch("/api/alerts"),
          fetch("/api/alerts/config"),
        ]);
        if (!alertsRes.ok) {
          throw new Error(`Alert API failed (${alertsRes.status})`);
        }
        if (!configRes.ok) {
          throw new Error(`Alert config API failed (${configRes.status})`);
        }

        const alertsPayload = (await alertsRes.json()) as AlertsApiResponse;
        const configPayload = (await configRes.json()) as AlertConfigResponse;
        if (!active) {
          return;
        }

        setAlerts(alertsPayload.alerts);
        setUnreadCount(alertsPayload.unread_count);
        setConfig(configPayload.config);
        setError(null);
      } catch (fetchError) {
        if (!active) {
          return;
        }
        setError(
          fetchError instanceof Error ? fetchError.message : "Failed to load alerts",
        );
      } finally {
        if (active) {
          setLoading(false);
        }
      }
    };

    void load();
    const interval = setInterval(load, 10000);
    return () => {
      active = false;
      clearInterval(interval);
    };
  }, []);

  const refresh = async () => {
    setLoading(true);
    try {
      const response = await fetch("/api/alerts");
      if (!response.ok) {
        throw new Error(`Alert API failed (${response.status})`);
      }
      const payload = (await response.json()) as AlertsApiResponse;
      setAlerts(payload.alerts);
      setUnreadCount(payload.unread_count);
      setError(null);
    } catch (refreshError) {
      setError(
        refreshError instanceof Error ? refreshError.message : "Failed to refresh alerts",
      );
    } finally {
      setLoading(false);
    }
  };

  const acknowledgeAlert = async (id: number) => {
    try {
      const response = await fetch(`/api/alerts/${id}/ack`, { method: "POST" });
      if (!response.ok) {
        throw new Error(`Failed to acknowledge alert (${response.status})`);
      }
      await refresh();
    } catch (ackError) {
      setError(
        ackError instanceof Error
          ? ackError.message
          : "Failed to acknowledge alert",
      );
    }
  };

  const acknowledgeAll = async () => {
    try {
      const response = await fetch("/api/alerts/ack-all", { method: "POST" });
      if (!response.ok) {
        throw new Error(`Failed to acknowledge alerts (${response.status})`);
      }
      await refresh();
    } catch (ackError) {
      setError(
        ackError instanceof Error
          ? ackError.message
          : "Failed to acknowledge alerts",
      );
    }
  };

  const saveConfig = async (nextConfig: AlertingConfig) => {
    setSaving(true);
    try {
      const response = await fetch("/api/alerts/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(nextConfig),
      });
      if (!response.ok) {
        throw new Error(`Failed to save alert config (${response.status})`);
      }
      setConfig(nextConfig);
      setError(null);
      await refresh();
    } catch (saveError) {
      setError(
        saveError instanceof Error
          ? saveError.message
          : "Failed to save alert config",
      );
    } finally {
      setSaving(false);
    }
  };

  return {
    alerts,
    unreadCount,
    config,
    setConfig,
    loading,
    saving,
    error,
    refresh,
    acknowledgeAlert,
    acknowledgeAll,
    saveConfig,
  };
}
