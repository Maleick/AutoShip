import { useCallback, useEffect, useState } from "react";
import type { MetricsData } from "../types";

export function useAdminMetrics() {
  const [metrics, setMetrics] = useState<MetricsData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch("/api/admin/metrics");
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const data: MetricsData = await res.json();
      setMetrics(data);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Failed to load metrics"
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetch();
    const interval = setInterval(fetch, 5000);
    return () => clearInterval(interval);
  }, [fetch]);

  return { metrics, loading, error };
}
