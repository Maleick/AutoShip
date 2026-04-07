import type {
  Assault,
  DpsEntry,
  Player,
  CombatLogEntry,
  Alert,
  NavItem,
  Character,
  GroupTemplate,
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
