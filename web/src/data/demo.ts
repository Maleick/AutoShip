import type {
  Account,
  Assault,
  BankingRule,
  DpsEntry,
  DzHistoryEntry,
  DzLockout,
  KronoSettings,
  Player,
  CombatLogEntry,
  Alert,
  AutoLootFilter,
  NavItem,
  Character,
  GroupTemplate,
  RaidInstance,
  TradeskillSupply,
  VendorRoute,
  WealthHistory,
  LootRules,
  CharacterLootFilter,
  DistributionConfig,
  MasterLooter,
  LootHistoryEntry,
  LootRule,
  LootPolicy,
  LootFilterAction,
  SpawnAlertEntry,
  WatchPattern,
  SpawnAlertConfig,
  SpawnAlertStats,
} from "../types";

export const assaults: Assault[] = [
  {
    id: "assault-1",
    zone: "Plane of Hate",
    target_name: "Maestro of Rancor",
    target_hp_pct: 14.2,
    engagement_time: "00:12:34",
    forces_active: 28,
    forces_total: 36,
    avg_mana_pct: 34,
    casualties: 8,
    variant: "magenta",
  },
  {
    id: "assault-2",
    zone: "Temple of Veeshan",
    target_name: "Lendiniara the Keeper",
    target_hp_pct: 88.5,
    engagement_time: "00:01:12",
    forces_active: 36,
    forces_total: 36,
    avg_mana_pct: 91,
    casualties: 0,
    variant: "cyan",
  },
];

export const dpsRankings: DpsEntry[] = [
  { name: "XxShadowAssassin", dps: 14233, color: "#facc15" },
  { name: "Bloodfury", dps: 12105, color: "#dc2626" },
  { name: "Frostweaver", dps: 8944, color: "#60a5fa" },
];

export const players: Player[] = [
  { name: "Noxus", hp_pct: 100, zone: "Neriak Commons", status: "online" },
  { name: "Aelrindel", hp_pct: 0, zone: "Plane of Hate", status: "dead" },
  { name: "Grok", hp_pct: 42, zone: "Lower Guk", status: "online" },
  { name: "Valerius", hp_pct: 88, zone: "Oasis of Marr", status: "online" },
];

export const combatLog: CombatLogEntry[] = [
  {
    timestamp: "23:14:01",
    message: "Maestro of Rancor hits Noxus for 1,247 points of damage.",
    highlights: [
      { text: "Maestro of Rancor", color: "text-red-400", bold: true },
      { text: "Noxus", color: "text-cyan-400" },
      { text: "1,247", color: "text-yellow-300", bold: true },
    ],
  },
  {
    timestamp: "23:14:02",
    message:
      "XxShadowAssassin backstabs Maestro of Rancor for 3,891 points of damage.",
    highlights: [
      { text: "XxShadowAssassin", color: "text-yellow-400", bold: true },
      { text: "Maestro of Rancor", color: "text-red-400" },
      { text: "3,891", color: "text-yellow-300", bold: true },
    ],
  },
  {
    timestamp: "23:14:03",
    message: "Frostweaver begins casting Complete Heal on Noxus.",
    highlights: [
      { text: "Frostweaver", color: "text-blue-400", bold: true },
      { text: "Complete Heal", color: "text-green-400" },
      { text: "Noxus", color: "text-cyan-400" },
    ],
  },
  {
    timestamp: "23:14:05",
    message: "Bloodfury slashes Maestro of Rancor for 2,104 points of damage.",
    highlights: [
      { text: "Bloodfury", color: "text-red-500", bold: true },
      { text: "Maestro of Rancor", color: "text-red-400" },
      { text: "2,104", color: "text-yellow-300", bold: true },
    ],
  },
  {
    timestamp: "23:14:06",
    message: "Aelrindel has been slain by Maestro of Rancor!",
    highlights: [
      { text: "Aelrindel", color: "text-gray-500", bold: true },
      { text: "slain", color: "text-red-600", bold: true },
      { text: "Maestro of Rancor", color: "text-red-400" },
    ],
  },
  {
    timestamp: "23:14:08",
    message: "Frostweaver's Complete Heal heals Noxus for 7,500 hit points.",
    highlights: [
      { text: "Frostweaver", color: "text-blue-400" },
      { text: "Complete Heal", color: "text-green-400" },
      { text: "Noxus", color: "text-cyan-400" },
      { text: "7,500", color: "text-green-300", bold: true },
    ],
  },
  {
    timestamp: "23:14:10",
    message: "Grok crushes Maestro of Rancor for 1,566 points of damage.",
    highlights: [
      { text: "Grok", color: "text-orange-400", bold: true },
      { text: "Maestro of Rancor", color: "text-red-400" },
      { text: "1,566", color: "text-yellow-300", bold: true },
    ],
  },
];

