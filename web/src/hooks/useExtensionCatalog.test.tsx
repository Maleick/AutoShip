import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useExtensionCatalog } from "./useExtensionCatalog";
import { jsonResponse } from "../test/http";

class MockWebSocket {
  static instances: MockWebSocket[] = [];

  onopen: null | (() => void) = null;
  onclose: null | (() => void) = null;
  onerror: null | (() => void) = null;
  onmessage: null | ((event: MessageEvent<string>) => void) = null;

  constructor(public url: string) {
    MockWebSocket.instances.push(this);
  }

  close() {
    this.onclose?.();
  }

  push(event: unknown) {
    this.onmessage?.({
      data: JSON.stringify(event),
    } as MessageEvent<string>);
  }
}

const ENTRY = {
  id: "mq2eqbc",
  displayName: "MQ2EQBC",
  description: "Relay integration",
  domain: "operator_utilities",
  compatibilityTier: "adapted",
  sourceKind: "textquest_native",
  configProvenance: {
    kind: "extension_catalog",
    label: "Dashboard sidecar",
    path: "config/extensions-catalog.json",
  },
  supportedScopes: ["character", "group", "session"],
  schema: [
    {
      key: "enabled",
      label: "Enabled",
      description: "Enable relay integration.",
      kind: "boolean",
      required: true,
      defaultValue: false,
    },
    {
      key: "host",
      label: "Host",
      description: "Relay host.",
      kind: "string",
      required: true,
      defaultValue: "127.0.0.1",
    },
  ],
  settings: {
    enabled: false,
    host: "127.0.0.1",
  },
  overrides: [],
  runtime: {
    enabled: false,
    adapterHealth: "disabled",
    lastSyncMessage: "Runtime disabled",
  },
  unsupportedFields: [],
};

describe("useExtensionCatalog", () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal("WebSocket", MockWebSocket as unknown as typeof WebSocket);
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("loads entries, saves settings, and applies runtime websocket events", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(jsonResponse([ENTRY]))
      .mockResolvedValueOnce(
        jsonResponse({
          ...ENTRY,
          settings: {
            enabled: true,
            host: "10.0.0.5",
          },
        }),
      );

    const { result } = renderHook(() => useExtensionCatalog());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.entries).toHaveLength(1);

    await act(async () => {
      await result.current.saveSettings("mq2eqbc", {
        enabled: true,
        host: "10.0.0.5",
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/extensions/catalog/mq2eqbc/settings",
      expect.objectContaining({
        method: "PUT",
        headers: { "Content-Type": "application/json" },
      }),
    );
    expect(result.current.entries[0]?.settings.host).toBe("10.0.0.5");

    await act(async () => {
      MockWebSocket.instances[0]?.push({
        type: "extension.runtime",
        entry: {
          ...ENTRY,
          runtime: {
            enabled: true,
            adapterHealth: "healthy",
            lastSyncAt: "2026-04-17T10:00:00Z",
            lastSyncMessage: "Runtime toggle applied from dashboard",
          },
        },
      });
    });

    expect(result.current.entries[0]?.runtime.enabled).toBe(true);
    expect(result.current.entries[0]?.runtime.adapterHealth).toBe("healthy");
  });

  it("ignores malformed runtime websocket events", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(jsonResponse([ENTRY]));

    const { result } = renderHook(() => useExtensionCatalog());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.entries).toEqual([ENTRY]);

    await act(async () => {
      MockWebSocket.instances[0]?.push({
        type: "extension.runtime",
      });
    });

    expect(result.current.entries).toEqual([ENTRY]);
    expect(result.current.error).toBeNull();
  });

  it("accepts runtime websocket events when provenance path is omitted", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(jsonResponse([ENTRY]));

    const { result } = renderHook(() => useExtensionCatalog());

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      MockWebSocket.instances[0]?.push({
        type: "extension.runtime",
        entry: {
          ...ENTRY,
          sourceKind: "legacy_profile",
          configProvenance: {
            kind: "legacy_import",
            label: "Imported legacy profile",
          },
          runtime: {
            enabled: true,
            adapterHealth: "degraded",
            lastSyncAt: "2026-04-17T10:00:00Z",
            lastSyncMessage: "Enabled in degraded mode for imported legacy profile",
          },
          legacySourceName: "kissassist.ini",
        },
      });
    });

    expect(result.current.entries[0]?.runtime.enabled).toBe(true);
    expect(result.current.entries[0]?.runtime.adapterHealth).toBe("degraded");
    expect(result.current.entries[0]?.configProvenance.kind).toBe("legacy_import");
  });
});
