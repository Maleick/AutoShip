import { useCallback, useEffect, useMemo, useState } from "react";

import type {
  ExtensionCatalogEntry,
  ExtensionRuntimeEvent,
  ExtensionRuntimeStatus,
  ScopeKind,
} from "../extensions";
import { useWebSocket } from "./useWebSocket";

const EXTENSION_DOMAINS = [
  "combat",
  "navigation",
  "loot",
  "awareness",
  "economy",
  "operator_utilities",
] as const;
const COMPATIBILITY_TIERS = ["native", "adapted", "legacy"] as const;
const EXTENSION_SOURCE_KINDS = ["textquest_native", "legacy_profile"] as const;
const CONFIG_PROVENANCE_KINDS = ["extension_catalog", "legacy_import"] as const;
const SCOPE_KINDS = ["character", "group", "session"] as const;
const FIELD_KINDS = ["boolean", "integer", "string", "enum", "string_array"] as const;
const ADAPTER_HEALTH_VALUES = ["healthy", "degraded", "disabled"] as const;

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isEnumValue<T extends string>(
  value: unknown,
  allowedValues: readonly T[]
): value is T {
  return typeof value === "string" && allowedValues.includes(value as T);
}

function isExtensionFieldOption(value: unknown): boolean {
  return (
    isObjectRecord(value) &&
    typeof value.value === "string" &&
    typeof value.label === "string"
  );
}

function isExtensionFieldSchema(value: unknown): boolean {
  return (
    isObjectRecord(value) &&
    typeof value.key === "string" &&
    typeof value.label === "string" &&
    typeof value.description === "string" &&
    isEnumValue(value.kind, FIELD_KINDS) &&
    typeof value.required === "boolean" &&
    Object.hasOwn(value, "defaultValue") &&
    (value.options === undefined ||
      (Array.isArray(value.options) && value.options.every(isExtensionFieldOption))) &&
    (value.min === undefined || isFiniteNumber(value.min)) &&
    (value.max === undefined || isFiniteNumber(value.max))
  );
}

function isConfigProvenanceInfo(value: unknown): boolean {
  return (
    isObjectRecord(value) &&
    isEnumValue(value.kind, CONFIG_PROVENANCE_KINDS) &&
    typeof value.label === "string" &&
    (value.path === undefined ||
      value.path === null ||
      typeof value.path === "string")
  );
}

function isScopeRef(value: unknown): boolean {
  return (
    isObjectRecord(value) &&
    isEnumValue(value.kind, SCOPE_KINDS) &&
    typeof value.id === "string"
  );
}

function isExtensionScopeOverride(value: unknown): boolean {
  return (
    isObjectRecord(value) &&
    isScopeRef(value.scope) &&
    isObjectRecord(value.settings)
  );
}

function isExtensionRuntimeStatus(value: unknown): boolean {
  return (
    isObjectRecord(value) &&
    typeof value.enabled === "boolean" &&
    isEnumValue(value.adapterHealth, ADAPTER_HEALTH_VALUES) &&
    typeof value.lastSyncMessage === "string" &&
    (value.degradedReason === undefined ||
      value.degradedReason === null ||
      typeof value.degradedReason === "string") &&
    (value.lastSyncAt === undefined ||
      value.lastSyncAt === null ||
      typeof value.lastSyncAt === "string")
  );
}

function isExtensionCatalogEntry(value: unknown): value is ExtensionCatalogEntry {
  return (
    isObjectRecord(value) &&
    typeof value.id === "string" &&
    typeof value.displayName === "string" &&
    typeof value.description === "string" &&
    isEnumValue(value.domain, EXTENSION_DOMAINS) &&
    isEnumValue(value.compatibilityTier, COMPATIBILITY_TIERS) &&
    isEnumValue(value.sourceKind, EXTENSION_SOURCE_KINDS) &&
    isConfigProvenanceInfo(value.configProvenance) &&
    Array.isArray(value.supportedScopes) &&
    value.supportedScopes.every((scope) => isEnumValue(scope, SCOPE_KINDS)) &&
    Array.isArray(value.schema) &&
    value.schema.every(isExtensionFieldSchema) &&
    isObjectRecord(value.settings) &&
    Array.isArray(value.overrides) &&
    value.overrides.every(isExtensionScopeOverride) &&
    isExtensionRuntimeStatus(value.runtime) &&
    isStringArray(value.unsupportedFields) &&
    (value.legacySourceName === undefined ||
      value.legacySourceName === null ||
      typeof value.legacySourceName === "string")
  );
}

