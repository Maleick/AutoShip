import type {
  AutoGroupConfig,
  CharacterConfig,
  CharacterConfigMap,
  SessionControlRecord,
  VendorRoute,
} from "./types.ts";

export const MOCK_SESSIONS: SessionControlRecord[] = [
  {
    session_id: 1,
    group_id: 1,
    routing_scope: "self",
    state: "active",
    character_name: "Thurgrek",
    class: "WAR",
    hp_pct: 96,
    mana_pct: 42,
    zone: "Plane of Fear",
    level: 65,
  },
  {
    session_id: 2,
    group_id: 1,
    routing_scope: "self",
    state: "active",
    character_name: "Venkhadrei",
    class: "ENC",
    hp_pct: 72,
    mana_pct: 68,
    zone: "Plane of Fear",
    level: 65,
  },
  {
    session_id: 3,
    group_id: 1,
    routing_scope: "self",
    state: "active",
    character_name: "Sylunariel",
    class: "CLR",
    hp_pct: 100,
    mana_pct: 81,
    zone: "Plane of Fear",
    level: 65,
  },
  {
    session_id: 4,
    group_id: 1,
    routing_scope: "self",
    state: "active",
    character_name: "Droznak",
    class: "SHM",
    hp_pct: 88,
    mana_pct: 59,
    zone: "Plane of Fear",
    level: 65,
  },
  {
    session_id: 5,
    group_id: 1,
    routing_scope: "self",
    state: "active",
    character_name: "Mahzekahl",
    class: "MNK",
    hp_pct: 34,
    mana_pct: 12,
    zone: "East Commons",
    level: 64,
  },
  {
    session_id: 6,
    group_id: 1,
    routing_scope: "self",
    state: "paused",
    character_name: "Vexkiir",
    class: "NEC",
    hp_pct: 88,
    mana_pct: 74,
    zone: "East Commons",
    level: 63,
  },
  {
    session_id: 7,
    group_id: 2,
    routing_scope: "group",
    state: "active",
    character_name: "Krayleen",
    class: "MAG",
    hp_pct: 8,
    mana_pct: 0,
    zone: "East Commons",
    level: 62,
  },
  {
    session_id: 8,
    group_id: 2,
    routing_scope: "group",
    state: "error",
    character_name: "Ilirendir",
    class: "RNG",
    hp_pct: 94,
    mana_pct: 65,
    zone: "Greater Faydark",
    level: 65,
  },
  {
    session_id: 9,
    group_id: 2,
    routing_scope: "group",
    state: "active",
    character_name: "Bhorrokk",
    class: "SHD",
    hp_pct: 77,
    mana_pct: 48,
    zone: "Greater Faydark",
    level: 63,
  },
  {
    session_id: 10,
    group_id: 3,
    routing_scope: "broadcast",
    state: "idle",
    character_name: "Ostrinka",
    class: "WIZ",
    hp_pct: 100,
    mana_pct: 100,
    zone: "Nexus",
    level: 60,
  },
  {
    session_id: 11,
    group_id: 3,
    routing_scope: "group",
    state: "active",
    character_name: "Drellemar",
    class: "BRD",
    hp_pct: 82,
    mana_pct: 55,
    zone: "Kael Drakkel",
    level: 65,
  },
  {
    session_id: 12,
    group_id: 3,
    routing_scope: "group",
    state: "active",
    character_name: "Zvakhess",
    class: "DRU",
    hp_pct: 91,
    mana_pct: 73,
    zone: "Kael Drakkel",
    level: 64,
  },
];

export const USE_MOCKS = import.meta.env.VITE_MOCK === "1";

// ─── Dashboard mocks ───────────────────────────────────────────────────────
export interface KpiHistory {
  clients: number[];
  groups: number[];
  economy: number[];
}