export const alerts: Alert[] = [
  {
    type: "warning",
    message: "Aggro warning: HolyLight has exceeded threat threshold",
    detail: "Reduce healing output or activate fade ability",
    highlight: "HolyLight",
  },
  {
    type: "info",
    message: "Cazic Thule respawn window opens in 12 minutes",
    detail: "Pre-position forces at Plane of Fear zone-in",
  },
];

export const navItems: NavItem[] = [
  {
    id: "engagements",
    label: "Active Engagements",
    icon: "Sword",
    active: true,
    pulse: true,
  },
  {
    id: "formations",
    label: "Fleet Formations",
    icon: "UsersThree",
  },
  {
    id: "map",
    label: "Realm Map",
    icon: "MapTrifold",
  },
  {
    id: "security",
    label: "Security Wards",
    icon: "ShieldCheck",
  },
];

// ── Group Builder demo data ────────────────────────────────────────────────

export const demoCharacters: Character[] = [
  { id: "c1",  name: "Thorin",       eqClass: "Warrior",      level: 60, zone: "Plane of Hate",   status: "online" },
  { id: "c2",  name: "Aelara",       eqClass: "Cleric",       level: 60, zone: "Plane of Hate",   status: "online" },
  { id: "c3",  name: "Mystik",       eqClass: "Enchanter",    level: 58, zone: "Plane of Hate",   status: "online" },
  { id: "c4",  name: "Zappy",        eqClass: "Wizard",       level: 60, zone: "Plane of Hate",   status: "online" },
  { id: "c5",  name: "Stabby",       eqClass: "Rogue",        level: 57, zone: "Neriak Commons",  status: "idle"   },
  { id: "c6",  name: "Boomy",        eqClass: "Magician",     level: 59, zone: "Plane of Hate",   status: "online" },
  { id: "c7",  name: "Rhapsody",     eqClass: "Bard",         level: 60, zone: "Temple of Veeshan", status: "online" },
  { id: "c8",  name: "Valerius",     eqClass: "Paladin",      level: 55, zone: "Oasis of Marr",   status: "idle"   },
  { id: "c9",  name: "Shadows",      eqClass: "Shadow Knight",level: 58, zone: "Lower Guk",       status: "online" },
  { id: "c10", name: "Lifeline",     eqClass: "Druid",        level: 60, zone: "Temple of Veeshan", status: "online" },
  { id: "c11", name: "Bonesaw",      eqClass: "Necromancer",  level: 60, zone: "Plane of Hate",   status: "online" },
  { id: "c12", name: "Ironfist",     eqClass: "Monk",         level: 56, zone: "Neriak Commons",  status: "idle"   },
  { id: "c13", name: "Totemic",      eqClass: "Shaman",       level: 60, zone: "Temple of Veeshan", status: "online" },
  { id: "c14", name: "Snakeyes",     eqClass: "Ranger",       level: 54, zone: "East Commonlands", status: "idle"   },
  { id: "c15", name: "Whirlwind",    eqClass: "Berserker",    level: 57, zone: "Lower Guk",       status: "online" },
  { id: "c16", name: "Fangclaw",     eqClass: "Beastlord",    level: 55, zone: "East Commonlands", status: "offline"},
];