function isExtensionRuntimeEvent(value: unknown): value is ExtensionRuntimeEvent {
  return (
    isObjectRecord(value) &&
    value.type === "extension.runtime" &&
    isExtensionCatalogEntry(value.entry)
  );
}

function getWebSocketUrl(): string {
  if (typeof window === "undefined") {
    return "ws://localhost/ws";
  }

  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/ws`;
}

function parseRuntimeEvent(raw: string | null): ExtensionRuntimeEvent | null {
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as unknown;
    return isExtensionRuntimeEvent(parsed) ? parsed : null;
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
    // Ignore parsing failures and fall back to the HTTP status.
  }

  return `HTTP ${response.status}`;
}

function replaceEntry(
  entries: ExtensionCatalogEntry[],
  nextEntry: ExtensionCatalogEntry
): ExtensionCatalogEntry[] {
  return entries.map((entry) => (entry.id === nextEntry.id ? nextEntry : entry));
}

function patchRuntime(
  entries: ExtensionCatalogEntry[],
  id: string,
  runtime: ExtensionRuntimeStatus
): ExtensionCatalogEntry[] {
  return entries.map((entry) =>
    entry.id === id ? { ...entry, runtime } : entry
  );
}

export function useExtensionCatalog() {
  const [entries, setEntries] = useState<ExtensionCatalogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const wsUrl = useMemo(() => getWebSocketUrl(), []);
  const { lastMessage } = useWebSocket(wsUrl);

  const refresh = useCallback(async () => {
    try {
      const response = await fetch("/api/extensions/catalog");
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload = (await response.json()) as ExtensionCatalogEntry[];
      setEntries(payload);
      setError(null);
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Failed to load extension catalog"
      );
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshEntry = useCallback(async (id: string) => {
    const response = await fetch(`/api/extensions/catalog/${id}`);
    if (!response.ok) {
      throw new Error(await readErrorMessage(response));
    }

    const payload = (await response.json()) as ExtensionCatalogEntry;
    setEntries((current) => replaceEntry(current, payload));
    return payload;
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const event = parseRuntimeEvent(lastMessage);
    if (!event) {
      return;
    }

    setEntries((current) =>
      current.some((entry) => entry.id === event.entry.id)
        ? replaceEntry(current, event.entry)
        : [...current, event.entry]
    );
  }, [lastMessage]);

  const saveSettings = useCallback(
    async (id: string, settings: Record<string, unknown>) => {
      const response = await fetch(`/api/extensions/catalog/${id}/settings`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ settings }),
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload = (await response.json()) as ExtensionCatalogEntry;
      setEntries((current) => replaceEntry(current, payload));
      setError(null);
      return payload;
    },
    []
  );

  const saveScopeOverride = useCallback(
    async (
      id: string,
      scopeKind: ScopeKind,
      scopeId: string,
      settings: Record<string, unknown>
    ) => {
      const response = await fetch(
        `/api/extensions/catalog/${id}/scopes/${scopeKind}/${encodeURIComponent(scopeId)}`,
        {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ settings }),
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      await refreshEntry(id);
      setError(null);
    },
    [refreshEntry]
  );

  const deleteScopeOverride = useCallback(
    async (id: string, scopeKind: ScopeKind, scopeId: string) => {
      const response = await fetch(
        `/api/extensions/catalog/${id}/scopes/${scopeKind}/${encodeURIComponent(scopeId)}`,
        {
          method: "DELETE",
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      await refreshEntry(id);
      setError(null);
    },
    [refreshEntry]
  );

  const saveRuntime = useCallback(async (id: string, enabled: boolean) => {
    const response = await fetch(`/api/extensions/catalog/${id}/runtime`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ enabled }),
    });
    if (!response.ok) {
      throw new Error(await readErrorMessage(response));
    }

    const runtime = (await response.json()) as ExtensionRuntimeStatus;
    setEntries((current) => patchRuntime(current, id, runtime));
    setError(null);
    return runtime;
  }, []);

  return {
    entries,
    loading,
    error,
    refresh,
    saveSettings,
    saveScopeOverride,
    deleteScopeOverride,
    saveRuntime,
  };
}
