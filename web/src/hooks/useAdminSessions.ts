import { useEffect, useState } from "react";

export interface AdminSession {
  session_id: number;
  character_name: string | null;
  class_name: string | null;
  group_id: number;
  routing_scope: {
    kind: string;
    label: string;
    group_id: number | null;
    toon_name: string | null;
  };
  lifecycle_state: string;
}

export function useAdminSessions() {
  const [sessions, setSessions] = useState<AdminSession[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/admin/sessions")
      .then((res) => {
        if (!res.ok) throw new Error("Failed to fetch sessions");
        return res.json();
      })
      .then((data) => {
        setSessions(data);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, []);

  return { sessions, loading, error };
}