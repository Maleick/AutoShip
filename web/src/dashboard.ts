export interface DashboardSnapshot {
  generatedAt: string;
  environment: EnvironmentSummary;
  sessions: SessionSection;
  groups: GroupSection;
  navigation: NavigationSection;
  relocation: RelocationSection;
  economy: EconomySection;
  combat: CombatSection;
  health: HealthSection;
}

export interface EnvironmentSummary {
  cluster: string;
  shard: string;
  zone: string;
  websocketConnected: boolean;
  alerts: number;
}

export interface SessionSection {
  recoveryEnabled: boolean;
  profiles: string[];
  items: SessionCard[];
}

export interface SessionCard {
  clientId: number;
  characterName: string;
  profile: string;
  groupId: string;
  zone: string;
  level: number;
  hpPct: number;
  manaPct: number;
  status: "online" | "offline" | "stuck";
  recoveryState: "stable" | "respawning" | "waiting";
  lastHeartbeat: string;
}

export interface GroupSection {
  items: GroupCard[];
  commandLog: GroupCommandLogEntry[];
}

export interface GroupCard {
  id: string;
  name: string;
  zone: string;
  formation: string;
  currentCommand: string;
  members: GroupMember[];
}

export interface GroupMember {
  characterName: string;
  role: string;
  status: string;
}

export interface GroupCommandLogEntry {
  id: string;
  issuedAt: string;
  groupName: string;
  command: string;
  status: string;
}

export interface NavigationSection {
  currentZone: string;
  activeRouteId: string;
  stuckClients: number;
  routes: RouteCard[];
}

export interface RelocationSection {
  readyDestinations: number;
  coolingDownCount: number;
  destinations: RelocationDestinationCard[];
}

export interface RelocationDestinationCard {
  zone: string;
  label: string;
  preferredOption: string | null;
  preferredSource: "aa" | "item" | null;
  options: RelocationOptionCard[];
}

export interface RelocationOptionCard {
  id: string;
  name: string;
  source: "aa" | "item";
  owned: boolean;
  ready: boolean;
  cooldownRemainingSecs: number | null;
}

export interface RouteCard {
  id: string;
  name: string;
  zone: string;
  destination: string;
  progressPct: number;
  waypoints: Waypoint[];
}

export interface Waypoint {
  id: string;
  x: number;
  y: number;
  label: string;
}

export interface EconomySection {
  itemsReceived: number;
  lastVendorRun: string;
  totalProfit: number;
  profitTrend: TrendPoint[];
  recentLoot: LootRecord[];
  wishlist: string[];
}

export interface TrendPoint {
  label: string;
  value: number;
}

export interface LootRecord {
  id: string;
  itemName: string;
  recipient: string;
  source: string;
  distribution: string;
}

export interface CombatSection {
  dpsSeries: DpsSeries[];
  spellUsage: SpellUsage[];
  deathLog: DeathEntry[];
  rotations: RotationSummary[];
}

export interface DpsSeries {
  characterName: string;
  color: string;
  samples: number[];
}

export interface SpellUsage {
  spellName: string;
  casts: number;
  efficiency: number;
}

export interface DeathEntry {
  id: string;
  characterName: string;
  reason: string;
  recoveredAt: string;
}

export interface RotationSummary {
  characterName: string;
  efficiency: number;
  driftMs: number;
}

export interface HealthSection {
  clients: ClientHealth[];
  ipcLatency: LatencySummary;
  errorLog: ErrorLogEntry[];
}

export interface ClientHealth {
  clientId: number;
  characterName: string;
  memoryMb: number;
  frameRate: number;
  status: "healthy" | "warning" | "critical";
}

export interface LatencySummary {
  p50: number;
  p95: number;
  p99: number;
}

export interface ErrorLogEntry {
  id: string;
  severity: "info" | "warning" | "critical";
  message: string;
  recoveryAction: string;
}

export interface DashboardEvent {
  type: "dashboard.snapshot";
  source: "bootstrap" | "tick" | "action";
  snapshot: DashboardSnapshot;
}

export type DashboardActionRequest =
  | {
      type: "create_session";
      profile: string;
      character_name: string;
    }
  | {
      type: "terminate_session";
      client_id: number;
    }
  | {
      type: "recover_session";
      client_id: number;
    }
  | {
      type: "create_group";
      name: string;
      zone: string;
      formation: string;
    }
  | {
      type: "update_group";
      group_id: string;
      formation: string;
      members: GroupMember[];
    }
  | {
      type: "issue_group_command";
      group_id: string;
      command: string;
    }
  | {
      type: "create_route";
      name: string;
      zone: string;
      destination: string;
      waypoints: Waypoint[];
    }
  | {
      type: "set_active_route";
      route_id: string;
    }
  | {
      type: "update_wishlist";
      items: string[];
    };