export const MOCK_KPI: KpiHistory = {
  clients: [8, 9, 9, 8, 9, 10, 9, 9, 11, 12, 11, 9],
  groups: [3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
  economy: [412.1, 410.8, 408.3, 404.7, 400.2, 395.6, 394.1, 392.8, 388.5, 386.9, 385.1, 384.6],
};

export type LogLevel =
  | "INFO"
  | "OK"
  | "CAST"
  | "WARN"
  | "LOOT"
  | "CH"
  | "ALERT"
  | "ROUTE"
  | "ECON"
  | "DMG";

export interface EventLogEntry {
  ts: string;
  level: LogLevel;
  text: string;
}

export const MOCK_LOG: EventLogEntry[] = [
  { ts: "03:18:01.4", level: "CAST", text: "Venkhadrei + tash landed · resist -75" },
  { ts: "03:18:04.2", level: "OK", text: "Sylunariel ✓ complete heal on Thurgrek · 8213 hp" },
  { ts: "03:18:07.0", level: "ECON", text: "system ● pp balance tick · -1.2" },
  { ts: "03:18:09.8", level: "ROUTE", text: "Ilirendir → waypoint reached · (-214, 882)" },
  { ts: "03:18:12.6", level: "OK", text: "Sylunariel ✓ complete heal on Thurgrek · 8213 hp" },
  { ts: "03:18:15.4", level: "WARN", text: "Mahzekahl ! stuck · awaiting unstick retry" },
  { ts: "03:18:18.2", level: "WARN", text: "Mahzekahl ! stuck · awaiting unstick retry" },
  { ts: "03:18:21.0", level: "CH", text: "chain ● CH cycle fired · MT 100%" },
  { ts: "03:18:23.8", level: "ECON", text: "system ● pp balance tick · -1.2" },
  { ts: "03:18:26.6", level: "LOOT", text: "Vexkiir ◇ lotto open · Shadow Ember" },
  { ts: "03:18:29.4", level: "ROUTE", text: "Ilirendir → waypoint reached · (-214, 882)" },
  { ts: "03:18:32.2", level: "CH", text: "chain ● CH cycle fired · MT 100%" },
  { ts: "03:18:35.0", level: "ROUTE", text: "Ilirendir → waypoint reached · (-214, 882)" },
  { ts: "03:18:37.8", level: "ROUTE", text: "Ilirendir → waypoint reached · (-214, 882)" },
  { ts: "03:18:40.6", level: "OK", text: "Sylunariel ✓ complete heal on Thurgrek · 8213 hp" },
  { ts: "03:18:43.4", level: "OK", text: "Sylunariel ✓ complete heal on Thurgrek · 8213 hp" },
  { ts: "03:18:46.2", level: "CAST", text: "Venkhadrei + tash landed · resist -75" },
  { ts: "03:18:49.0", level: "ECON", text: "system ● pp balance tick · -1.2" },
  { ts: "03:18:51.8", level: "CH", text: "chain ● CH cycle fired · MT 100%" },
  { ts: "03:18:54.6", level: "LOOT", text: "Vexkiir ◇ lotto open · Shadow Ember" },
  { ts: "03:18:57.4", level: "OK", text: "Sylunariel ✓ complete heal on Thurgrek · 8213 hp" },
];

// ─── Characters ────────────────────────────────────────────────────────────
function mkChar(
  name: string,
  cls: string,
  role: string,
  heal: number,
  mana: number,
  nuke: number,
  group: number,
): CharacterConfig {
  return {
    character_name: name,
    class: cls,
    role,
    heal_at_pct: heal,
    mana_sit_pct: mana,
    nuke_at_pct: nuke,
    rotation: [
      { id: "r1", name: "primary", priority: 1, enabled: true },
      { id: "r2", name: "burn", priority: 2, enabled: true },
      { id: "r3", name: "defensive", priority: 3, enabled: false },
    ],
    class_params: {
      ch_chain_timing_ms: cls === "CLR" ? 6000 : null,
      dot_overlap_pct: cls === "SHM" || cls === "NEC" ? 15 : null,
      burn_at_hp_pct: cls === "WIZ" || cls === "MAG" ? 90 : null,
      slow_at_hp_pct: cls === "SHM" || cls === "ENC" ? 95 : null,
    },
    auto_rez: {
      enabled: role.includes("healer") || cls === "CLR" || cls === "DRU" || cls === "SHM",
      min_xp_pct: 92,
      trusted_casters: ["Sylunariel", "Zvakhess", "Droznak"],
      decline_if_untrusted: true,
    },
    group_override: false,
    group_name: `G${group}`,
    window_title_format: "{character} · {zone}",
    tribute_preferences: {
      auto_activate: false,
      warning_threshold_secs: 300,
      preferred_tributes: [],
    },
  };
}

export const MOCK_CHARACTERS: CharacterConfigMap = Object.fromEntries(
  [
    mkChar("Thurgrek", "WAR", "main_tank", 55, 0, 0, 1),
    mkChar("Venkhadrei", "ENC", "mez", 70, 35, 0, 1),
    mkChar("Sylunariel", "CLR", "main_healer", 65, 45, 0, 1),
    mkChar("Droznak", "SHM", "healer", 70, 40, 0, 1),
    mkChar("Mahzekahl", "MNK", "melee_dps", 50, 0, 0, 1),
    mkChar("Vexkiir", "NEC", "dps", 60, 30, 80, 1),
    mkChar("Krayleen", "MAG", "ranged_dps", 70, 30, 85, 2),
    mkChar("Ilirendir", "RNG", "puller", 65, 30, 0, 2),
    mkChar("Bhorrokk", "SHD", "off_tank", 60, 30, 0, 2),
    mkChar("Ostrinka", "WIZ", "ranged_dps", 70, 25, 90, 3),
    mkChar("Drellemar", "BRD", "support", 65, 35, 0, 3),
    mkChar("Zvakhess", "DRU", "healer", 70, 40, 70, 3),
  ].map((c) => [c.character_name, c]),
);

// ─── Auto-Group config ─────────────────────────────────────────────────────
export const MOCK_AUTO_GROUP: AutoGroupConfig = {
  enabled: true,
  members: [
    { name: "Thurgrek", role: "tank" },
    { name: "Sylunariel", role: "healer" },
    { name: "Venkhadrei", role: "mez" },
    { name: "Droznak", role: "slow" },
    { name: "Mahzekahl", role: "dps" },
    { name: "Vexkiir", role: "dps" },
  ],
  completion_command: "/g ready — Fear Core up",
  max_retries: 3,
  invite_interval_ticks: 10,
  member_wait_ticks: 20,
};

// ─── Vendor routes (Economy) ───────────────────────────────────────────────
export const MOCK_VENDOR_ROUTES: VendorRoute[] = [
  {
    id: "r1",
    zone: "East Commons",
    npc_name: "Merchant Qeldar",
    path_notes: "zone-in → east tunnel → first kiosk",
    item_categories: ["weapon", "armor"],
    enabled: true,
  },
  {
    id: "r2",
    zone: "Plane of Knowledge",
    npc_name: "Poknowledge Alchemy Vendor",
    path_notes: "portal → south alchemy square",
    item_categories: ["potion", "spell_reagent"],
    enabled: true,
  },
  {
    id: "r3",
    zone: "Bazaar",
    npc_name: "Parcel Merchant",
    path_notes: "main concourse → center isle",
    item_categories: ["gem", "tradeskill"],
    enabled: false,
  },
];

// ─── Loot mock ─────────────────────────────────────────────────────────────
export const MOCK_LOOT_RULES = {
  keep_items: ["Shadow Ember", "Glowing Rune", "Rusty Flamberge", "Tome of the Ancients"],
  sell_items: ["Bronze Sword", "Small Patchwork Armor", "Tattered Cloth"],
  destroy_items: ["Bone Chips", "Rat Ears"],
  loot_all: true,
  auto_split: true,
};

export const MOCK_LOOT_HISTORY = [
  {
    id: 1,
    timestamp: "03:17:22",
    item_name: "Shadow Ember",
    recipient: "Sylunariel",
    source_mob: "Tunare",
    zone: "Plane of Growth",
    quantity: 1,
    assigned_by: "master",
  },
  {
    id: 2,
    timestamp: "03:12:48",
    item_name: "Glowing Rune",
    recipient: "Vexkiir",
    source_mob: "Cazic Thule",
    zone: "Plane of Fear",
    quantity: 1,
    assigned_by: "roll",
  },
  {
    id: 3,
    timestamp: "03:08:11",
    item_name: "Rusty Flamberge",
    recipient: "Thurgrek",
    source_mob: "Samurai of Kael",
    zone: "Kael",
    quantity: 1,
    assigned_by: "master",
  },
  {
    id: 4,
    timestamp: "03:03:57",
    item_name: "Tome of the Ancients",
    recipient: "Ostrinka",
    source_mob: "Lord Nagafen",
    zone: "Nagafen's Lair",
    quantity: 1,
    assigned_by: "roll",
  },
  {
    id: 5,
    timestamp: "02:58:02",
    item_name: "Bronze Sword",
    recipient: "vendor",
    source_mob: "orc pawn",
    zone: "Greater Faydark",
    quantity: 3,
    assigned_by: undefined,
  },
];
