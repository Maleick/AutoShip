import { useCallback, useEffect, useMemo, useState } from "react";

import type {
  DashboardActionRequest,
  DashboardEvent,
  DashboardSnapshot,
} from "../dashboard";
import { useWebSocket } from "./useWebSocket";

function getWebSocketUrl(): string {
  if (typeof window === "undefined") {
    return "ws://localhost/ws";
  }

  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/ws`;
}

function parseEvent(raw: string | null): DashboardEvent | null {
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as DashboardEvent;
    return parsed.type === "dashboard.snapshot" ? parsed : null;
  } catch {
    return null;
  }
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

function mergeById<T extends { clientId: number }>(current: T[], incoming: T[]) {
  const incomingIds = new Set(incoming.map((item) => item.clientId));
  return [...incoming, ...current.filter((item) => !incomingIds.has(item.clientId))];
}

function mergeSnapshot(
  current: DashboardSnapshot | null,
  incoming: DashboardSnapshot
): DashboardSnapshot {
  if (!current) {
    return incoming;
  }

  return {
    ...incoming,
    sessions: {
      ...incoming.sessions,
      items: mergeById(current.sessions.items, incoming.sessions.items),
    },
    health: {
      ...incoming.health,
      clients: mergeById(current.health.clients, incoming.health.clients),
    },
  };
}

export function useDashboard() {
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const wsUrl = useMemo(() => getWebSocketUrl(), []);
  const { connected, lastMessage } = useWebSocket(wsUrl);

  const refresh = useCallback(async () => {
    try {
      const response = await fetch("/api/dashboard");
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const nextSnapshot = (await response.json()) as DashboardSnapshot;
      setSnapshot((current) => mergeSnapshot(current, nextSnapshot));
      setError(null);
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Failed to fetch dashboard snapshot"
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const event = parseEvent(lastMessage);
    if (!event) {
      return;
    }
    setSnapshot((current) => mergeSnapshot(current, event.snapshot));
  }, [lastMessage]);

  const submitAction = useCallback(
    async (action: DashboardActionRequest) => {
      const response = await fetch("/api/dashboard/action", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(action),
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const nextSnapshot = (await response.json()) as DashboardSnapshot;
      setSnapshot((current) => mergeSnapshot(current, nextSnapshot));
      return nextSnapshot;
    },
    []
  );

  return {
    snapshot,
    loading,
    error,
    connected,
    refresh,
    submitAction,
  };
}
