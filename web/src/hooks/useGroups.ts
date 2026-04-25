import { useState, useEffect, useCallback } from "react";
import type {
  Group,
  CreateGroupPayload,
  UpdateGroupPayload,
  CampConfiguration,
  CreateCampConfigPayload,
  UpdateCampConfigPayload,
} from "../types";

export function useGroups() {
  const [groups, setGroups] = useState<Group[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchGroups = useCallback(async () => {
    try {
      const res = await fetch("/api/groups");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: Group[] = await res.json();
      setGroups(data);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to fetch groups");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchGroups();
  }, [fetchGroups]);

  const createGroup = useCallback(
    async (payload: CreateGroupPayload): Promise<Group> => {
      const res = await fetch("/api/groups", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const created: Group = await res.json();
      setGroups((prev) =>
        [...prev, created].sort((a, b) => a.name.localeCompare(b.name)),
      );
      return created;
    },
    [],
  );

  const updateGroup = useCallback(
    async (id: string, payload: UpdateGroupPayload): Promise<Group> => {
      const res = await fetch(`/api/groups/${encodeURIComponent(id)}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const updated: Group = await res.json();
      setGroups((prev) => prev.map((g) => (g.id === id ? updated : g)));
      return updated;
    },
    [],
  );

  const deleteGroup = useCallback(async (id: string): Promise<void> => {
    const res = await fetch(`/api/groups/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body.error ?? `HTTP ${res.status}`);
    }
    setGroups((prev) => prev.filter((g) => g.id !== id));
  }, []);

  return {
    groups,
    loading,
    error,
    createGroup,
    updateGroup,
    deleteGroup,
    refetch: fetchGroups,
  };
}

export function useCampConfiguration() {
  const [campConfigs, setCampConfigs] = useState<CampConfiguration[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchCampConfigs = useCallback(async () => {
    try {
      const res = await fetch("/api/camps");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: CampConfiguration[] = await res.json();
      setCampConfigs(data);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Failed to fetch camp configurations",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchCampConfigs();
  }, [fetchCampConfigs]);

  const createCampConfig = useCallback(
    async (payload: CreateCampConfigPayload): Promise<CampConfiguration> => {
      const res = await fetch("/api/camps", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const created: CampConfiguration = await res.json();
      setCampConfigs((prev) => [...prev, created]);
      return created;
    },
    [],
  );

  const updateCampConfig = useCallback(
    async (
      id: string,
      payload: UpdateCampConfigPayload,
    ): Promise<CampConfiguration> => {
      const res = await fetch(`/api/camps/${encodeURIComponent(id)}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const updated: CampConfiguration = await res.json();
      setCampConfigs((prev) => prev.map((c) => (c.id === id ? updated : c)));
      return updated;
    },
    [],
  );

  const deleteCampConfig = useCallback(async (id: string): Promise<void> => {
    const res = await fetch(`/api/camps/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body.error ?? `HTTP ${res.status}`);
    }
    setCampConfigs((prev) => prev.filter((c) => c.id !== id));
  }, []);

  return {
    campConfigs,
    loading,
    error,
    createCampConfig,
    updateCampConfig,
    deleteCampConfig,
    refetch: fetchCampConfigs,
  };
}
