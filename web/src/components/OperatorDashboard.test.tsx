import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DashboardSnapshot } from "../dashboard";
import OperatorDashboard from "./OperatorDashboard";
import { useDashboard } from "../hooks/useDashboard";

vi.mock("../hooks/useDashboard", () => ({
  useDashboard: vi.fn(),
}));

vi.mock("./AutoAcceptPanel", () => ({
  default: () => <div>AutoAcceptPanel</div>,
}));

vi.mock("./BoxChatPanel", () => ({
  default: () => <div>BoxChatPanel</div>,
}));

vi.mock("./ChatLogPanel", () => ({
  default: () => <div>ChatLogPanel</div>,
}));

vi.mock("./DiscordConfigPanel", () => ({
  default: () => <div>DiscordConfigPanel</div>,
}));

vi.mock("./GmAlertPanel", () => ({
  default: () => <div>GmAlertPanel</div>,
}));

vi.mock("./KillTrackerPanel", () => ({
  default: () => <div>KillTrackerPanel</div>,
}));

const SNAPSHOT: DashboardSnapshot = {
  generatedAt: "2026-04-17T09:00:00Z",
  environment: {
    cluster: "Teek",
    shard: "operator-1",
    zone: "Plane of Fire",
    websocketConnected: true,
    alerts: 2,
  },
  sessions: {
    recoveryEnabled: true,
    profiles: ["Cleric Anchor"],
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
        lastHeartbeat: "1s ago",
      },
    ],
  },
  spawnFinder: {
    observers: [
      {
        clientId: 1,
        characterName: "Frostreaver",
        zone: "Plane of Fire",
        totalSpawns: 2,
      },
    ],
    items: [
      {
        observerClientId: 1,
        observerName: "Frostreaver",
        observerZone: "Plane of Fire",
        spawnId: 9001,
        name: "a fire giant",
        spawnType: "NPC",
        level: 61,
        className: "WAR",
        raceName: "Ogre",
        distance: 18,
        hpPct: 82,
        isCurrentTarget: true,
      },
      {
        observerClientId: 1,
        observerName: "Frostreaver",
        observerZone: "Plane of Fire",
        spawnId: 9002,
        name: "a lava walker",
        spawnType: "NPC",
        level: 60,
        className: "MNK",
        raceName: "Human",
        distance: 42,
        hpPct: 100,
        isCurrentTarget: false,
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
        members: [{ characterName: "Frostreaver", role: "healer", status: "support" }],
      },
    ],
    commandLog: [
      {
        id: "cmd-1",
        issuedAt: "09:00",
        groupName: "Fire Core",
        command: "camp",
        status: "applied",
      },
    ],
  },
  navigation: {
    currentZone: "Plane of Fire",
    activeRouteId: "route-1",
    stuckClients: 0,
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
        ],
      },
    ],
  },
  relocation: {
    readyDestinations: 2,
    coolingDownCount: 2,
    destinations: [
      {
        zone: "guildlobby",
        label: "Guild Lobby",
        preferredOption: "Throne of Heroes",
        preferredSource: "aa",
        options: [
          {
            id: "throne_of_heroes",
            name: "Throne of Heroes",
            source: "aa",
            owned: true,
            ready: true,
            cooldownRemainingSecs: null,
          },
        ],
      },
      {
        zone: "guildhall",
        label: "Guild Hall",
        preferredOption: "Secondary Anchor",
        preferredSource: "item",
        options: [
          {
            id: "primary_anchor",
            name: "Primary Anchor",
            source: "item",
            owned: true,
            ready: false,
            cooldownRemainingSecs: 900,
          },
          {
            id: "secondary_anchor",
            name: "Secondary Anchor",
            source: "item",
            owned: true,
            ready: false,
            cooldownRemainingSecs: 480,
          },
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
    ],
    recentLoot: [
      {
        id: "loot-1",
        itemName: "Mace of Fiery Might",
        recipient: "Frostreaver",
        source: "Fennin",
        distribution: "main assist",
      },
    ],
    wishlist: ["Spell: Ancient Flame"],
  },
  combat: {
    dpsSeries: [{ characterName: "Frostreaver", color: "#60a5fa", samples: [1200, 1800] }],
    spellUsage: [{ spellName: "Complete Heal", casts: 12, efficiency: 97 }],
    deathLog: [
      {
        id: "death-1",
        characterName: "Valerius",
        reason: "Lava pathing",
        recoveredAt: "09:05",
      },
    ],
    rotations: [{ characterName: "Frostreaver", efficiency: 95, driftMs: 160 }],
  },
  health: {
    clients: [
      {
        clientId: 1,
        characterName: "Frostreaver",
        memoryMb: 684,
        frameRate: 58,
        status: "healthy",
      },
    ],
    ipcLatency: { p50: 9, p95: 22, p99: 37 },
    errorLog: [
      {
        id: "err-1",
        severity: "warning",
        message: "Route watchdog tripped",
        recoveryAction: "Respawn requested",
      },
    ],
  },
};

describe("OperatorDashboard relocation panel", () => {
  beforeEach(() => {
    vi.mocked(useDashboard).mockReturnValue({
      snapshot: SNAPSHOT,
      loading: false,
      error: null,
      connected: true,
      refresh: vi.fn().mockResolvedValue(undefined),
      submitAction: vi.fn().mockResolvedValue(SNAPSHOT),
    });
  });

  it(
    "renders relocation destinations with preferred source and cooldown status",
    () => {
      render(<OperatorDashboard />);

      expect(
        screen.getByRole("heading", { name: /relocation network/i })
      ).toBeInTheDocument();
      expect(screen.getByRole("heading", { name: /spawn finder/i })).toBeInTheDocument();
      expect(screen.getByText("Guild Lobby")).toBeInTheDocument();
      expect(screen.getAllByText("Throne of Heroes")).toHaveLength(2);
      expect(screen.getByText("Preferred AA")).toBeInTheDocument();
      expect(screen.getByText("480s cooldown")).toBeInTheDocument();
      expect(screen.getByText("Primary Anchor")).toBeInTheDocument();
      expect(screen.getAllByText("Secondary Anchor")).toHaveLength(2);
    },
    15000
  );
});
