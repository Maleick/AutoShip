import { useCallback, useEffect, useState } from "react";
import type { LogEntry } from "../types";

export function useAdminLogs() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    try {
      const res = await fetch("/api/admin/logs");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const data = await res.json();
      setLogs(Array.isArray(data) ? data : []);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Failed to load logs"
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetch();
    const interval = setInterval(fetch, 3000);
    return () => clearInterval(interval);
  }, [fetch]);

  return { logs, loading, error };
}