export const demoGroupTemplates: GroupTemplate[] = [
  {
    id: "tpl-standard",
    name: "Standard Group",
    description: "Balanced 6-man group",
    slots: [
      { role: "Tank",   characterId: "c1", locked: false },
      { role: "Healer", characterId: "c2", locked: false },
      { role: "CC",     characterId: "c3", locked: false },
      { role: "DPS",    characterId: "c4", locked: false },
      { role: "DPS",    characterId: "c6", locked: false },
      { role: "DPS",    characterId: null,  locked: false },
    ],
  },
  {
    id: "tpl-raid-main",
    name: "Raid Main Group",
    description: "Primary raid group with puller",
    slots: [
      { role: "Tank",    characterId: "c9",  locked: false },
      { role: "Healer",  characterId: "c10", locked: false },
      { role: "Support", characterId: "c13", locked: false },
      { role: "CC",      characterId: "c7",  locked: false },
      { role: "DPS",     characterId: "c11", locked: false },
      { role: "Puller",  characterId: "c12", locked: false },
    ],
  },
];

// ── Accounts demo data ───────────────────────────────────────────────────────

export const accounts: Account[] = [
  {
    id: "acct-1",
    name: "frostreaver01",
    server: "Firiona Vie",
    character: "Frostreaver",
    class: "CLR",
    group: 1,
    status: "active",
    has_password: true,
  },
  {
    id: "acct-2",
    name: "noxus01",
    server: "Firiona Vie",
    character: "Noxus",
    class: "WAR",
    group: 1,
    status: "active",
    has_password: true,
  },
  {
    id: "acct-3",
    name: "shadowdancer01",
    server: "Rizlona",
    character: "Shadowdancer",
    class: "ROG",
    group: 2,
    status: "locked",
    has_password: false,
  },
  {
    id: "acct-4",
    name: "ironclad01",
    server: "Teek",
    character: "Ironclad",
    class: "PAL",
    group: 0,
    status: "banned",
    has_password: false,
  },
];

// ── Dynamic zone demo data ──────────────────────────────────────────────────

export const dzLockouts: DzLockout[] = [
  {
    id: "dz-lockout-1",
    expedition: "Plane of Time",
    lockout_type: "6.5d full",
    character: "Frostreaver",
    expires_at: "2026-04-08T03:30:00Z",
  },
  {
    id: "dz-lockout-2",
    expedition: "Tacvi",
    lockout_type: "18h replay",
    character: "Noxus",
    expires_at: "2026-04-07T23:45:00Z",
  },
  {
    id: "dz-lockout-3",
    expedition: "Anguish",
    lockout_type: "2.5d mission",
    character: "Shadowdancer",
    expires_at: "2026-04-10T10:00:00Z",
  },
];

export const raidInstances: RaidInstance[] = [
  {
    id: "raid-inst-1",
    expedition: "Citadel of Anguish",
    zone: "Anguish",
    group: "Raid Alpha",
    entered_at: "2026-04-07T18:10:00Z",
    elapsed_secs: 8400,
    members: ["Frostreaver", "Noxus", "Shadowdancer", "Ironclad", "Mystik", "Boomy"],
  },
  {
    id: "raid-inst-2",
    expedition: "Temple of Veeshan",
    zone: "ToV",
    group: "Raid Beta",
    entered_at: "2026-04-07T19:05:00Z",
    elapsed_secs: 6900,
    members: ["Valerius", "Rhapsody", "Lifeline", "Bonesaw"],
  },
];

