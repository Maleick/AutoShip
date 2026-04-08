import { useState, useEffect } from "react";
import type { Session } from "../types";

export function useSessions() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchSessions = async () => {
      try {
        const res = await fetch("/api/sessions");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data = await res.json();
        setSessions(data);
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Failed to fetch");
      } finally {
        setLoading(false);
      }
    };
    fetchSessions();
    const interval = setInterval(fetchSessions, 5000);
    return () => clearInterval(interval);
  }, []);

  return { sessions, loading, error };
}
