import { useCallback, useEffect, useState } from "react";
import type {
  KillTrackerDashboard,
  KillTrackerSettings,
  SessionStats,
} from "../types";

export interface KillTrackerState {
  dashboard: KillTrackerDashboard | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  updateSettings: (settings: KillTrackerSettings) => Promise<void>;
}

export function useKillTracker(): KillTrackerState {
  const [dashboard, setDashboard] = useState<KillTrackerDashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchDashboard = useCallback(async () => {
    try {
      const [settingsRes, sessionsRes] = await Promise.all([
        fetch("/api/kill-tracker/settings"),
        fetch("/api/kill-tracker/sessions"),
      ]);

      if (!settingsRes.ok) {
        throw new Error(`HTTP ${settingsRes.status}`);
      }

      const settings = (await settingsRes.json()) as KillTrackerSettings;
      let sessions: SessionStats[] = [];
      if (sessionsRes.ok) {
        sessions = (await sessionsRes.json()) as SessionStats[];
      }

      const currentSession = sessions.length > 0 ? sessions[sessions.length - 1] : null;

      setDashboard({
        currentSession,
        characterHistory: [],
        settings,
      });
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to fetch kill tracker data");
    } finally {
      setLoading(false);
    }
  }, []);

  const updateSettings = useCallback(async (settings: KillTrackerSettings) => {
    try {
      const response = await fetch("/api/kill-tracker/settings", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(settings),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      setDashboard((prev) =>
        prev ? { ...prev, settings } : null
      );
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update settings");
      throw err;
    }
  }, []);

  useEffect(() => {
    void fetchDashboard();
  }, [fetchDashboard]);

  return {
    dashboard,
    loading,
    error,
    refresh: fetchDashboard,
    updateSettings,
  };
}