export const dzHistory: DzHistoryEntry[] = [
  {
    id: "dz-history-1",
    expedition: "Plane of Time",
    zone: "Plane of Time",
    completed_at: "2026-04-06T04:20:00Z",
    duration_secs: 7140,
    participants: ["Frostreaver", "Noxus", "Mystik", "Boomy", "Shadowdancer"],
    loot: ["Quarm's Token", "Timeless Breastplate Mold"],
  },
  {
    id: "dz-history-2",
    expedition: "Tacvi",
    zone: "Tacvi",
    completed_at: "2026-04-05T02:55:00Z",
    duration_secs: 5280,
    participants: ["Ironclad", "Rhapsody", "Lifeline", "Bonesaw"],
    loot: ["Qvic Portal Stone"],
  },
];

// ── Economy demo data ───────────────────────────────────────────────────────

export const kronoSettings: KronoSettings = {
  target_rate_per_day: 3,
  min_sell_price: 800,
  max_buy_price: 750,
  restock_threshold: 5,
  enabled: true,
};

export const vendorRoutes: VendorRoute[] = [
  {
    id: "vr-1",
    zone: "East Commonlands",
    npc_name: "Merchant Ooldi",
    path_notes: "Near the West Commonlands tunnel for fast reagent restocks.",
    item_categories: ["Food", "Drink", "Reagents"],
    enabled: true,
  },
  {
    id: "vr-2",
    zone: "Neriak Commons",
    npc_name: "Vira S`Lex",
    path_notes: "Armory circuit for weapon and armor liquidation.",
    item_categories: ["Weapons", "Armor"],
    enabled: true,
  },
];

export const bankingRules: BankingRule[] = [
  {
    id: "bank-plat",
    item_category: "Platinum",
    deposit_threshold: 5000,
    keep_on_hand: 500,
    auto_deposit: true,
  },
  {
    id: "bank-krono",
    item_category: "Krono",
    deposit_threshold: 10,
    keep_on_hand: 2,
    auto_deposit: true,
  },
  {
    id: "bank-tradeskill",
    item_category: "Tradeskill Mats",
    deposit_threshold: 200,
    keep_on_hand: 20,
    auto_deposit: false,
  },
];

export const tradeskillSupplies: TradeskillSupply[] = [
  {
    id: "ts-1",
    skill: "Tailoring",
    materials: ["Silk Thread", "Spiderling Silk", "High Quality Bear Skin"],
    restock_quantity: 120,
    source_zone: "East Karana",
    enabled: true,
  },
  {
    id: "ts-2",
    skill: "Smithing",
    materials: ["Iron Ore", "Coal", "High Quality Ore"],
    restock_quantity: 50,
    source_zone: "Kaladim",
    enabled: true,
  },
  {
    id: "ts-3",
    skill: "Baking",
    materials: ["Bat Wings", "Mammoth Meat", "Frosting"],
    restock_quantity: 80,
    source_zone: "Thurgadin",
    enabled: false,
  },
];

export const wealthHistory: WealthHistory = {
  snapshots: [
    {
      timestamp: "2026-04-01T00:00:00Z",
      plat: 120_000,
      krono: 28,
      item_value_estimate: 210_000,
    },
    {
      timestamp: "2026-04-02T00:00:00Z",
      plat: 134_500,
      krono: 30,
      item_value_estimate: 230_000,
    },
    {
      timestamp: "2026-04-03T00:00:00Z",
      plat: 148_200,
      krono: 33,
      item_value_estimate: 255_000,
    },
    {
      timestamp: "2026-04-04T00:00:00Z",
      plat: 155_900,
      krono: 35,
      item_value_estimate: 270_000,
    },
    {
      timestamp: "2026-04-05T00:00:00Z",
      plat: 164_100,
      krono: 38,
      item_value_estimate: 285_000,
    },
    {
      timestamp: "2026-04-06T00:00:00Z",
      plat: 172_800,
      krono: 40,
      item_value_estimate: 298_000,
    },
    {
      timestamp: "2026-04-07T00:00:00Z",
      plat: 187_430,
      krono: 42,
      item_value_estimate: 312_000,
    },
  ],
  current: {
    timestamp: "2026-04-07T00:00:00Z",
    plat: 187_430,
    krono: 42,
    item_value_estimate: 312_000,
  },
};

