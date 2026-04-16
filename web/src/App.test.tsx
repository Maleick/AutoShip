import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";
import { createDefaultDiscordSettings } from "./hooks/useDiscordConfig";
import { jsonResponse } from "./test/http";

class MockWebSocket {
  static instances: MockWebSocket[] = [];

  onopen: null | (() => void) = null;
  onclose: null | (() => void) = null;
  onerror: null | (() => void) = null;
  onmessage: null | ((event: MessageEvent<string>) => void) = null;
  sent: string[] = [];

  constructor(public url: string) {
    MockWebSocket.instances.push(this);
  }

  send(data: string) {
    this.sent.push(data);
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

const baseSnapshot = {
  generatedAt: "2026-04-15T08:00:00Z",
  environment: {
    cluster: "Teek",
    shard: "operator-1",
    zone: "Plane of Fire",
    websocketConnected: true,
    alerts: 2,
  },
  sessions: {
    recoveryEnabled: true,
    profiles: ["Cleric Anchor", "Pull Squad", "Loot Crew"],
    items: [
      {
        clientId: 1,
        characterName: "Frostreaver",
        profile: "Cleric Anchor",
        groupId: "grp-1",
        zone: "Plane of Fire",
        level: 60,
        hpPct: 98,
        manaPct: 87,
        status: "online",
        recoveryState: "stable",
        lastHeartbeat: "2s ago",
      },
      {
        clientId: 2,
        characterName: "Noxus",
        profile: "Pull Squad",
        groupId: "grp-1",
        zone: "Plane of Fire",
        level: 60,
        hpPct: 72,
        manaPct: 24,
        status: "stuck",
        recoveryState: "respawning",
        lastHeartbeat: "12s ago",
      },
    ],
  },
  groups: {
    items: [
      {
        id: "grp-1",
        name: "Fire Core",
        zone: "Plane of Fire",
        formation: "Tight Camp",
        currentCommand: "camp",
        members: [
          { characterName: "Noxus", role: "tank", status: "engaged" },
          { characterName: "Frostreaver", role: "healer", status: "support" },
          { characterName: "Aelrindel", role: "dps", status: "engaged" },
        ],
      },
    ],
    commandLog: [
      {
        id: "cmd-1",
        issuedAt: "08:00",
        groupName: "Fire Core",
        command: "camp",
        status: "applied",
      },
    ],
  },
  navigation: {
    currentZone: "Plane of Fire",
    activeRouteId: "route-1",
    stuckClients: 1,
    routes: [
      {
        id: "route-1",
        name: "Fire Ring Sweep",
        zone: "Plane of Fire",
        destination: "Magi Ring",
        progressPct: 62,
        waypoints: [
          { id: "wp-1", x: 10, y: 20, label: "Camp" },
          { id: "wp-2", x: 64, y: 48, label: "Ridge" },
          { id: "wp-3", x: 88, y: 20, label: "Ring" },
        ],
      },
    ],
  },
  economy: {
    itemsReceived: 17,
    lastVendorRun: "14m ago",
    totalProfit: 18423,
    profitTrend: [
      { label: "Mon", value: 2100 },
      { label: "Tue", value: 2800 },
      { label: "Wed", value: 3100 },
      { label: "Thu", value: 2700 },
      { label: "Fri", value: 4100 },
    ],
    recentLoot: [
      {
        id: "loot-1",
        itemName: "Mace of Fiery Might",
        recipient: "Noxus",
        source: "Fennin",
        distribution: "main assist",
      },
    ],
    wishlist: ["Spell: Ancient Flame", "Earring of the Forge"],
  },
  combat: {
    dpsSeries: [
      { characterName: "Aelrindel", color: "#60a5fa", samples: [1200, 1800, 2400, 2100] },
      { characterName: "Noxus", color: "#f97316", samples: [900, 1100, 950, 1050] },
    ],
    spellUsage: [
      { spellName: "Ice Comet", casts: 24, efficiency: 91 },
      { spellName: "Complete Heal", casts: 12, efficiency: 97 },
    ],
    deathLog: [
      {
        id: "death-1",
        characterName: "Valerius",
        reason: "Lava pathing",
        recoveredAt: "08:05",
      },
    ],
    rotations: [
      { characterName: "Aelrindel", efficiency: 88, driftMs: 420 },
      { characterName: "Frostreaver", efficiency: 95, driftMs: 160 },
    ],
  },
  health: {
    clients: [
      { clientId: 1, characterName: "Frostreaver", memoryMb: 684, frameRate: 58, status: "healthy" },
      { clientId: 2, characterName: "Noxus", memoryMb: 742, frameRate: 41, status: "warning" },
    ],
    ipcLatency: { p50: 9, p95: 22, p99: 37 },
    errorLog: [
      {
        id: "err-1",
        severity: "warning",
        message: "Noxus exceeded stuck threshold",
        recoveryAction: "Respawn requested",
      },
    ],
  },
} as const;

const baseDiscordSettings = createDefaultDiscordSettings();

function mockDashboardFetch(actionSnapshot = baseSnapshot) {
  const fetchMock = vi.mocked(fetch);
  fetchMock.mockImplementation(async (input, init) => {
    if (input === "/api/dashboard") {
      return jsonResponse(baseSnapshot);
    }
    if (input === "/api/config/discord") {
      return jsonResponse(baseDiscordSettings);
    }
    if (input === "/api/dashboard/action") {
      expect(init).toEqual(
        expect.objectContaining({
          method: "POST",
          headers: { "Content-Type": "application/json" },
        })
      );
      return jsonResponse(actionSnapshot);
    }

    throw new Error(`Unexpected fetch call: ${String(input)}`);
  });

  return fetchMock;
}

describe("App dashboard integration", () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal("WebSocket", MockWebSocket as unknown as typeof WebSocket);
    vi.stubGlobal("fetch", vi.fn());
  });

