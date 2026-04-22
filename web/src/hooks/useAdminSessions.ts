import { useEffect, useState } from "react";

import type { AdminSessionRecord } from "../types";

const POLL_INTERVAL_MS = 5000;

function readString(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }

  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function readNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function readFirstString(
  record: Record<string, unknown>,
  keys: string[],
): string | null {
  for (const key of keys) {
    const value = readString(record[key]);
    if (value) {
      return value;
    }
  }

  return null;
}

function readFirstNumber(
  record: Record<string, unknown>,
  keys: string[],
): number | null {
  for (const key of keys) {
    const value = readNumber(record[key]);
    if (value != null) {
      return value;
    }
  }

  return null;
}

function normalizeSessionRecord(value: unknown): AdminSessionRecord | null {
  if (!value || typeof value !== "object") {
    return null;
  }

  const record = value as Record<string, unknown>;
  const rawSessionId =
    readFirstString(record, ["sessionId", "session_id"]) ??
    readFirstString(record, ["clientId", "client_id"]) ??
    (() => {
      const numberValue = readFirstNumber(record, [
        "sessionId",
        "session_id",
        "clientId",
        "client_id",
      ]);
      return numberValue == null ? null : String(numberValue);
    })();
  const characterName = readFirstString(record, [
    "characterName",
    "character_name",
    "name",
  ]);

  if (!rawSessionId || !characterName) {
    return null;
  }

  return {
    sessionId: rawSessionId,
    characterName,
    profile: readFirstString(record, ["profile"]),
    groupId:
      readFirstString(record, ["groupId", "group_id"]) ??
      (() => {
        const numberValue = readFirstNumber(record, ["groupId", "group_id"]);
        return numberValue == null ? null : String(numberValue);
      })(),
    routingScope: readFirstString(record, ["routingScope", "routing_scope"]),
    lifecycle: readFirstString(record, [
      "lifecycle",
      "slotLifecycle",
      "slot_lifecycle",
    ]),
    status: readFirstString(record, ["status", "state"]),
    zone: readFirstString(record, ["zone", "zoneLongName", "zone_long_name"]),
    level: readFirstNumber(record, ["level"]),
    className: readFirstString(record, ["className", "class_name"]),
    lastHeartbeat: readFirstString(record, ["lastHeartbeat", "last_heartbeat"]),
  };
}

function normalizeAdminSessions(payload: unknown): AdminSessionRecord[] {
  if (!Array.isArray(payload)) {
    return [];
  }

  return payload
    .map((value) => normalizeSessionRecord(value))
    .filter((value): value is AdminSessionRecord => value !== null);
}

export function useAdminSessions() {
  const [sessions, setSessions] = useState<AdminSessionRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const fetchSessions = async () => {
      try {
        const response = await fetch("/api/admin/sessions");
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }

        const payload = await response.json();
        if (cancelled) {
          return;
        }

        setSessions(normalizeAdminSessions(payload));
        setError(null);
      } catch (nextError) {
        if (cancelled) {
          return;
        }

        setError(
          nextError instanceof Error
            ? nextError.message
            : "Failed to fetch admin sessions",
        );
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void fetchSessions();
    const interval = window.setInterval(() => {
      void fetchSessions();
    }, POLL_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  return { sessions, loading, error };
}