// ── Loot demo data ────────────────────────────────────────────────────────────

export const demoLootRules: LootRules = {
  keep_items: ["Rubicite Breastplate", "Mithril Breastplate", "Flowing Black Silk Sash"],
  sell_items: ["Rusty Sword", "Tattered Cloth"],
  destroy_items: ["Bone Chips", "Rat Whisker"],
  loot_all: true,
  auto_split: true,
};

export const demoCharacterFilters: CharacterLootFilter[] = [
  {
    character: "Frostreaver",
    filters: [
      { item_name: "Bone Chips", action: "destroy" },
      { item_name: "Rusty Sword", action: "sell" },
    ],
  },
  {
    character: "Shadowdancer",
    filters: [
      { item_name: "Bone Chips", action: "destroy" },
      { item_name: "Tattered Cloth", action: "sell" },
    ],
  },
  {
    character: "Ironclad",
    filters: [
      { item_name: "Bone Chips", action: "destroy" },
    ],
  },
  {
    character: "Lightbringer",
    filters: [
      { item_name: "Rat Whisker", action: "destroy" },
      { item_name: "Rusty Sword", action: "sell" },
    ],
  },
];

export const demoDistributionConfig: DistributionConfig = {
  rules: [
    { id: "rule-1", item_type: "armor",  quality: "nodrop", method: "need_before_greed" },
    { id: "rule-2", item_type: "weapon", quality: "nodrop", method: "need_before_greed" },
    { id: "rule-3", item_type: "armor",  quality: "rare",   method: "master_looter" },
    { id: "rule-4", item_type: "all",    quality: null,     method: "round_robin" },
  ],
};

export const demoMasterLooter: MasterLooter = { character: null };

export const demoLootHistory: LootHistoryEntry[] = [
  {
    id: 1,
    timestamp: "2025-01-15 23:14:01",
    item_name: "Mithril Breastplate",
    recipient: "Frostreaver",
    source_mob: "Maestro of Rancor",
    zone: "Plane of Hate",
    quantity: 1,
    assigned_by: "Frostreaver",
    item_type: "armor",
    quality: "nodrop",
    assigned_to: "Frostreaver",
    looted_by: "Frostreaver",
    source: "Maestro of Rancor",
    policy: "need-before-greed",
    estimated_value: 12_500,
  },
  {
    id: 2,
    timestamp: "2025-01-15 23:10:45",
    item_name: "Flowing Black Silk Sash",
    recipient: "Shadowdancer",
    source_mob: "Maestro of Rancor",
    zone: "Plane of Hate",
    quantity: 1,
    assigned_by: "Frostreaver",
    item_type: "waist",
    quality: "rare",
    assigned_to: "Shadowdancer",
    looted_by: "Frostreaver",
    source: "Maestro of Rancor",
    policy: "master-looter",
    estimated_value: 25_000,
  },
  {
    id: 3,
    timestamp: "2025-01-15 22:55:12",
    item_name: "Rubicite Breastplate",
    recipient: "Ironclad",
    source_mob: "Innoruuk",
    zone: "Plane of Hate",
    quantity: 1,
    assigned_by: "Frostreaver",
    item_type: "armor",
    quality: "nodrop",
    assigned_to: "Ironclad",
    looted_by: "Frostreaver",
    source: "Innoruuk",
    policy: "need-before-greed",
    estimated_value: 18_750,
  },
  {
    id: 4,
    timestamp: "2025-01-15 22:30:00",
    item_name: "Lendiniara's Signet Ring",
    recipient: "Lightbringer",
    source_mob: "Lendiniara the Keeper",
    zone: "Temple of Veeshan",
    quantity: 1,
    assigned_by: null,
    item_type: "jewelry",
    quality: "rare",
    assigned_to: "Lightbringer",
    looted_by: "Lightbringer",
    source: "Lendiniara the Keeper",
    policy: "round-robin",
    estimated_value: 30_000,
  },
  {
    id: 5,
    timestamp: "2025-01-15 22:20:33",
    item_name: "Bone Chips",
    recipient: "Frostreaver",
    source_mob: "Skeleton",
    zone: "Lower Guk",
    quantity: 5,
    assigned_by: null,
    item_type: "misc",
    quality: "common",
    assigned_to: "Frostreaver",
    looted_by: "Frostreaver",
    source: "Skeleton",
    policy: "greed-only",
    estimated_value: 250,
  },
  {
    id: 6,
    timestamp: "2025-01-15 21:45:00",
    item_name: "Veeshan's Peak Key",
    recipient: "Shadowdancer",
    source_mob: "Phara Dar",
    zone: "Veeshan's Peak",
    quantity: 1,
    assigned_by: "Frostreaver",
    item_type: "key",
    quality: "quest",
    assigned_to: "Shadowdancer",
    looted_by: "Frostreaver",
    source: "Phara Dar",
    policy: "need-before-greed",
    estimated_value: 1_000,
  },
];