  it("renders the operator dashboard sections from the backend snapshot", async () => {
    const fetchMock = mockDashboardFetch();

    render(<App />);

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/dashboard")
    );

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: /session command center/i })
      ).toBeInTheDocument()
    );

    expect(screen.getByRole("heading", { name: /group coordination/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /navigation control/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /economy monitoring/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /combat analytics/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /system health/i })).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: /security wards/i })).toBeInTheDocument()
    );
    expect(screen.getAllByText(/plane of fire/i).length).toBeGreaterThan(0);
  }, 15000);

  it("submits session actions and applies websocket snapshot refreshes", async () => {
    const fetchMock = mockDashboardFetch({
      ...baseSnapshot,
      sessions: {
        ...baseSnapshot.sessions,
        items: [
          ...baseSnapshot.sessions.items,
          {
            clientId: 3,
            characterName: "Newpuller",
            profile: "Loot Crew",
            groupId: "grp-2",
            zone: "Plane of Fire",
            level: 60,
            hpPct: 100,
            manaPct: 100,
            status: "online",
            recoveryState: "stable",
            lastHeartbeat: "0s ago",
          },
        ],
      },
    });

    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /new session/i })
      ).toBeInTheDocument()
    );

    fireEvent.click(screen.getByRole("button", { name: /new session/i }));
    fireEvent.change(screen.getByLabelText(/profile/i), {
      target: { value: "Loot Crew" },
    });
    fireEvent.change(screen.getByLabelText(/character name/i), {
      target: { value: "Newpuller" },
    });
    fireEvent.click(screen.getByRole("button", { name: /create session/i }));

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/dashboard/action",
        expect.objectContaining({
          method: "POST",
          headers: { "Content-Type": "application/json" },
        })
      )
    );

    const nextSnapshot = {
      type: "dashboard.snapshot",
      source: "tick",
      snapshot: {
        ...baseSnapshot,
        health: {
          ...baseSnapshot.health,
          ipcLatency: { p50: 11, p95: 29, p99: 44 },
        },
      },
    };

    await act(async () => {
      MockWebSocket.instances[0].push(nextSnapshot);
    });

    await waitFor(() => expect(screen.getByText("44 ms")).toBeInTheDocument());
    expect(screen.getByText(/newpuller/i)).toBeInTheDocument();
  }, 15000);
});