// Compatibility fixtures for the legacy loot governance screen.
export const lootRules: LootRule[] = [
  {
    id: "loot-rule-1",
    item_scope: "Armor",
    quality: "nodrop",
    policy: "need-before-greed" as LootPolicy,
    master_looter: "Frostreaver",
    enabled: true,
  },
  {
    id: "loot-rule-2",
    item_scope: "Weapons",
    quality: "rare",
    policy: "master-looter" as LootPolicy,
    master_looter: "Shadowdancer",
    enabled: true,
  },
  {
    id: "loot-rule-3",
    item_scope: "All",
    quality: "common",
    policy: "round-robin" as LootPolicy,
    master_looter: "Raid Council",
    enabled: false,
  },
];

export const autoLootFilters: AutoLootFilter[] = [
  {
    id: "auto-filter-1",
    character_name: "Frostreaver",
    matcher: "Bone Chips",
    action: "destroy" as LootFilterAction,
    notes: "Low value trash",
  },
  {
    id: "auto-filter-2",
    character_name: "Frostreaver",
    matcher: "Rusty Sword",
    action: "sell" as LootFilterAction,
    notes: "Vendor fodder",
  },
  {
    id: "auto-filter-3",
    character_name: "Shadowdancer",
    matcher: "Flowing Black Silk Sash",
    action: "keep" as LootFilterAction,
    notes: "Raid upgrade",
  },
  {
    id: "auto-filter-4",
    character_name: "Ironclad",
    matcher: "Rubicite Breastplate",
    action: "keep" as LootFilterAction,
    notes: "Tank gear",
  },
];

export const lootHistory: LootHistoryEntry[] = [
  {
    id: 101,
    timestamp: "2025-01-15 23:14:01",
    item_name: "Mithril Breastplate",
    recipient: "Frostreaver",
    source_mob: "Maestro of Rancor",
    zone: "Plane of Hate",
    quantity: 1,
    assigned_by: "Frostreaver",
    item_type: "armor",
    quality: "nodrop",
    assigned_to: "Frostreaver",
    looted_by: "Frostreaver",
    source: "Maestro of Rancor",
    policy: "need-before-greed",
    estimated_value: 12_500,
  },
  {
    id: 102,
    timestamp: "2025-01-15 23:10:45",
    item_name: "Flowing Black Silk Sash",
    recipient: "Shadowdancer",
    source_mob: "Maestro of Rancor",
    zone: "Plane of Hate",
    quantity: 1,
    assigned_by: "Frostreaver",
    item_type: "waist",
    quality: "rare",
    assigned_to: "Shadowdancer",
    looted_by: "Frostreaver",
    source: "Maestro of Rancor",
    policy: "master-looter",
    estimated_value: 25_000,
  },
  {
    id: 103,
    timestamp: "2025-01-15 22:55:12",
    item_name: "Rubicite Breastplate",
    recipient: "Ironclad",
    source_mob: "Innoruuk",
    zone: "Plane of Hate",
    quantity: 1,
    assigned_by: "Frostreaver",
    item_type: "armor",
    quality: "nodrop",
    assigned_to: "Ironclad",
    looted_by: "Frostreaver",
    source: "Innoruuk",
    policy: "need-before-greed",
    estimated_value: 18_750,
  },
  {
    id: 104,
    timestamp: "2025-01-15 22:30:00",
    item_name: "Lendiniara's Signet Ring",
    recipient: "Lightbringer",
    source_mob: "Lendiniara the Keeper",
    zone: "Temple of Veeshan",
    quantity: 1,
    assigned_by: null,
    item_type: "jewelry",
    quality: "rare",
    assigned_to: "Lightbringer",
    looted_by: "Lightbringer",
    source: "Lendiniara the Keeper",
    policy: "round-robin",
    estimated_value: 30_000,
  },
];

// ── Spawn Alerts demo data ──────────────────────────────────────────────────────

export const spawnAlertStats: SpawnAlertStats = {
  total_alerts: 47,
  spawns_up: 3,
  spawns_down: 44,
};

export const spawnAlertConfig: SpawnAlertConfig = {
  watch_named_enabled: true,
  watch_patterns: [
    { pattern: "*Maestro*", enabled: true },
    { pattern: "Emperor Crush", enabled: true },
    { pattern: "*Rancor*", enabled: false },
    { pattern: "Lord Nagafen", enabled: true },
    { pattern: "*Innoruuk*", enabled: false },
  ],
  broadcast_to_web: true,
  broadcast_to_clients: false,
};

export const spawnAlerts: SpawnAlertEntry[] = [
  {
    id: 1,
    spawn_name: "Maestro of Rancor",
    zone: "Plane of Hate",
    is_up: true,
    timestamp: "2026-04-15T10:30:00Z",
    time_since_last_pop_ms: 3600000,
    match_source: "*Maestro*",
  },
  {
    id: 2,
    spawn_name: "Maestro of Rancor",
    zone: "Plane of Hate",
    is_up: false,
    timestamp: "2026-04-15T09:30:00Z",
    time_since_last_pop_ms: null,
    match_source: "*Maestro*",
  },
  {
    id: 3,
    spawn_name: "Emperor Crush",
    zone: "Crushbone",
    is_up: true,
    timestamp: "2026-04-15T08:00:00Z",
    time_since_last_pop_ms: 7200000,
    match_source: "Named",
  },
  {
    id: 4,
    spawn_name: "Lord Nagafen",
    zone: "Nagafen",
    is_up: false,
    timestamp: "2026-04-15T06:00:00Z",
    time_since_last_pop_ms: null,
    match_source: "Named",
  },
  {
    id: 5,
    spawn_name: "Phinigel Autropos",
    zone: "Kedge Keep",
    is_up: true,
    timestamp: "2026-04-15T05:30:00Z",
    time_since_last_pop_ms: 14400000,
    match_source: "Named",
  },
  {
    id: 6,
    spawn_name: "Innoruuk Prince",
    zone: "Plane of Hate",
    is_up: false,
    timestamp: "2026-04-14T22:00:00Z",
    time_since_last_pop_ms: null,
    match_source: "*Innoruuk*",
  },
  {
    id: 7,
    spawn_name: "Tallon Zek",
    zone: "Plane of Sky",
    is_up: false,
    timestamp: "2026-04-14T20:00:00Z",
    time_since_last_pop_ms: null,
    match_source: "Named",
  },
  {
    id: 8,
    spawn_name: "Vallon Zek",
    zone: "Plane of Sky",
    is_up: true,
    timestamp: "2026-04-14T18:00:00Z",
    time_since_last_pop_ms: 28800000,
    match_source: "Named",
  },
];
